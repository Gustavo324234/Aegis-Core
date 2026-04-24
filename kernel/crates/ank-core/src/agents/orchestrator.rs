use crate::agents::node::{AgentId, AgentRole, AgentState, ProjectId};
use crate::agents::message::{AgentContext, AgentMessage, AgentResult, ReportStatus};
use crate::agents::tree::AgentTree;
use crate::pcb::TaskType;
use crate::router::CognitiveRouter;
use crate::vcm::VirtualContextManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

/// Orquestador central del árbol de agentes cognitivos (Epic 43).
/// Coordina el ciclo de vida: creación, dispatch, reporte y terminación.
/// Vive como campo de `CognitiveScheduler`; no lo reemplaza.
pub struct AgentOrchestrator {
    pub tree: Arc<RwLock<AgentTree>>,
    pub router: Arc<RwLock<CognitiveRouter>>,
    pub vcm: Arc<VirtualContextManager>,
    /// Canales de entrada por agente: AgentId → Sender del canal del agente.
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
}

impl AgentOrchestrator {
    pub fn new(
        router: Arc<RwLock<CognitiveRouter>>,
        vcm: Arc<VirtualContextManager>,
    ) -> Self {
        Self {
            tree: Arc::new(RwLock::new(AgentTree::new())),
            router,
            vcm,
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Crea un ProjectSupervisor para el proyecto dado si no existe.
    /// Idempotente: dos llamadas con el mismo project_id retornan el mismo AgentId.
    pub async fn ensure_project_supervisor(
        &self,
        project_id: &ProjectId,
        project_description: &str,
    ) -> anyhow::Result<AgentId> {
        {
            let tree = self.tree.read().await;
            if let Some(root) = tree.project_root(project_id) {
                info!(
                    project_id = %project_id,
                    agent_id = %root.agent_id,
                    "[PROJECT][{}] Supervisor already active — reusing.",
                    project_id
                );
                return Ok(root.agent_id);
            }
        }

        let system_prompt = format!(
            "Eres el ProjectSupervisor del proyecto «{}». \
             Tu trabajo: entender el objetivo global, descomponerlo en tareas \
             para tus DomainSupervisors, y agregar sus reportes en un resumen ejecutivo.",
            project_description
        );

        let node = crate::agents::node::AgentNode::new(
            AgentRole::ProjectSupervisor,
            project_id.clone(),
            project_id.as_str(),
            None,
            system_prompt,
            TaskType::Planning,
        );

        let agent_id = node.agent_id;
        let (tx, rx) = mpsc::channel::<AgentMessage>(32);

        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }

        {
            let mut channels = self.channels.write().await;
            channels.insert(agent_id, tx);
        }

        let tree_ref = Arc::clone(&self.tree);
        let router_ref = Arc::clone(&self.router);
        let channels_ref = Arc::clone(&self.channels);

        tokio::spawn(async move {
            Self::run_agent_loop(agent_id, tree_ref, router_ref, channels_ref, rx, None).await;
        });

        info!(
            project_id = %project_id,
            agent_id = %agent_id,
            "[PROJECT][{}] ProjectSupervisor spawned.",
            project_id
        );

        Ok(agent_id)
    }

    /// Spawnea un nuevo agente como hijo del padre indicado.
    /// Falla con `Err` si el padre no existe en el árbol.
    pub async fn spawn_agent(
        &self,
        role: AgentRole,
        project_id: ProjectId,
        domain: String,
        parent_id: AgentId,
        system_prompt: String,
        task_type: TaskType,
    ) -> anyhow::Result<AgentId> {
        {
            let tree = self.tree.read().await;
            if tree.get(&parent_id).is_none() {
                anyhow::bail!("Parent agent {} not found in tree", parent_id);
            }
        }

        let node = crate::agents::node::AgentNode::new(
            role.clone(),
            project_id.clone(),
            domain.clone(),
            Some(parent_id),
            system_prompt,
            task_type,
        );

        let agent_id = node.agent_id;
        let (tx, rx) = mpsc::channel::<AgentMessage>(32);

        {
            let mut tree = self.tree.write().await;
            tree.insert(node)?;
        }

        let parent_tx = {
            let channels = self.channels.read().await;
            channels.get(&parent_id).cloned()
        };

        {
            let mut channels = self.channels.write().await;
            channels.insert(agent_id, tx);
        }

        let tree_ref = Arc::clone(&self.tree);
        let router_ref = Arc::clone(&self.router);
        let channels_ref = Arc::clone(&self.channels);

        tokio::spawn(async move {
            Self::run_agent_loop(agent_id, tree_ref, router_ref, channels_ref, rx, parent_tx).await;
        });

        info!(
            parent = %parent_id,
            agent_id = %agent_id,
            role = ?role,
            domain = %domain,
            project = %project_id,
            "[DOMAIN][{}] Agent spawned under parent {}.",
            domain, parent_id
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

        let channels = self.channels.read().await;
        let tx = channels
            .get(&agent_id)
            .ok_or_else(|| anyhow::anyhow!("No channel found for agent {}", agent_id))?;

        tx.send(msg).await.map_err(|e| {
            anyhow::anyhow!("Failed to dispatch to agent {}: {}", agent_id, e)
        })?;

        {
            let mut tree = self.tree.write().await;
            if let Some(node) = tree.get_mut(&agent_id) {
                node.set_state(AgentState::Running);
            }
        }

        Ok(())
    }

    /// Loop interno de un agente. Se ejecuta en su propio `tokio::spawn`.
    /// Espera mensajes Dispatch, ejecuta la tarea (via CMR — integrado en CORE-165),
    /// y reporta al padre.
    async fn run_agent_loop(
        agent_id: AgentId,
        tree: Arc<RwLock<AgentTree>>,
        router: Arc<RwLock<CognitiveRouter>>,
        channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
        mut rx: mpsc::Receiver<AgentMessage>,
        parent_tx: Option<mpsc::Sender<AgentMessage>>,
    ) {
        let (role_label, domain_label, task_type) = {
            let t = tree.read().await;
            if let Some(node) = t.get(&agent_id) {
                let label = match node.role {
                    AgentRole::ProjectSupervisor => "PROJECT",
                    AgentRole::DomainSupervisor => "DOMAIN",
                    AgentRole::Specialist => "SPECIALIST",
                };
                (label, node.domain.clone(), node.task_type)
            } else {
                ("UNKNOWN", String::new(), TaskType::Chat)
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
                        "[{}][{}] Dispatch received: {}",
                        role_label, domain_label, &task_description[..task_description.len().min(80)]
                    );

                    // CORE-165 integra el CMR aquí. Por ahora se delega routing
                    // al CognitiveRouter existente construyendo un PCB mínimo.
                    let routing_result = {
                        let r = router.read().await;
                        let mut mock_pcb = crate::pcb::PCB::new(
                            format!("agent_{}", agent_id),
                            5,
                            task_description.clone(),
                        );
                        mock_pcb.task_type = task_type;
                        r.decide(&mock_pcb).await
                    };

                    let model_id = match routing_result {
                        Ok(decision) => {
                            info!(
                                "[{}][{}][CORE-165] → modelo seleccionado: {} (task: {:?})",
                                role_label, domain_label, decision.model_id, task_type
                            );
                            decision.model_id
                        }
                        Err(e) => {
                            warn!(
                                "[{}][{}] CMR routing failed: {}. Marking agent as Failed.",
                                role_label, domain_label, e
                            );
                            {
                                let mut t = tree.write().await;
                                if let Some(node) = t.get_mut(&agent_id) {
                                    node.set_state(AgentState::Failed {
                                        reason: e.to_string(),
                                    });
                                }
                            }
                            if let Some(ref ptx) = parent_tx {
                                let report = AgentMessage::Report {
                                    from: agent_id,
                                    result: AgentResult {
                                        agent_id,
                                        role_description: format!("{}/{}", role_label, domain_label),
                                        summary: format!("Agent failed: {}", e),
                                        artifacts: Vec::new(),
                                        metadata: serde_json::Value::Null,
                                    },
                                    status: ReportStatus::Failure {
                                        reason: e.to_string(),
                                    },
                                };
                                let _ = ptx.send(report).await;
                            }
                            return;
                        }
                    };

                    // Aggregar reportes de hijos si los hay
                    let child_summary = if !context.child_reports.is_empty() {
                        context
                            .child_reports
                            .iter()
                            .map(|r| format!("• {}: {}", r.role_description, r.summary))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        String::new()
                    };

                    let summary = if child_summary.is_empty() {
                        format!(
                            "[{}][{}] Task acknowledged via model {}. Awaiting execution.",
                            role_label, domain_label, model_id
                        )
                    } else {
                        format!(
                            "[{}][{}] Aggregated from sub-agents:\n{}",
                            role_label, domain_label, child_summary
                        )
                    };

                    {
                        let mut t = tree.write().await;
                        if let Some(node) = t.get_mut(&agent_id) {
                            node.set_state(AgentState::Complete);
                        }
                    }

                    let result = AgentResult {
                        agent_id,
                        role_description: format!("{}/{}", role_label, domain_label),
                        summary,
                        artifacts: Vec::new(),
                        metadata: serde_json::json!({
                            "model_used": model_id,
                            "reply_to": reply_to.to_string(),
                        }),
                    };

                    if let Some(ref ptx) = parent_tx {
                        let report = AgentMessage::Report {
                            from: agent_id,
                            result,
                            status: ReportStatus::Success,
                        };
                        if let Err(e) = ptx.send(report).await {
                            error!(
                                "[{}][{}] Failed to send report to parent: {}",
                                role_label, domain_label, e
                            );
                        }
                    }
                }
                AgentMessage::Cancel { reason } => {
                    warn!(
                        agent = %agent_id,
                        "[{}][{}] Cancellation received: {}",
                        role_label, domain_label, reason
                    );
                    {
                        let mut t = tree.write().await;
                        if let Some(node) = t.get_mut(&agent_id) {
                            node.set_state(AgentState::Failed { reason });
                        }
                    }
                    // Limpiar canal propio
                    let mut ch = channels.write().await;
                    ch.remove(&agent_id);
                    return;
                }
                AgentMessage::Report { from, result, status } => {
                    info!(
                        "[{}][{}] Received report from child {}: {:?}",
                        role_label, domain_label, from, status
                    );
                    // Verificar si todos los hijos completaron para re-despachar síntesis
                    let all_children_done = {
                        let t = tree.read().await;
                        if let Some(node) = t.get(&agent_id) {
                            node.children.iter().all(|child_id| {
                                t.get(child_id)
                                    .map(|c| {
                                        matches!(c.state, AgentState::Complete | AgentState::Failed { .. })
                                    })
                                    .unwrap_or(true)
                            })
                        } else {
                            false
                        }
                    };

                    if all_children_done {
                        info!(
                            "[{}][{}] All children reported. Synthesizing.",
                            role_label, domain_label
                        );
                        // Collect child results and build aggregate dispatch
                        let child_results = {
                            let t = tree.read().await;
                            if let Some(node) = t.get(&agent_id) {
                                node.children
                                    .iter()
                                    .filter_map(|cid| t.get(cid))
                                    .map(|c| AgentResult {
                                        agent_id: c.agent_id,
                                        role_description: c.domain.clone(),
                                        summary: String::from("Child completed"),
                                        artifacts: Vec::new(),
                                        metadata: serde_json::Value::Null,
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                vec![result]
                            }
                        };

                        let synth_msg = AgentMessage::Dispatch {
                            task_description: "Synthesize all child reports into a final summary.".to_string(),
                            context: AgentContext {
                                child_reports: child_results,
                                relevant_files: Vec::new(),
                                memory_snippets: Vec::new(),
                                token_budget: 4096,
                            },
                            reply_to: agent_id,
                            deadline_ms: None,
                        };

                        let self_tx = {
                            let ch = channels.read().await;
                            ch.get(&agent_id).cloned()
                        };

                        if let Some(stx) = self_tx {
                            let _ = stx.send(synth_msg).await;
                        }
                    }
                }
            }
        }
    }

    /// Procesa el reporte de un agente subordinado.
    pub async fn handle_report(
        &self,
        from: AgentId,
        result: AgentResult,
        status: ReportStatus,
    ) -> anyhow::Result<()> {
        let parent_id = {
            let tree = self.tree.read().await;
            tree.get(&from).and_then(|n| n.parent_id)
        };

        if let Some(pid) = parent_id {
            let channels = self.channels.read().await;
            if let Some(ptx) = channels.get(&pid) {
                ptx.send(AgentMessage::Report { from, result, status })
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to forward report: {}", e))?;
            }
        }
        Ok(())
    }

    /// Elimina un agente y su subárbol del árbol activo.
    pub async fn terminate(&self, agent_id: &AgentId) -> anyhow::Result<()> {
        {
            let channels = self.channels.read().await;
            if let Some(tx) = channels.get(agent_id) {
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
            let mut channels = self.channels.write().await;
            channels.remove(agent_id);
            for desc_id in &descendants {
                channels.remove(desc_id);
            }
        }

        {
            let mut tree = self.tree.write().await;
            tree.prune(agent_id)?;
        }

        Ok(())
    }

    /// Retorna una copia del árbol actual (para la API HTTP — CORE-163).
    pub async fn tree_snapshot(&self) -> AgentTree {
        let tree = self.tree.read().await;
        // Clone the tree by rebuilding it from the current state.
        // AgentTree no implementa Clone directamente para evitar copiar los canales.
        // Construimos un árbol nuevo sin canales para serialización.
        let mut snapshot = AgentTree::new();
        for node in tree.all_roots() {
            let _ = Self::clone_subtree(&tree, &mut snapshot, node.agent_id);
        }
        snapshot
    }

    fn clone_subtree(
        src: &AgentTree,
        dst: &mut AgentTree,
        id: AgentId,
    ) -> anyhow::Result<()> {
        if let Some(node) = src.get(&id) {
            let cloned = node.clone();
            dst.insert(cloned)?;
            for child_id in src.get(&id).map(|n| n.children.clone()).unwrap_or_default() {
                Self::clone_subtree(src, dst, child_id)?;
            }
        }
        Ok(())
    }
}
