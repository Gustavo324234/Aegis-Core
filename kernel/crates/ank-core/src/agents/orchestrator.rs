use crate::agents::context::ContextBudget;
use crate::agents::instructions::{state_summary_template, InstructionLoader};
use crate::agents::message::{
    AgentContext, AgentMessage, AgentResult, AgentToolCall, QueryId, ReportStatus,
    ToolCallReportStatus,
};
use crate::agents::node::{AgentId, AgentRole, AgentState, ProjectId};
use crate::agents::persistence::AgentPersistence;
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
}

impl AgentOrchestrator {
    pub fn new(
        router: Arc<RwLock<CognitiveRouter>>,
        vcm: Arc<VirtualContextManager>,
        workspace_root: &std::path::Path,
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
        {
            let tree = self.tree.read().await;
            if let Some(root) = tree.project_root(project_id) {
                info!(
                    project = %project_id,
                    "[PROJECT] Already active — reusing agent {}.",
                    root.agent_id
                );
                return Ok(root.agent_id);
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

    /// CORE-243: Alias simplificado para el Chat Agent.
    /// Permite crear un proyecto desde el contexto del Chat (sin agent_id).
    pub async fn create_project(
        &self,
        name: String,
        scope: String,
        _task_type: Option<crate::pcb::TaskType>,
        tenant_id: Option<String>,
    ) -> anyhow::Result<crate::agents::node::AgentId> {
        self.activate_project(tenant_id.as_deref().unwrap_or("default"), &scope, &name)
            .await
    }

    async fn create_project_supervisor(
        &self,
        _tenant_id: &str,
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
        );
        node.context_budget = budget.max_tokens;

        let agent_id = node.agent_id;
        let (tx, _rx) = mpsc::channel::<AgentMessage>(32);

        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }
        {
            let mut ch = self.channels.write().await;
            ch.insert(agent_id, tx);
        }

        self.spawn_loop(agent_id, None);

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

        // Insertar nodos en el árbol activo y asignar state summaries como contexto
        {
            let mut tree = self.tree.write().await;
            let mut loader = self.instruction_loader.write().await;

            let snapshot = restored_tree.serialize()?;
            for mut node in snapshot.nodes {
                // Inyectar el state summary como contexto inicial si existe
                if let Some(summary) = summaries.get(&node.agent_id) {
                    let base_prompt =
                        loader.build_system_prompt(&node.role, project_id, Some(summary.as_str()));
                    node.system_prompt = base_prompt;
                }
                let agent_id = node.agent_id;
                let _parent_tx = node.parent_id.and_then(|_pid| {
                    // Se asignará al crear el canal — se pasa None por ahora
                    None::<mpsc::Sender<AgentMessage>>
                });
                tree.nodes_mut_raw().insert(agent_id, node);
                if matches!(
                    tree.get(&agent_id).map(|n| &n.role),
                    Some(AgentRole::ProjectSupervisor { .. })
                ) {
                    tree.register_root(project_id.clone(), agent_id);
                }
            }
        }

        // Crear canales y loops para todos los nodos restaurados
        let agent_ids: Vec<AgentId> = {
            let tree = self.tree.read().await;
            tree.descendants(&root_id)
                .iter()
                .map(|n| n.agent_id)
                .chain(std::iter::once(root_id))
                .collect()
        };

        for agent_id in agent_ids {
            let (tx, _rx) = mpsc::channel::<AgentMessage>(32);
            let mut ch = self.channels.write().await;
            ch.insert(agent_id, tx);
            // Los loops se crean bajo demanda al recibir el primer Dispatch
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

        let mut node = crate::agents::node::AgentNode::new(
            role,
            project_id.clone(),
            Some(parent_id),
            system_prompt,
            task_type,
        );
        node.context_budget = budget.max_tokens;

        let agent_id = node.agent_id;
        let (tx, _rx) = mpsc::channel::<AgentMessage>(32);

        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }

        let parent_tx = {
            let ch = self.channels.read().await;
            ch.get(&parent_id).cloned()
        };

        {
            let mut ch = self.channels.write().await;
            ch.insert(agent_id, tx);
        }

        self.spawn_loop(agent_id, parent_tx);

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

        {
            let mut tree = self.tree.write().await;
            tree.prune(agent_id)?;
        }
        Ok(())
    }

    // --- UI / snapshot ---

    pub async fn tree_snapshot(&self) -> Vec<AgentNodeSummary> {
        let tree = self.tree.read().await;
        tree.all_nodes()
            .iter()
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

    fn spawn_loop(&self, agent_id: AgentId, parent_tx: Option<mpsc::Sender<AgentMessage>>) {
        let tree_ref = Arc::clone(&self.tree);
        let router_ref = Arc::clone(&self.router);
        let channels_ref = Arc::clone(&self.channels);

        let (tx, rx) = {
            // Reemplazar el canal existente con uno que tenga rx
            let tx_existing = {
                let ch = channels_ref.try_read();
                ch.ok().and_then(|c| c.get(&agent_id).cloned())
            };
            if tx_existing.is_some() {
                // Ya hay canal — crear un nuevo par y actualizar
                let (new_tx, new_rx) = mpsc::channel::<AgentMessage>(32);
                (new_tx, new_rx)
            } else {
                mpsc::channel::<AgentMessage>(32)
            }
        };

        {
            // Actualizar el canal en el mapa
            if let Ok(mut ch) = channels_ref.try_write() {
                ch.insert(agent_id, tx);
            }
        }

        tokio::spawn(async move {
            Self::run_agent_loop(agent_id, tree_ref, router_ref, channels_ref, rx, parent_tx).await;
        });
    }

    async fn run_agent_loop(
        agent_id: AgentId,
        tree: Arc<RwLock<AgentTree>>,
        router: Arc<RwLock<CognitiveRouter>>,
        channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
        mut rx: mpsc::Receiver<AgentMessage>,
        parent_tx: Option<mpsc::Sender<AgentMessage>>,
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

        while let Some(msg) = rx.recv().await {
            match msg {
                AgentMessage::Dispatch {
                    task_description,
                    context,
                    reply_to,
                    ..
                } => {
                    info!(
                        agent = %agent_id,
                        "[{}] Dispatch: {}",
                        role_label,
                        &task_description[..task_description.len().min(80)]
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

                    let model_id = match routing_result {
                        Ok(d) => {
                            info!(
                                "[{}][CORE-208] → modelo: {} (task: {:?}, pref: {:?})",
                                role_label, d.model_id, task_type, model_preference
                            );
                            d.model_id
                        }
                        Err(e) => {
                            warn!("[{}] CMR routing failed: {}", role_label, e);
                            Self::fail_agent(agent_id, &e.to_string(), &tree, &parent_tx).await;
                            return;
                        }
                    };

                    let child_summary = context
                        .child_reports
                        .iter()
                        .map(|r| format!("• {}: {}", r.role_description, r.summary))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let summary = if child_summary.is_empty() {
                        format!(
                            "[{}] Task via model {}. Awaiting execution.",
                            role_label, model_id
                        )
                    } else {
                        format!("[{}] Aggregated:\n{}", role_label, child_summary)
                    };

                    {
                        let mut t = tree.write().await;
                        if let Some(n) = t.get_mut(&agent_id) {
                            n.set_state(AgentState::Complete);
                            n.set_last_report(summary.clone());
                        }
                    }

                    if let Some(ref ptx) = parent_tx {
                        let report = AgentMessage::Report {
                            from: agent_id,
                            result: AgentResult {
                                agent_id,
                                role_description: role_label.clone(),
                                summary,
                                artifacts: Vec::new(),
                                metadata: serde_json::json!({
                                    "model_used": model_id,
                                    "reply_to": reply_to.to_string(),
                                }),
                            },
                            status: ReportStatus::Success,
                        };
                        if let Err(e) = ptx.send(report).await {
                            error!("[{}] Failed to report to parent: {}", role_label, e);
                        }
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

                    if all_done {
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
