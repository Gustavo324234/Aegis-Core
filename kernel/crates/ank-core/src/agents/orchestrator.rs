use crate::agents::context::ContextBudget;
use crate::agents::event::AgentEvent;
use crate::agents::instructions::{state_summary_template, InstructionLoader};
use crate::agents::message::{
    AgentContext, AgentMessage, AgentResult, AgentToolCall, QueryId, ReportStatus,
    ToolCallReportStatus,
};
use crate::agents::node::{AgentId, AgentRole, AgentState, ProjectId};
use crate::agents::persistence::AgentPersistence;
use crate::agents::project_ledger::ProjectLedger;
use crate::agents::tool_registry::{ProviderKind, ToolRegistry};

use crate::agents::tree::AgentTree;
use crate::pcb::TaskType;
use crate::router::CognitiveRouter;
use crate::scheduler::ModelPreference;
use crate::vcm::VirtualContextManager;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

/// Snapshot de un nodo para la UI y el Chat Agent (sin datos de runtime).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentNodeSummary {
    pub agent_id: AgentId,
    pub role_label: String,
    pub state: String,
    pub project_id: ProjectId,
    pub parent_id: Option<AgentId>,
    pub model: String,
    pub task_type: String,
    pub is_restored: bool,
    pub last_report: Option<String>,
    /// true si el proveedor del agente no soporta tool use (CORE-237).
    pub degraded: bool,
    /// Tenant dueño de este agente — para filtrado cross-tenant (CORE-300).
    pub tenant_id: String,
}

/// CORE-287: Convierte un nombre de proyecto en un ID de filesystem válido.
/// "Aegis-Core" → "aegis-core"
/// "Mi Proyecto 2025!" → "mi-proyecto-2025"
fn sanitize_project_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Orquestador central del árbol de agentes cognitivos — Epic 45.
///
/// Responsabilidades:
/// - Ciclo de vida de nodos: spawn, dispatch, terminate
/// - Routing de mensajes entre nodos (Dispatch, Report, Query, QueryReply)
/// - Persistencia en cierre de sesión: state summaries + árbol
/// - Restauración al reactivar un proyecto
/// - CMR per-agent: selección de modelo según task_type y model_preference por nodo
pub struct AgentOrchestrator {
    pub tree: Arc<RwLock<AgentTree>>,
    pub router: Arc<RwLock<CognitiveRouter>>,
    pub vcm: Arc<VirtualContextManager>,
    /// Canales de entrada por agente.
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
    persistence: Arc<AgentPersistence>,
    instruction_loader: Arc<RwLock<InstructionLoader>>,
    /// CORE-262: HAL para inferencia LLM real en run_agent_loop.
    pub hal: Arc<crate::chal::CognitiveHAL>,
    /// CORE-263: Canales oneshot para respuestas de usuario a supervisores pausados.
    pending_user_replies: Arc<RwLock<HashMap<AgentId, tokio::sync::oneshot::Sender<String>>>>,
    /// Preguntas de supervisor que siguen sin responder, indexadas por agente.
    /// Guardamos el evento `SupervisorQuestion` completo para poder re-enviarlo
    /// cuando el usuario reconecta el WebSocket (replay) — el broadcast original
    /// se perdió mientras estaba desconectado.
    pending_questions: Arc<RwLock<HashMap<AgentId, AgentEvent>>>,
    /// CORE-268: Canal de broadcast para emitir AgentEvents al WebSocket del tenant.
    event_tx: std::sync::RwLock<Option<tokio::sync::broadcast::Sender<AgentEvent>>>,
    /// CORE-FIX (A1): tokens de cancelación por agente. Cuando el usuario cierra
    /// el WebSocket (o un endpoint admin lo pide), `cancel_tenant_agents`
    /// dispara los tokens de todos los agents del tenant para que sus
    /// `run_agent_loop`s salgan limpiamente en lugar de seguir quemando
    /// tokens en background hasta el AGENT_IDLE_TIMEOUT de 5 min.
    cancel_tokens: Arc<RwLock<HashMap<AgentId, tokio_util::sync::CancellationToken>>>,
}

impl AgentOrchestrator {
    pub fn new(
        router: Arc<RwLock<CognitiveRouter>>,
        vcm: Arc<VirtualContextManager>,
        workspace_root: &std::path::Path,
        hal: Arc<crate::chal::CognitiveHAL>,
    ) -> Self {
        let mut loader = InstructionLoader::default_from_workspace(workspace_root);
        if let Err(e) = loader.preload() {
            warn!(
                "[AgentOrchestrator] InstructionLoader preload failed: {}. Using fallbacks.",
                e
            );
        }
        Self {
            tree: Arc::new(RwLock::new(AgentTree::new())),
            router,
            vcm,
            channels: Arc::new(RwLock::new(HashMap::new())),
            persistence: Arc::new(AgentPersistence::from_env()),
            instruction_loader: Arc::new(RwLock::new(loader)),
            hal,
            pending_user_replies: Arc::new(RwLock::new(HashMap::new())),
            pending_questions: Arc::new(RwLock::new(HashMap::new())),
            event_tx: std::sync::RwLock::new(None),
            cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // --- Ciclo de vida ---

    /// Activa un proyecto:
    /// - Si tiene árbol guardado en disk: lo restaura y carga state summaries (CORE-193).
    /// - Si no tiene árbol: crea un ProjectSupervisor nuevo.
    pub async fn activate_project(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
        project_description: &str,
    ) -> anyhow::Result<AgentId> {
        // Verificar si ya está activo en memoria
        let root_info = {
            let tree = self.tree.read().await;
            tree.project_root(project_id).map(|n| n.agent_id)
        };

        if let Some(root_id) = root_info {
            let has_channel = self.channels.read().await.contains_key(&root_id);
            if has_channel {
                info!(
                    project = %project_id,
                    "[PROJECT] Already active — reusing agent {}.",
                    root_id
                );
                return Ok(root_id);
            } else {
                info!(
                    project = %project_id,
                    "[PROJECT] Active in memory but channel missing (died on disconnect) — reviving tree starting at agent {}.",
                    root_id
                );
                self.revive_in_memory_tree(&root_id).await?;
                return Ok(root_id);
            }
        }

        // Intentar restaurar desde filesystem (ADR-CAA-005v2)
        if self.persistence.has_saved_tree(tenant_id, project_id) {
            return self.restore_project(tenant_id, project_id).await;
        }

        // Primera activación — crear ProjectSupervisor nuevo
        self.create_project_supervisor(tenant_id, project_id, project_description)
            .await
    }

    /// Revive los canales y loops de ejecución para todos los agentes activos en el árbol in-memory
    /// que se quedaron huérfanos/sin canal debido a una desconexión de WebSocket.
    async fn revive_in_memory_tree(&self, root_id: &AgentId) -> anyhow::Result<()> {
        let bfs_order: Vec<AgentId> = {
            let tree = self.tree.read().await;
            let mut order = vec![*root_id];
            let mut stack: Vec<AgentId> = vec![*root_id];
            while let Some(current) = stack.pop() {
                let children: Vec<AgentId> =
                    tree.children(&current).iter().map(|n| n.agent_id).collect();
                for child_id in children {
                    order.push(child_id);
                    stack.push(child_id);
                }
            }
            order
        };

        // Resetear los estados de los nodos que no estén completados a Idle para que puedan reanudar ejecución
        {
            let mut tree = self.tree.write().await;
            for id in &bfs_order {
                if let Some(n) = tree.get_mut(id) {
                    if n.state != AgentState::Complete {
                        n.set_state(AgentState::Idle);
                    }
                }
            }
        }

        for agent_id in bfs_order {
            let parent_id = {
                let tree = self.tree.read().await;
                tree.get(&agent_id).and_then(|n| n.parent_id)
            };
            let parent_tx = match parent_id {
                Some(pid) => self.channels.read().await.get(&pid).cloned(),
                None => None,
            };

            // Crear el canal de comunicación
            let (tx, rx) = mpsc::channel::<AgentMessage>(32);
            {
                let mut ch = self.channels.write().await;
                ch.insert(agent_id, tx);
            }

            // Crear y registrar el token de cancelación para este agente
            let cancel_token = tokio_util::sync::CancellationToken::new();
            self.cancel_tokens
                .write()
                .await
                .insert(agent_id, cancel_token.clone());

            self.spawn_loop(agent_id, rx, parent_tx, cancel_token);
        }

        info!(
            root_id = %root_id,
            "[PROJECT] Revived active memory tree for agent."
        );
        Ok(())
    }

    /// CORE-243: Alias simplificado para el Chat Agent.
    /// Permite crear un proyecto desde el contexto del Chat (sin agent_id).
    pub async fn create_project(
        &self,
        name: String,
        scope: String,
        _task_type: Option<crate::pcb::TaskType>,
        tenant_id: Option<String>,
    ) -> anyhow::Result<crate::agents::node::AgentId> {
        // CORE-287: project_id es el nombre sanitizado, no el scope
        let project_id = sanitize_project_id(&name);
        self.activate_project(
            tenant_id.as_deref().unwrap_or("default"),
            &project_id,
            &scope,
        )
        .await
    }

    async fn create_project_supervisor(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
        description: &str,
    ) -> anyhow::Result<AgentId> {
        let role = AgentRole::ProjectSupervisor {
            name: project_id.clone(),
            description: description.to_string(),
        };
        let system_prompt = {
            let mut loader = self.instruction_loader.write().await;
            loader.build_system_prompt(&role, project_id, None)
        };
        let budget = ContextBudget::for_role(&role, TaskType::Planning);
        let mut node = crate::agents::node::AgentNode::new(
            role,
            project_id.clone(),
            None,
            system_prompt,
            TaskType::Planning,
        )
        .with_tenant(tenant_id);
        node.context_budget = budget.max_tokens;

        let agent_id = node.agent_id;
        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }

        // Crear canal y publicar tx antes de spawnear el loop (await async-safe).
        let (tx, rx) = mpsc::channel::<AgentMessage>(32);
        {
            let mut ch = self.channels.write().await;
            ch.insert(agent_id, tx);
        }
        // CORE-FIX (A1): create and register the cancel token so callers can
        // tear this agent down on demand.
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_tokens
            .write()
            .await
            .insert(agent_id, cancel_token.clone());
        self.spawn_loop(agent_id, rx, None, cancel_token);

        info!(
            project = %project_id,
            agent = %agent_id,
            "[PROJECT] ProjectSupervisor created."
        );
        Ok(agent_id)
    }

    /// Restaura un proyecto desde el filesystem (CORE-193).
    /// Carga agent_tree.json + agent_contexts/*.md como contexto inicial de cada supervisor.
    async fn restore_project(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<AgentId> {
        let restored_tree = self
            .persistence
            .load_tree(tenant_id, project_id)?
            .ok_or_else(|| anyhow::anyhow!("No saved tree for project {}", project_id))?;

        let summaries = self
            .persistence
            .load_all_summaries(tenant_id, &restored_tree)?;

        let root_id = restored_tree
            .project_root(project_id)
            .map(|n| n.agent_id)
            .ok_or_else(|| anyhow::anyhow!("No root found in restored tree for {}", project_id))?;

        let node_count = restored_tree.len();

        // Cargar el ledger del proyecto para inyectarlo al ProjectSupervisor (CORE-273)
        let ledger_context = match self.persistence.load_ledger(tenant_id, project_id) {
            Ok(Some(ledger)) => {
                let formatted = ledger.format_for_prompt();
                if formatted.is_empty() {
                    None
                } else {
                    Some(formatted)
                }
            }
            _ => None,
        };

        // Insertar nodos en el árbol activo y asignar state summaries como contexto
        {
            let mut tree = self.tree.write().await;
            let mut loader = self.instruction_loader.write().await;

            let snapshot = restored_tree.serialize()?;
            for mut node in snapshot.nodes {
                // CORE-300: propagar tenant_id en nodos restaurados sin él (backward compat).
                if node.tenant_id.is_empty() {
                    node.tenant_id = tenant_id.to_string();
                }
                let is_project_supervisor =
                    matches!(node.role, AgentRole::ProjectSupervisor { .. });

                // Construir contexto: state summary + ledger (solo para ProjectSupervisor)
                let extra_context = if is_project_supervisor {
                    let summary = summaries.get(&node.agent_id).map(|s| s.as_str());
                    let ledger_str = ledger_context.as_deref().unwrap_or("");
                    match (summary, ledger_str.is_empty()) {
                        (Some(s), false) => Some(format!("{}\n\n{}", s, ledger_str)),
                        (Some(s), true) => Some(s.to_string()),
                        (None, false) => Some(ledger_str.to_string()),
                        (None, true) => None,
                    }
                } else {
                    summaries.get(&node.agent_id).cloned()
                };

                if let Some(ctx) = extra_context {
                    let base_prompt =
                        loader.build_system_prompt(&node.role, project_id, Some(ctx.as_str()));
                    node.system_prompt = base_prompt;
                }

                let agent_id = node.agent_id;
                tree.nodes_mut_raw().insert(agent_id, node);
                if matches!(
                    tree.get(&agent_id).map(|n| &n.role),
                    Some(AgentRole::ProjectSupervisor { .. })
                ) {
                    tree.register_root(project_id.clone(), agent_id);
                }
            }
        }

        // CORE-FIX: Antes este bloque solo creaba canales descartando `_rx` y NO
        // arrancaba los loops, así que los nodos restaurados quedaban mudos. Peor:
        // también descartaba `parent_tx` (ver bloque comentado de líneas previas),
        // dejando huérfanos a los hijos restaurados — sus Reports caían al vacío.
        //
        // Ahora recorremos el árbol en BFS desde el root, así cuando spawneamos
        // cada nodo el canal de su padre ya está en `self.channels` y podemos
        // resolver el parent_tx correcto.
        let bfs_order: Vec<AgentId> = {
            let tree = self.tree.read().await;
            let mut order = vec![root_id];
            let mut stack: Vec<AgentId> = vec![root_id];
            while let Some(current) = stack.pop() {
                let children: Vec<AgentId> =
                    tree.children(&current).iter().map(|n| n.agent_id).collect();
                for child_id in children {
                    order.push(child_id);
                    stack.push(child_id);
                }
            }
            order
        };

        for agent_id in bfs_order {
            let parent_id = {
                let tree = self.tree.read().await;
                tree.get(&agent_id).and_then(|n| n.parent_id)
            };
            let parent_tx = match parent_id {
                Some(pid) => self.channels.read().await.get(&pid).cloned(),
                None => None,
            };
            // Crear canal y publicar tx antes de spawnear — así el siguiente
            // hijo en el BFS puede encontrar su parent_tx.
            let (tx, rx) = mpsc::channel::<AgentMessage>(32);
            {
                let mut ch = self.channels.write().await;
                ch.insert(agent_id, tx);
            }
            // CORE-FIX (A1): create cancel token for the restored agent.
            let cancel_token = tokio_util::sync::CancellationToken::new();
            self.cancel_tokens
                .write()
                .await
                .insert(agent_id, cancel_token.clone());
            self.spawn_loop(agent_id, rx, parent_tx, cancel_token);
        }

        info!(
            project = %project_id,
            nodes = node_count,
            "[PROJECT] Restored from filesystem ({} nodes).",
            node_count
        );
        Ok(root_id)
    }

    /// Spawea un nuevo agente como hijo del padre indicado.
    pub async fn spawn_agent(
        &self,
        role: AgentRole,
        project_id: ProjectId,
        parent_id: AgentId,
        system_prompt_hint: Option<String>,
        task_type: TaskType,
    ) -> anyhow::Result<AgentId> {
        {
            let tree = self.tree.read().await;
            if tree.get(&parent_id).is_none() {
                anyhow::bail!("Parent agent {} not found in tree", parent_id);
            }
        }

        let budget = ContextBudget::for_role(&role, task_type);
        let system_prompt = {
            let mut loader = self.instruction_loader.write().await;
            loader.build_system_prompt(&role, &project_id, system_prompt_hint.as_deref())
        };

        // CORE-300: heredar tenant_id del nodo padre para propagar aislamiento.
        let parent_tenant_id = {
            let tree = self.tree.read().await;
            tree.get(&parent_id)
                .map(|n| n.tenant_id.clone())
                .unwrap_or_default()
        };

        let mut node = crate::agents::node::AgentNode::new(
            role,
            project_id.clone(),
            Some(parent_id),
            system_prompt,
            task_type,
        )
        .with_tenant(&parent_tenant_id);
        node.context_budget = budget.max_tokens;

        let agent_id = node.agent_id;
        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }

        let parent_tx = {
            let ch = self.channels.read().await;
            ch.get(&parent_id).cloned()
        };

        // Crear canal y publicar tx antes de spawnear el loop.
        let (tx, rx) = mpsc::channel::<AgentMessage>(32);
        {
            let mut ch = self.channels.write().await;
            ch.insert(agent_id, tx);
        }
        // CORE-FIX (A1): per-agent cancel token.
        let cancel_token = tokio_util::sync::CancellationToken::new();
        self.cancel_tokens
            .write()
            .await
            .insert(agent_id, cancel_token.clone());
        self.spawn_loop(agent_id, rx, parent_tx, cancel_token);

        info!(
            parent = %parent_id,
            agent = %agent_id,
            task_type = ?task_type,
            project = %project_id,
            "[SPAWN] Agent spawned."
        );
        Ok(agent_id)
    }

    /// Despacha una tarea al agente indicado.
    pub async fn dispatch(
        &self,
        agent_id: AgentId,
        task_description: String,
        scope_hints: Vec<String>,
    ) -> anyhow::Result<()> {
        let token_budget = {
            let tree = self.tree.read().await;
            tree.get(&agent_id)
                .map(|n| n.context_budget)
                .unwrap_or(8192)
        };

        let context = AgentContext {
            relevant_files: scope_hints,
            memory_snippets: Vec::new(),
            child_reports: Vec::new(),
            token_budget,
        };

        let msg = AgentMessage::Dispatch {
            task_description,
            context,
            reply_to: agent_id,
            deadline_ms: None,
        };

        self.send_to(agent_id, msg).await?;
        {
            let mut tree = self.tree.write().await;
            if let Some(n) = tree.get_mut(&agent_id) {
                n.set_state(AgentState::Running);
            }
        }
        Ok(())
    }

    /// Envía una Query descendente (CORE-199).
    /// No genera trabajo — solo consulta información.
    pub async fn query(
        &self,
        target_id: AgentId,
        question: String,
        context_hint: Option<String>,
        reply_to: AgentId,
    ) -> anyhow::Result<QueryId> {
        let query_id = uuid::Uuid::new_v4();
        let msg = AgentMessage::Query {
            question,
            context_hint,
            reply_to,
            query_id,
        };
        self.send_to(target_id, msg).await?;
        Ok(query_id)
    }

    // --- Comunicación bottom-up: ask_user / answer_supervisor (CORE-263) ---

    /// Registra un oneshot channel para que el supervisor con `agent_id` pueda recibir
    /// la respuesta del usuario cuando éste responda via `answer_supervisor`.
    pub async fn register_user_reply(
        &self,
        agent_id: AgentId,
        tx: tokio::sync::oneshot::Sender<String>,
    ) {
        self.pending_user_replies.write().await.insert(agent_id, tx);
    }

    /// Recuerda el evento `SupervisorQuestion` de un agente para poder
    /// re-emitirlo (replay) cuando el usuario reconecte. Se limpia al responder,
    /// reanudar o expirar la pregunta.
    pub async fn set_pending_question(&self, agent_id: AgentId, question: AgentEvent) {
        self.pending_questions
            .write()
            .await
            .insert(agent_id, question);
    }

    /// Olvida la pregunta pendiente de un agente (respondida / reanudada / expirada).
    pub async fn clear_pending_question(&self, agent_id: &AgentId) {
        self.pending_questions.write().await.remove(agent_id);
    }

    /// Preguntas de supervisor sin responder que pertenecen al tenant dado.
    /// Usado por el WS de chat para re-mostrar el modal al reconectar.
    pub async fn pending_questions_for_tenant(&self, tenant_id: &str) -> Vec<AgentEvent> {
        let pending = self.pending_questions.read().await;
        if pending.is_empty() {
            return Vec::new();
        }
        let tree = self.tree.read().await;
        pending
            .iter()
            .filter_map(|(agent_id, evt)| {
                let same_tenant = tree
                    .get(agent_id)
                    .map(|n| n.tenant_id == tenant_id)
                    .unwrap_or(false);
                if same_tenant {
                    Some(evt.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Entrega la respuesta del usuario al supervisor pausado. Retorna `true` si había
    /// un supervisor esperando, `false` si no hay ninguno registrado para ese agent_id.
    pub async fn answer_user_question(&self, agent_id: AgentId, answer: String) -> bool {
        // La pregunta ya no está pendiente — limpiamos el replay store.
        self.clear_pending_question(&agent_id).await;
        match self.pending_user_replies.write().await.remove(&agent_id) {
            Some(tx) => tx.send(answer).is_ok(),
            None => false,
        }
    }

    // --- CORE-FIX (A1): cancellation API ---

    /// Cancel a single agent by id. Wakes its `run_agent_loop` immediately so
    /// it exits the `tokio::select!` instead of waiting for AGENT_IDLE_TIMEOUT.
    /// Returns `true` if a token was found and cancelled, `false` if the
    /// agent doesn't exist or has already exited.
    pub async fn cancel_agent(&self, agent_id: &AgentId) -> bool {
        let tokens = self.cancel_tokens.read().await;
        match tokens.get(agent_id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Cancel every agent that belongs to `tenant_id`. Called by the WS chat
    /// handler when the user disconnects so we stop burning provider tokens
    /// on work the user is no longer watching. Returns how many agents were
    /// signalled.
    ///
    /// Filters by `AgentNode.tenant_id` (CORE-300). Agents created before
    /// tenant_id was tracked (i.e. with an empty tenant_id) are NOT touched
    /// unless `tenant_id` is also empty — protects against accidentally
    /// nuking legacy state on upgrade.
    pub async fn cancel_tenant_agents(&self, tenant_id: &str) -> usize {
        let target_ids: Vec<AgentId> = {
            let tree = self.tree.read().await;
            tree.all_nodes()
                .iter()
                .filter(|n| n.tenant_id == tenant_id)
                .map(|n| n.agent_id)
                .collect()
        };

        if target_ids.is_empty() {
            return 0;
        }

        let tokens = self.cancel_tokens.read().await;
        let mut cancelled = 0usize;
        for id in &target_ids {
            if let Some(token) = tokens.get(id) {
                token.cancel();
                cancelled += 1;
            }
        }
        if cancelled > 0 {
            info!(
                tenant = %tenant_id,
                count = cancelled,
                "AgentOrchestrator: cancelled tenant agents on disconnect"
            );
        }
        cancelled
    }

    // --- CORE-268: AgentEvent broadcast channel ---

    /// Registra el broadcast sender para emitir AgentEvents al WebSocket del tenant.
    pub fn set_event_channel(&self, tx: tokio::sync::broadcast::Sender<AgentEvent>) {
        if let Ok(mut guard) = self.event_tx.write() {
            *guard = Some(tx);
        }
    }

    /// Emite un AgentEvent al canal registrado. No-op si no hay canal configurado.
    pub fn emit_event(&self, event: AgentEvent) {
        if let Ok(guard) = self.event_tx.read() {
            if let Some(tx) = &*guard {
                let _ = tx.send(event);
            }
        }
    }

    // --- ProjectLedger (CORE-273) ---

    /// Agrega una entrada al ledger del proyecto al que pertenece el agente.
    /// Crea el ledger si no existe. Guarda inmediatamente en disco.
    pub async fn add_project_ledger_entry(
        &self,
        tenant_id: &str,
        agent_id: AgentId,
        content: String,
    ) -> anyhow::Result<()> {
        let (project_id, role_label) = {
            let tree = self.tree.read().await;
            let node = tree
                .get(&agent_id)
                .ok_or_else(|| anyhow::anyhow!("Agent {} not found in tree", agent_id))?;
            (
                node.project_id.clone(),
                node.role.display_name().to_string(),
            )
        };

        let mut ledger = self
            .persistence
            .load_ledger(tenant_id, &project_id)
            .unwrap_or(None)
            .unwrap_or_else(|| ProjectLedger::new(project_id.clone(), project_id.clone()));

        ledger.add_entry(content, agent_id.to_string(), role_label);
        self.persistence
            .save_ledger(tenant_id, &project_id, &ledger)?;
        Ok(())
    }

    // --- Modo degradado (CORE-237) ---

    /// Marca un agente como degradado (sin soporte de tool use).
    /// Loguea WARN y actualiza el nodo en el árbol.
    pub async fn mark_agent_degraded(&self, agent_id: &AgentId, model_name: &str) {
        warn!(
            agent = %agent_id,
            model = %model_name,
            "ollama model {} does not support tool use — degraded mode",
            model_name
        );
        let mut tree = self.tree.write().await;
        if let Some(node) = tree.get_mut(agent_id) {
            node.is_degraded = true;
        }
    }

    /// Retorna true si el agente está en modo degradado.
    pub async fn is_agent_degraded(&self, agent_id: &AgentId) -> bool {
        let tree = self.tree.read().await;
        tree.get(agent_id).map(|n| n.is_degraded).unwrap_or(false)
    }

    // --- Tool Use Dispatch (EPIC 47 — CORE-234) ---

    /// Retorna el payload de herramientas para el agente indicado, serializado para su proveedor.
    /// Se inyecta en cada llamada de inferencia de agentes del árbol.
    pub async fn tools_payload_for(
        &self,
        agent_id: &AgentId,
        provider: &ProviderKind,
    ) -> Vec<serde_json::Value> {
        let tree = self.tree.read().await;
        match tree.get(agent_id) {
            Some(node) => ToolRegistry::tools_for(&node.role, provider),
            None => Vec::new(),
        }
    }

    /// Procesa un `AgentToolCall` recibido del LLM via tool use.
    /// Retorna el resultado como JSON string para incluir en el historial como `tool_result`.
    pub async fn handle_tool_call(
        &self,
        caller_id: AgentId,
        call: AgentToolCall,
    ) -> anyhow::Result<String> {
        match call {
            AgentToolCall::Spawn {
                role,
                name,
                scope,
                task_type,
            } => {
                // Verificar modo degradado: agente degradado no puede hacer spawn (CORE-237)
                if self.is_agent_degraded(&caller_id).await {
                    warn!(
                        agent = %caller_id,
                        "Degraded agent attempted spawn — returning BLOCKED"
                    );
                    return Ok(serde_json::json!({
                        "status": "blocked",
                        "reason": "Agent is in degraded mode — tool use not supported by provider"
                    })
                    .to_string());
                }

                let project_id = {
                    let tree = self.tree.read().await;
                    tree.get(&caller_id)
                        .map(|n| n.project_id.clone())
                        .unwrap_or_else(|| "default".to_string())
                };

                // Verificar que caller no es Specialist (ADR-CAA-007)
                {
                    let tree = self.tree.read().await;
                    if let Some(node) = tree.get(&caller_id) {
                        if node.role.is_specialist() {
                            anyhow::bail!(
                                "Specialist agents cannot spawn subordinates (ADR-CAA-007)"
                            );
                        }
                    }
                }

                // CORE-281: Deduplicación — si ya existe un ProjectSupervisor activo
                // para este proyecto, retornar su ID sin crear uno nuevo.
                // CORE-300: filtrar también por tenant_id del caller.
                if matches!(role, AgentRole::ProjectSupervisor { .. }) {
                    let project_name = name.as_deref().unwrap_or(&scope).to_string();
                    let caller_tenant_id = {
                        let tree = self.tree.read().await;
                        tree.get(&caller_id)
                            .map(|n| n.tenant_id.clone())
                            .unwrap_or_default()
                    };
                    let existing_id = {
                        let tree = self.tree.read().await;
                        tree.all_nodes()
                            .iter()
                            .find(|n| {
                                if let AgentRole::ProjectSupervisor { name: n_name, .. } = &n.role {
                                    n_name.to_lowercase() == project_name.to_lowercase()
                                        && n.tenant_id == caller_tenant_id
                                        && !matches!(
                                            n.state,
                                            AgentState::Complete | AgentState::Failed { .. }
                                        )
                                } else {
                                    false
                                }
                            })
                            .map(|n| n.agent_id)
                    };
                    if let Some(existing_agent_id) = existing_id {
                        return Ok(serde_json::json!({
                            "status": "already_active",
                            "agent_id": existing_agent_id.to_string(),
                            "message": format!("ProjectSupervisor for '{}' is already active.", project_name)
                        })
                        .to_string());
                    }
                }

                let agent_role = match role {
                    AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. } => {
                        AgentRole::Supervisor {
                            name: name.clone().unwrap_or_else(|| scope.clone()),
                            scope: scope.clone(),
                        }
                    }
                    AgentRole::Specialist { .. } => AgentRole::Specialist {
                        scope: scope.clone(),
                    },
                    AgentRole::ChatAgent => AgentRole::Specialist {
                        scope: scope.clone(),
                    },
                };

                let tt = task_type.unwrap_or(crate::pcb::TaskType::Code);
                let agent_id = self
                    .spawn_agent(agent_role, project_id, caller_id, None, tt)
                    .await?;

                Ok(serde_json::json!({
                    "agent_id": agent_id.to_string(),
                    "status": "spawned"
                })
                .to_string())
            }

            AgentToolCall::Query { project, question } => {
                let target_id = {
                    let tree = self.tree.read().await;
                    tree.project_root(&project)
                        .map(|n| n.agent_id)
                        .ok_or_else(|| anyhow::anyhow!("Project '{}' not active", project))?
                };
                let _query_id = self.query(target_id, question, None, caller_id).await?;
                Ok(serde_json::json!({ "answer": "Query dispatched. Reply will arrive via QueryReply message." }).to_string())
            }

            AgentToolCall::Report {
                status,
                summary,
                observations,
            } => {
                let new_state = match status {
                    ToolCallReportStatus::Completed => AgentState::Complete,
                    ToolCallReportStatus::Error => AgentState::Failed {
                        reason: summary.clone(),
                    },
                    ToolCallReportStatus::Blocked => AgentState::Failed {
                        reason: format!("Blocked: {}", summary),
                    },
                };

                {
                    let mut tree = self.tree.write().await;
                    if let Some(node) = tree.get_mut(&caller_id) {
                        node.set_state(new_state);
                        let report_text = match &observations {
                            Some(obs) => format!("{}\n\nObservations: {}", summary, obs),
                            None => summary.clone(),
                        };
                        node.set_last_report(report_text);
                    }
                }

                Ok(serde_json::json!({ "acknowledged": true }).to_string())
            }
        }
    }

    /// Procesa múltiples `spawn_agent` tool calls en paralelo via tokio::spawn (CORE-234).
    pub async fn handle_parallel_spawns(
        &self,
        _caller_id: AgentId,
        spawns: Vec<AgentToolCall>,
    ) -> Vec<anyhow::Result<String>> {
        let mut handles = Vec::new();

        for call in spawns {
            // Clonar el Arc del árbol para mover al task
            let tree_ref = Arc::clone(&self.tree);
            let router_ref = Arc::clone(&self.router);
            let vcm_ref = Arc::clone(&self.vcm);
            let loader_ref = Arc::clone(&self.instruction_loader);
            let channels_ref = Arc::clone(&self.channels);
            let persistence_ref = Arc::clone(&self.persistence);

            let handle = tokio::spawn(async move {
                // Recrear un mini-orchestrator para el spawn paralelo no es viable
                // directamente; en su lugar retornamos la llamada estructurada para
                // que el caller las procese. Este punto de integración se completará
                // cuando el Orchestrator sea instanciado con Arc<Self>.
                let _ = (
                    tree_ref,
                    router_ref,
                    vcm_ref,
                    loader_ref,
                    channels_ref,
                    persistence_ref,
                );
                match call {
                    AgentToolCall::Spawn {
                        role,
                        name,
                        scope,
                        task_type,
                    } => Ok(serde_json::json!({
                        "queued_spawn": {
                            "role": format!("{:?}", role),
                            "name": name,
                            "scope": scope,
                            "task_type": format!("{:?}", task_type),
                        }
                    })
                    .to_string()),
                    _ => Err(anyhow::anyhow!(
                        "handle_parallel_spawns only accepts Spawn calls"
                    )),
                }
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(
                handle
                    .await
                    .unwrap_or_else(|e| Err(anyhow::anyhow!("Task join error: {}", e))),
            );
        }
        results
    }

    // --- Persistencia al cerrar sesión (CORE-207) ---

    /// Cierra un proyecto: genera state summaries, serializa el árbol, destruye nodos de memoria.
    pub async fn close_project(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<()> {
        let root_id = {
            let tree = self.tree.read().await;
            tree.project_root(project_id).map(|n| n.agent_id)
        };

        let Some(root_id) = root_id else {
            return Ok(()); // Ya no está activo
        };

        // Notificar a todos los supervisores para que generen state summary
        self.generate_state_summaries(tenant_id, project_id).await?;

        // Serializar el árbol
        {
            let tree = self.tree.read().await;
            self.persistence.save_tree(tenant_id, project_id, &tree)?;
        }

        // Destruir nodos de memoria
        self.terminate(&root_id).await?;

        info!(
            project = %project_id,
            "[PROJECT] Closed and persisted."
        );
        Ok(())
    }

    /// Genera el state summary de todos los supervisores del proyecto (CORE-207).
    async fn generate_state_summaries(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<()> {
        let fecha = Utc::now().format("%Y-%m-%d").to_string();
        let supervisor_ids: Vec<AgentId> = {
            let tree = self.tree.read().await;
            tree.all_supervisors()
                .iter()
                .filter(|n| n.project_id == *project_id)
                .map(|n| n.agent_id)
                .collect()
        };

        for agent_id in supervisor_ids {
            let summary = {
                let tree = self.tree.read().await;
                if let Some(node) = tree.get(&agent_id) {
                    // Usar el último reporte si existe; si no, el template vacío
                    node.last_report
                        .clone()
                        .unwrap_or_else(|| state_summary_template(&fecha))
                } else {
                    continue;
                }
            };

            let path = self
                .persistence
                .save_state_summary(tenant_id, project_id, &agent_id, &summary)?;

            // Guardar el path en el nodo
            {
                let mut tree = self.tree.write().await;
                if let Some(node) = tree.get_mut(&agent_id) {
                    node.set_persisted_context_path(path);
                }
            }
        }
        Ok(())
    }

    /// Termina un agente y toda su subárea.
    pub async fn terminate(&self, agent_id: &AgentId) -> anyhow::Result<()> {
        {
            let ch = self.channels.read().await;
            if let Some(tx) = ch.get(agent_id) {
                let _ = tx
                    .send(AgentMessage::Cancel {
                        reason: "Terminated by orchestrator".to_string(),
                    })
                    .await;
            }
        }

        let descendants: Vec<AgentId> = {
            let tree = self.tree.read().await;
            tree.descendants(agent_id)
                .iter()
                .map(|n| n.agent_id)
                .collect()
        };

        {
            let mut ch = self.channels.write().await;
            ch.remove(agent_id);
            for d in &descendants {
                ch.remove(d);
            }
        }

        // CORE-286: Marcar todos los nodos terminados como Failed si no estaban Complete
        {
            let mut tree = self.tree.write().await;
            let all_ids: Vec<AgentId> = std::iter::once(*agent_id)
                .chain(descendants.iter().copied())
                .collect();
            for id in all_ids {
                if let Some(n) = tree.get_mut(&id) {
                    if !matches!(n.state, AgentState::Complete) {
                        n.set_state(AgentState::Failed {
                            reason: "Terminated by orchestrator".to_string(),
                        });
                    }
                }
            }
        }

        {
            let mut tree = self.tree.write().await;
            tree.prune(agent_id)?;
        }
        Ok(())
    }

    // --- UI / snapshot ---

    pub async fn tree_snapshot(&self) -> Vec<AgentNodeSummary> {
        self.tree_snapshot_filtered(None).await
    }

    /// CORE-300: Retorna solo los nodos del tenant indicado — aislamiento cross-tenant.
    pub async fn tree_snapshot_for_tenant(&self, tenant_id: &str) -> Vec<AgentNodeSummary> {
        self.tree_snapshot_filtered(Some(tenant_id)).await
    }

    async fn tree_snapshot_filtered(&self, tenant_filter: Option<&str>) -> Vec<AgentNodeSummary> {
        let tree = self.tree.read().await;
        tree.all_nodes()
            .iter()
            .filter(|n| tenant_filter.map(|t| n.tenant_id == t).unwrap_or(true))
            .map(|n| {
                let degraded_suffix = if n.is_degraded { " [degraded]" } else { "" };
                let (role_label, task_type_str) = match &n.role {
                    AgentRole::ChatAgent => {
                        (format!("Chat Agent{}", degraded_suffix), "chat".to_string())
                    }
                    AgentRole::ProjectSupervisor { name, .. } => (
                        format!("Project Supervisor — {}{}", name, degraded_suffix),
                        "planning".to_string(),
                    ),
                    AgentRole::Supervisor { name, .. } => (
                        format!("Supervisor — {}{}", name, degraded_suffix),
                        "analysis".to_string(),
                    ),
                    AgentRole::Specialist { scope } => (
                        format!("Specialist — {}{}", scope, degraded_suffix),
                        "code".to_string(),
                    ),
                };
                let model = match n.model_preference {
                    ModelPreference::CloudOnly => "cloud",
                    ModelPreference::HybridSmart => "hybrid",
                    ModelPreference::LocalOnly => "local",
                };
                AgentNodeSummary {
                    agent_id: n.agent_id,
                    role_label,
                    state: format!("{:?}", n.state),
                    project_id: n.project_id.clone(),
                    parent_id: n.parent_id,
                    model: model.to_string(),
                    task_type: task_type_str,
                    is_restored: n.is_restored,
                    last_report: n.last_report.clone(),
                    degraded: n.is_degraded,
                    tenant_id: n.tenant_id.clone(),
                }
            })
            .collect()
    }

    // --- Helpers internos ---

    async fn send_to(&self, agent_id: AgentId, msg: AgentMessage) -> anyhow::Result<()> {
        let ch = self.channels.read().await;
        let tx = ch
            .get(&agent_id)
            .ok_or_else(|| anyhow::anyhow!("No channel for agent {}", agent_id))?;
        tx.send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("Send failed to agent {}: {}", agent_id, e))?;
        Ok(())
    }

    /// Lanza el run loop del agente. El caller debe haber insertado ya el `tx`
    /// del canal en `self.channels` y pasarnos el `rx` correspondiente.
    ///
    /// CORE-FIX: La versión anterior usaba `try_write` con fallback silencioso
    /// dentro de la función, lo que bajo contención perdía el insert y dejaba
    /// al agente sin canal. Ahora la responsabilidad de publicar el canal queda
    /// en el caller (que ya está en contexto async y puede usar `write().await`
    /// sin perder la inserción).
    fn spawn_loop(
        &self,
        agent_id: AgentId,
        rx: mpsc::Receiver<AgentMessage>,
        parent_tx: Option<mpsc::Sender<AgentMessage>>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) {
        let tree_ref = Arc::clone(&self.tree);
        let router_ref = Arc::clone(&self.router);
        let channels_ref = Arc::clone(&self.channels);
        let cancel_tokens_ref = Arc::clone(&self.cancel_tokens);
        let hal_ref = Arc::clone(&self.hal);

        tokio::spawn(async move {
            Self::run_agent_loop(
                agent_id,
                tree_ref,
                router_ref,
                channels_ref,
                rx,
                parent_tx,
                hal_ref,
                cancel_token,
            )
            .await;
            // CORE-FIX (A1): limpiar el token cuando el loop termina, sea por
            // cancel, idle timeout, complete o channel close. Si no lo
            // hiciéramos, `cancel_tokens` crecería indefinidamente.
            cancel_tokens_ref.write().await.remove(&agent_id);
        });
    }

    // CI clippy flags this at 8 args. The function is a long-lived tokio task
    // body that owns its dependencies (tree, router, channels, hal, rx, ...)
    // for the agent's whole lifetime — bundling them into a single struct
    // would add a layer of indirection without simplifying any caller, since
    // `spawn_loop` is the only caller and it constructs each Arc separately.
    // Tagged with a comment + targeted allow rather than #[allow] at module
    // level so future additions get re-questioned.
    #[allow(clippy::too_many_arguments)]
    async fn run_agent_loop(
        agent_id: AgentId,
        tree: Arc<RwLock<AgentTree>>,
        router: Arc<RwLock<CognitiveRouter>>,
        channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
        mut rx: mpsc::Receiver<AgentMessage>,
        parent_tx: Option<mpsc::Sender<AgentMessage>>,
        hal: Arc<crate::chal::CognitiveHAL>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) {
        let (role_label, task_type, model_preference) = {
            let t = tree.read().await;
            if let Some(n) = t.get(&agent_id) {
                let label = n.role.display_name().to_string();
                (label, n.task_type, n.model_preference)
            } else {
                (
                    "UNKNOWN".to_string(),
                    TaskType::Chat,
                    ModelPreference::HybridSmart,
                )
            }
        };

        // Must exceed ask_user's 600s pause (chal::ask_user) so a supervisor
        // blocked waiting for the user isn't reaped mid-question. The WaitingUser
        // guard in the timeout arm below is the primary protection; this margin
        // is belt-and-suspenders.
        const AGENT_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(900);
        let mut synthesis_done = false; // CORE-288: flag anti-síntesis múltiple

        loop {
            let msg = tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(m) => m,
                        None => {
                            info!("[{}] Channel closed, loop exiting.", role_label);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(AGENT_IDLE_TIMEOUT) => {
                    // Don't reap a supervisor that's blocked on ask_user waiting
                    // for the user's answer — that pause is legitimate and can
                    // outlast an idle window. ask_user has its own 600s timeout
                    // that moves the node back to Running, after which a truly
                    // idle agent gets reaped on the next pass.
                    let waiting_user = {
                        let t = tree.read().await;
                        t.get(&agent_id)
                            .map(|n| matches!(n.state, AgentState::WaitingUser))
                            .unwrap_or(false)
                    };
                    if waiting_user {
                        continue;
                    }
                    warn!(
                        agent = %agent_id,
                        "[{}] Idle timeout after {}s — self-terminating.",
                        role_label,
                        AGENT_IDLE_TIMEOUT.as_secs()
                    );
                    {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            if matches!(n.state, AgentState::Idle | AgentState::Running | AgentState::WaitingReport) {
                                n.set_state(AgentState::Failed {
                                    reason: "Idle timeout — no activity".to_string(),
                                });
                            }
                        }
                    }
                    channels.write().await.remove(&agent_id);
                    break;
                }
                // CORE-FIX (A1): listen for the cancellation token. Wakes the
                // loop immediately when cancel_tenant_agents() (or cancel_agent)
                // fires, even mid-Dispatch — instead of waiting up to 5 minutes
                // for the idle timeout. Mark the node Failed{reason=Cancelled}
                // so the parent supervisor sees this child died deliberately.
                _ = cancel_token.cancelled() => {
                    warn!(
                        agent = %agent_id,
                        "[{}] Cancelled by orchestrator — exiting loop.",
                        role_label
                    );
                    {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            if !matches!(n.state, AgentState::Complete | AgentState::Failed { .. }) {
                                n.set_state(AgentState::Failed {
                                    reason: "Cancelled".to_string(),
                                });
                            }
                        }
                    }
                    channels.write().await.remove(&agent_id);
                    break;
                }
            };

            match msg {
                AgentMessage::Dispatch {
                    task_description,
                    context,
                    reply_to: _,
                    ..
                } => {
                    // CORE-288: ignorar Dispatch tardío si ya completamos con síntesis
                    {
                        let state = tree.read().await.get(&agent_id).map(|n| n.state.clone());
                        if matches!(state, Some(AgentState::Complete)) && synthesis_done {
                            continue;
                        }
                    }
                    // CORE-FIX: cap at a char boundary (not byte) and append an
                    // ellipsis when truncated. The old `[..min(80)]` could panic
                    // on multi-byte UTF-8 and silently dropped the tail — an
                    // 80-char task that ended in a repo URL logged without it,
                    // which led to a misdiagnosis that the URL was being lost in
                    // dispatch (it wasn't — only the log was truncated).
                    let preview: String = task_description.chars().take(160).collect();
                    let suffix = if task_description.chars().count() > 160 {
                        "…"
                    } else {
                        ""
                    };
                    info!(
                        agent = %agent_id,
                        "[{}] Dispatch: {}{}",
                        role_label,
                        preview,
                        suffix
                    );

                    // CORE-208: CMR per-agent — usa task_type + model_preference del nodo
                    let routing_result = {
                        let r = router.read().await;
                        let mut mock_pcb = crate::pcb::PCB::new(
                            format!("agent_{}", agent_id),
                            5,
                            task_description.clone(),
                        );
                        mock_pcb.task_type = task_type;
                        mock_pcb.model_pref = model_preference;
                        mock_pcb.agent_id = Some(agent_id);
                        r.decide(&mock_pcb).await
                    };

                    let decision = match routing_result {
                        Ok(d) => {
                            info!(
                                "[{}][CORE-208] → modelo: {} (task: {:?}, pref: {:?})",
                                role_label, d.model_id, task_type, model_preference
                            );
                            d
                        }
                        Err(e) => {
                            warn!("[{}] CMR routing failed: {}", role_label, e);
                            Self::fail_agent(agent_id, &e.to_string(), &tree, &parent_tx).await;
                            return;
                        }
                    };

                    // Construir mensajes del agente
                    let system_prompt = {
                        let t = tree.read().await;
                        let base = t
                            .get(&agent_id)
                            .map(|n| n.system_prompt.clone())
                            .unwrap_or_default();
                        // CORE-281: Inyectar header [PROJECT] para ProjectSupervisor
                        // para que nunca pregunte cuál es su proyecto.
                        if let Some(n) = t.get(&agent_id) {
                            if let AgentRole::ProjectSupervisor { name: ps_name, .. } = &n.role {
                                format!(
                                    "[PROJECT]\nName: {}\nProject ID: {}\nYour role: ProjectSupervisor\n\
                                     You are responsible for this specific project. Do not ask the user \
                                     which project to work on — you already know it is '{}'.\n\n{}",
                                    ps_name, n.project_id, ps_name, base
                                )
                            } else {
                                base
                            }
                        } else {
                            base
                        }
                    };

                    let child_reports_text = context
                        .child_reports
                        .iter()
                        .map(|r| format!("• {}: {}", r.role_description, r.summary))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let mut messages = vec![crate::chal::ChatMessage {
                        role: crate::chal::ChatRole::System,
                        content: Some(system_prompt),
                        ..Default::default()
                    }];
                    if !child_reports_text.is_empty() {
                        messages.push(crate::chal::ChatMessage {
                            role: crate::chal::ChatRole::System,
                            content: Some(format!(
                                "[REPORTES DE SUBAGENTES]\n{}",
                                child_reports_text
                            )),
                            ..Default::default()
                        });
                    }
                    messages.push(crate::chal::ChatMessage {
                        role: crate::chal::ChatRole::User,
                        content: Some(task_description.clone()),
                        ..Default::default()
                    });

                    let tools = {
                        let t = tree.read().await;
                        t.get(&agent_id).and_then(|n| {
                            let provider = ProviderKind::from_string(&decision.provider);
                            let defs = ToolRegistry::tools_for(&n.role, &provider);
                            if defs.is_empty() {
                                None
                            } else {
                                Some(defs)
                            }
                        })
                    };

                    let (text_tx, mut text_rx) = tokio::sync::mpsc::unbounded_channel::<
                        Result<String, crate::chal::ExecutionError>,
                    >();
                    let hal_clone = Arc::clone(&hal);
                    let model_id_for_meta = decision.model_id.clone();

                    let collect_task = tokio::spawn(async move {
                        let mut full = String::new();
                        while let Some(Ok(token)) = text_rx.recv().await {
                            full.push_str(&token);
                        }
                        full
                    });

                    if let Err(e) = hal_clone
                        .execute_agent_loop(decision, messages, tools, text_tx, agent_id)
                        .await
                    {
                        warn!("[{}] LLM execution failed: {}", role_label, e);
                        Self::fail_agent(agent_id, &e.to_string(), &tree, &parent_tx).await;
                        return;
                    }

                    let response = collect_task.await.unwrap_or_default();

                    // CORE-FIX: Si el LLM llamó la tool `report` con status=error|blocked,
                    // el handler en chal::execute_tool_call_internal ya invocó
                    // orchestrator.handle_tool_call(Report{...}), que setea el state
                    // (Complete/Failed) y el last_report en el nodo. NO sobrescribir.
                    // Solo asumimos Complete en el happy path (LLM terminó sin tool report).
                    let (final_state, terminal_report) = {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            let already_terminal =
                                matches!(n.state, AgentState::Complete | AgentState::Failed { .. });
                            if !already_terminal {
                                n.set_state(AgentState::Complete);
                            }
                            if n.last_report.is_none() && !response.is_empty() {
                                n.set_last_report(response.clone());
                            }
                            (n.state.clone(), n.last_report.clone().unwrap_or_default())
                        } else {
                            (AgentState::Complete, response.clone())
                        }
                    };

                    // CORE-268: emitir SupervisorCompleted para supervisores
                    {
                        let is_supervisor = matches!(
                            tree.read().await.get(&agent_id).map(|n| &n.role),
                            Some(
                                AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. }
                            )
                        );
                        if is_supervisor {
                            let project_name = tree
                                .read()
                                .await
                                .get(&agent_id)
                                .map(|n| n.project_id.clone())
                                .unwrap_or_default();
                            let orch_opt = hal.agent_orchestrator.read().await.clone();
                            if let Some(orch) = orch_opt {
                                orch.emit_event(AgentEvent::SupervisorCompleted {
                                    agent_id,
                                    project_name,
                                    summary: terminal_report.clone(),
                                });
                            }
                        }
                    }

                    // Map AgentState → ReportStatus para que el padre se entere
                    // si el hijo falló (antes era siempre Success aunque la tool
                    // report dijera "blocked" o "error" — bug que ocultaba fallas).
                    let report_status = match &final_state {
                        AgentState::Failed { reason } => ReportStatus::Failure {
                            reason: reason.clone(),
                        },
                        _ => ReportStatus::Success,
                    };

                    if let Some(ref ptx) = parent_tx {
                        let report = AgentMessage::Report {
                            from: agent_id,
                            result: AgentResult {
                                agent_id,
                                role_description: role_label.clone(),
                                summary: terminal_report,
                                artifacts: Vec::new(),
                                metadata: serde_json::json!({ "model_used": model_id_for_meta }),
                            },
                            status: report_status,
                        };
                        if let Err(e) = ptx.send(report).await {
                            error!("[{}] Failed to report to parent: {}", role_label, e);
                        }
                    } else {
                        // CORE-FIX: ProjectSupervisor (sin padre) queda Idle esperando
                        // nuevos Dispatch del ChatAgent. Antes eliminábamos el canal y
                        // rompíamos el loop, dejando al supervisor inutilizable y
                        // obligando al ChatAgent a respawnear (con el costo del restore).
                        {
                            let mut t = tree.write().await;
                            if let Some(n) = t.get_mut(&agent_id) {
                                if matches!(n.state, AgentState::Complete) {
                                    n.set_state(AgentState::Idle);
                                }
                            }
                        }
                        // Permitir que una nueva tanda de hijos dispare síntesis otra vez.
                        synthesis_done = false;
                        // No `break` — el loop sigue esperando mensajes (rx.recv).
                    }
                }

                AgentMessage::Report {
                    from,
                    result,
                    status,
                } => {
                    info!("[{}] Report from child {}: {:?}", role_label, from, status);
                    {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            n.set_last_report(result.summary.clone());
                        }
                    }

                    // Verificar si todos los hijos completaron
                    let all_done = {
                        let t = tree.read().await;
                        t.get(&agent_id)
                            .map(|n| {
                                n.children.iter().all(|cid| {
                                    t.get(cid)
                                        .map(|c| {
                                            matches!(
                                                c.state,
                                                AgentState::Complete | AgentState::Failed { .. }
                                            )
                                        })
                                        .unwrap_or(true)
                                })
                            })
                            .unwrap_or(false)
                    };

                    // CORE-288: solo sintetizar UNA vez — marcar antes de enviar
                    if all_done && !synthesis_done {
                        synthesis_done = true;

                        let child_results = {
                            let t = tree.read().await;
                            t.get(&agent_id)
                                .map(|n| {
                                    n.children
                                        .iter()
                                        .filter_map(|cid| t.get(cid))
                                        .map(|c| AgentResult {
                                            agent_id: c.agent_id,
                                            role_description: c.role.display_name().to_string(),
                                            summary: c.last_report.clone().unwrap_or_default(),
                                            artifacts: Vec::new(),
                                            metadata: serde_json::Value::Null,
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_else(|| vec![result])
                        };

                        let synth = AgentMessage::Dispatch {
                            task_description: "Synthesize all child reports.".to_string(),
                            context: AgentContext {
                                child_reports: child_results,
                                relevant_files: Vec::new(),
                                memory_snippets: Vec::new(),
                                token_budget: 4096,
                            },
                            reply_to: agent_id,
                            deadline_ms: None,
                        };
                        let self_tx = { channels.read().await.get(&agent_id).cloned() };
                        if let Some(stx) = self_tx {
                            let _ = stx.send(synth).await;
                        }
                    }
                    // Si all_done && synthesis_done → ignorar silenciosamente
                }

                // Query: responder con información sin generar trabajo (ADR-CAA-003)
                AgentMessage::Query {
                    question,
                    context_hint,
                    reply_to,
                    query_id,
                } => {
                    info!(
                        "[{}] Query received: {}",
                        role_label,
                        &question[..question.len().min(60)]
                    );

                    // Si tiene hijos, delegar al hijo más adecuado según context_hint
                    let children = {
                        let t = tree.read().await;
                        t.children(&agent_id)
                            .iter()
                            .map(|n| (n.agent_id, n.role.display_name().to_string()))
                            .collect::<Vec<_>>()
                    };

                    if children.is_empty() {
                        // Nodo hoja: responder directamente con last_report o indicación
                        let answer = {
                            let t = tree.read().await;
                            t.get(&agent_id)
                                .and_then(|n| n.last_report.clone())
                                .unwrap_or_else(|| {
                                    format!("Sin información disponible sobre: {}", question)
                                })
                        };
                        let reply = AgentMessage::QueryReply {
                            answer,
                            query_id,
                            from: agent_id,
                        };
                        let reply_tx = { channels.read().await.get(&reply_to).cloned() };
                        if let Some(tx) = reply_tx {
                            let _ = tx.send(reply).await;
                        }
                    } else {
                        // Delegar al primer hijo relevante (o al que coincide con context_hint)
                        let target = context_hint
                            .as_deref()
                            .and_then(|hint| {
                                children
                                    .iter()
                                    .find(|(_, name)| {
                                        name.to_lowercase().contains(&hint.to_lowercase())
                                    })
                                    .map(|(id, _)| *id)
                            })
                            .unwrap_or(children[0].0);

                        let fwd = AgentMessage::Query {
                            question,
                            context_hint,
                            reply_to: agent_id, // las respuestas suben a este nodo
                            query_id,
                        };
                        let target_tx = { channels.read().await.get(&target).cloned() };
                        if let Some(tx) = target_tx {
                            let _ = tx.send(fwd).await;
                        }
                    }
                }

                // QueryReply: condensar y reenviar hacia arriba (ADR-CAA-011)
                AgentMessage::QueryReply {
                    answer,
                    query_id,
                    from: _,
                } => {
                    info!(
                        "[{}] QueryReply for query {}: condensing.",
                        role_label, query_id
                    );
                    // Condensar — en producción el LLM resumen; aquí pasamos tal cual hacia arriba
                    if let Some(ref ptx) = parent_tx {
                        let condensed = AgentMessage::QueryReply {
                            answer,
                            query_id,
                            from: agent_id,
                        };
                        let _ = ptx.send(condensed).await;
                    }
                }

                AgentMessage::Cancel { reason } => {
                    warn!("[{}] Cancelled: {}", role_label, reason);
                    {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            n.set_state(AgentState::Failed { reason });
                        }
                    }
                    let mut ch = channels.write().await;
                    ch.remove(&agent_id);
                    return;
                }
            }
        }
    }

    async fn fail_agent(
        agent_id: AgentId,
        reason: &str,
        tree: &Arc<RwLock<AgentTree>>,
        parent_tx: &Option<mpsc::Sender<AgentMessage>>,
    ) {
        {
            let mut t = tree.write().await;
            if let Some(n) = t.get_mut(&agent_id) {
                n.set_state(AgentState::Failed {
                    reason: reason.to_string(),
                });
            }
        }
        if let Some(ptx) = parent_tx {
            let _ = ptx
                .send(AgentMessage::Report {
                    from: agent_id,
                    result: AgentResult {
                        agent_id,
                        role_description: "unknown".to_string(),
                        summary: format!("Agent failed: {}", reason),
                        artifacts: Vec::new(),
                        metadata: serde_json::Value::Null,
                    },
                    status: ReportStatus::Failure {
                        reason: reason.to_string(),
                    },
                })
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_project_id_normal() {
        assert_eq!(sanitize_project_id("Aegis-Core"), "aegis-core");
    }

    #[test]
    fn test_sanitize_project_id_spaces() {
        assert_eq!(sanitize_project_id("Mi Proyecto"), "mi-proyecto");
    }

    #[test]
    fn test_sanitize_project_id_special_chars() {
        assert_eq!(sanitize_project_id("Proyecto 2025!"), "proyecto-2025");
    }

    #[test]
    fn test_sanitize_project_id_empty() {
        assert_eq!(sanitize_project_id(""), "");
    }

    #[test]
    fn test_sanitize_project_id_multiple_separators() {
        assert_eq!(sanitize_project_id("My  --  Project"), "my-project");
    }
}
