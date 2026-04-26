pub mod compiler;
pub mod graph;
pub mod persistence;

use crate::agents::orchestrator::AgentOrchestrator;
use crate::dag::{DagNodeStatus, ExecutionGraph, GraphManager, NodeResult};
use crate::pcb::{PcbByPriority, ProcessRole, ProcessState, PCB};
use crate::scheduler::persistence::StatePersistor;
use crate::swarm::client::SwarmClient;
use crate::swarm::{NodeStatus, SwarmManager};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{error, info, instrument, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelPreference {
    LocalOnly,
    CloudOnly,
    HybridSmart,
}

impl std::str::FromStr for ModelPreference {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LocalOnly" => Ok(ModelPreference::LocalOnly),
            "CloudOnly" => Ok(ModelPreference::CloudOnly),
            "HybridSmart" => Ok(ModelPreference::HybridSmart),
            _ => Ok(ModelPreference::HybridSmart), // Fallback sensible
        }
    }
}

pub type SharedPCB = Arc<RwLock<PCB>>;

impl ModelPreference {
    /// Determina si la tarea es lo suficientemente compleja como para requerir
    /// un Hardware Tier superior (> 1).
    pub fn is_complex(&self) -> bool {
        match self {
            ModelPreference::CloudOnly => true,
            ModelPreference::HybridSmart => true,
            ModelPreference::LocalOnly => false,
        }
    }
}

impl PCB {
    pub fn mock(_pid: &str, priority: u32) -> Self {
        Self::new("MockTask".to_string(), priority, "mock prompt".to_string())
    }
}

/// CORE-154: Lightweight stats snapshot for the status API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_processes: u32,
    pub active_workers: u32,
}

/// --- EVENT BUS (Canales MPSC) ---
#[derive(Debug)]
pub enum SchedulerEvent {
    ScheduleTask(Box<PCB>),
    /// Like ScheduleTask but replies with the confirmed PID via the oneshot sender.
    ScheduleTaskConfirmed(Box<PCB>, tokio::sync::oneshot::Sender<String>),
    DispatchLocal(Box<PCB>),
    SyscallCompleted {
        pid: String,
        result: String,
    },
    RemoteEvent(String, ank_proto::v1::TaskEvent),
    RegisterGraph(Box<ExecutionGraph>),
    ProcessCompleted {
        pid: String,
        output: String,
    },
    PreemptCurrent,
    TerminateProcess(String),
    ListProcesses(tokio::sync::oneshot::Sender<Vec<PCB>>),
    /// CORE-092: Notificación de que el HAL Runner cerró su canal.
    /// El scheduler limpia `current_running` y marca el proceso como Failed.
    HalRunnerDied {
        reason: String,
    },
    /// CORE-154: Lightweight stats request for the HTTP status endpoint.
    GetStats(tokio::sync::oneshot::Sender<SchedulerStats>),
}

/// --- COGNITIVE SCHEDULER ---
pub struct CognitiveScheduler {
    pub ready_queue: BinaryHeap<PcbByPriority>,
    pub waiting_queue: HashMap<String, PCB>,
    pub process_table: HashMap<String, PCB>,
    pub current_running: Vec<String>, // CORE-154: Soporte para múltiples procesos paralelos
    /// CORE-154: Maps supervisor_pid → set of pending worker_pids.
    pub worker_tracker: HashMap<String, HashSet<String>>,
    pub last_activity: DateTime<Utc>,

    pub swarm_manager: Option<Arc<SwarmManager>>,
    pub swarm_client: Arc<SwarmClient>,
    pub internal_tx: Option<mpsc::Sender<SchedulerEvent>>,
    pub graph_manager: Arc<RwLock<GraphManager>>,
    pub persistence: Arc<dyn StatePersistor>,
    pub execution_tx: Option<mpsc::Sender<Box<PCB>>>,
    /// CORE-158 (Epic 43): Orquestador del árbol de agentes jerárquico.
    /// None hasta que se inicialice con el CognitiveRouter y VCM.
    pub agent_orchestrator: Option<Arc<AgentOrchestrator>>,
    /// CORE-185: Pending output senders keyed by pid — fired when ProcessCompleted arrives.
    pub output_pending: HashMap<String, oneshot::Sender<String>>,
}

impl CognitiveScheduler {
    pub fn new(persistence: Arc<dyn StatePersistor>) -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
            waiting_queue: HashMap::new(),
            process_table: HashMap::new(),
            current_running: Vec::new(),
            worker_tracker: HashMap::new(),
            last_activity: Utc::now(),
            swarm_manager: None,
            swarm_client: Arc::new(SwarmClient),
            internal_tx: None,
            graph_manager: Arc::new(RwLock::new(GraphManager::new())),
            persistence,
            execution_tx: None,
            agent_orchestrator: None,
            output_pending: HashMap::new(),
        }
    }

    #[instrument(skip(self, event_rx), name = "ANK_Scheduler_Loop")]
    pub async fn start(
        mut self,
        mut event_rx: mpsc::Receiver<SchedulerEvent>,
        internal_tx: mpsc::Sender<SchedulerEvent>,
    ) -> anyhow::Result<()> {
        self.internal_tx = Some(internal_tx);
        info!("Cognitive Scheduler engine initialized.");

        let mut gc_interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    use anyhow::Context;
                    self.handle_event(event).await.context("Error handling scheduler event")?;
                }

                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    use anyhow::Context;
                    self.reconcile().await.context("Error during state reconciliation")?;
                }

                _ = gc_interval.tick() => {
                    let now = chrono::Utc::now();
                    let five_mins = chrono::Duration::minutes(5);
                    self.process_table.retain(|_, pcb| {
                        let is_finished = matches!(
                            pcb.state,
                            crate::pcb::ProcessState::Completed | crate::pcb::ProcessState::Failed
                        );
                        let is_old = (now - pcb.created_at) > five_mins;
                        !(is_finished && is_old)
                    });
                }
            }
        }
    }

    #[instrument(skip(self), name = "ANK_Handle_Event")]
    async fn handle_event(&mut self, event: SchedulerEvent) -> anyhow::Result<()> {
        use anyhow::Context;
        self.last_activity = Utc::now();
        match event {
            SchedulerEvent::ScheduleTask(pcb_box) => {
                info!(pid = %pcb_box.pid, prio = pcb_box.priority, "Task queued (ScheduleTask).");
                let mut pcb = *pcb_box;

                // CORE-154: Register worker → supervisor relationship
                let parent_pid = pcb.parent_pid.clone();
                if parent_pid.is_some() {
                    pcb.role = ProcessRole::Worker;
                }

                self.persistence
                    .save_pcb(&pcb)
                    .await
                    .context("Atomic persistence failed during ScheduleTask")?;

                let worker_pid = pcb.pid.clone();
                pcb.state = ProcessState::Ready;
                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                self.ready_queue.push(PcbByPriority(pcb));

                if let Some(ppid) = parent_pid {
                    self.worker_tracker
                        .entry(ppid.clone())
                        .or_default()
                        .insert(worker_pid.clone());
                    if let Some(parent) = self.process_table.get_mut(&ppid) {
                        parent.role = ProcessRole::Supervisor;
                    }
                    info!(supervisor = %ppid, worker = %worker_pid, "CORE-154: Worker registered for supervisor.");
                }
            }
            SchedulerEvent::ScheduleTaskConfirmed(pcb_box, confirm_tx) => {
                info!(pid = %pcb_box.pid, prio = pcb_box.priority, "Task queued (ScheduleTaskConfirmed).");
                let mut pcb = *pcb_box;

                let parent_pid = pcb.parent_pid.clone();
                if parent_pid.is_some() {
                    pcb.role = ProcessRole::Worker;
                }

                self.persistence
                    .save_pcb(&pcb)
                    .await
                    .context("Atomic persistence failed during ScheduleTaskConfirmed")?;

                let confirmed_pid = pcb.pid.clone();
                pcb.state = ProcessState::Ready;
                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                self.ready_queue.push(PcbByPriority(pcb));

                if let Some(ppid) = parent_pid {
                    self.worker_tracker
                        .entry(ppid.clone())
                        .or_default()
                        .insert(confirmed_pid.clone());
                    if let Some(parent) = self.process_table.get_mut(&ppid) {
                        parent.role = ProcessRole::Supervisor;
                    }
                }

                // CORE-185: Store sender so ProcessCompleted can deliver the LLM output.
                self.output_pending.insert(confirmed_pid, confirm_tx);
            }
            SchedulerEvent::RegisterGraph(graph_box) => {
                let graph = *graph_box;
                let mut lock = self.graph_manager.write().await;
                crate::scheduler::graph::GraphIntegrator::validate_and_register(&mut lock, graph);

                let new_pcbs = lock.tick();
                drop(lock);

                for mut pcb in new_pcbs {
                    self.persistence
                        .save_pcb(&pcb)
                        .await
                        .context("Failed to persist initial DAG task")?;
                    pcb.state = ProcessState::Ready;
                    self.process_table.insert(pcb.pid.clone(), pcb.clone());
                    self.ready_queue.push(PcbByPriority(pcb));
                }
            }
            SchedulerEvent::DispatchLocal(pcb_box) => {
                info!(pid = %pcb_box.pid, prio = pcb_box.priority, "Task queued (DispatchLocal).");
                let mut pcb = *pcb_box;

                self.persistence
                    .save_pcb(&pcb)
                    .await
                    .context("Failed to persist DispatchLocal")?;

                pcb.state = ProcessState::Ready;
                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                self.ready_queue.push(PcbByPriority(pcb));
            }
            SchedulerEvent::SyscallCompleted { pid, result } => {
                if let Some(mut pcb) = self.waiting_queue.remove(&pid) {
                    info!(pid = %pid, "Syscall returned result. Returning process to Ready.");
                    pcb.registers
                        .temp_vars
                        .insert("last_syscall_result".to_string(), result);

                    self.persistence
                        .save_pcb(&pcb)
                        .await
                        .context("Failed to persist SyscallCompleted")?;

                    pcb.state = ProcessState::Ready;
                    self.ready_queue.push(PcbByPriority(pcb));
                }
            }
            SchedulerEvent::RemoteEvent(pid, remote_event) => {
                info!(pid = %pid, "Received remote execution event from Swarm.");
                if let Some(payload) = remote_event.payload {
                    match payload {
                        ank_proto::v1::task_event::Payload::Output(result) => {
                            info!(pid = %pid, "Remote process completed with output.");
                            if let Some(pcb) = self.process_table.get_mut(&pid) {
                                pcb.registers
                                    .temp_vars
                                    .insert("final_output".to_string(), result.clone());

                                self.persistence
                                    .save_pcb(pcb)
                                    .await
                                    .context("Failed to persist Remote Completed state")?;
                                pcb.state = ProcessState::Completed;
                            }

                            {
                                let mut lock = self.graph_manager.write().await;
                                let _ = lock.handle_result(NodeResult {
                                    node_id: pid.clone(),
                                    output: result,
                                    status: DagNodeStatus::Completed,
                                });
                            }

                            let new_pcbs = {
                                let mut lock = self.graph_manager.write().await;
                                lock.tick()
                            };

                            for mut pcb in new_pcbs {
                                self.persistence
                                    .save_pcb(&pcb)
                                    .await
                                    .context("Failed to persist DAG next-ready task")?;
                                pcb.state = ProcessState::Ready;
                                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                                self.ready_queue.push(PcbByPriority(pcb));
                            }
                        }
                        ank_proto::v1::task_event::Payload::Syscall(syscall) => {
                            info!(pid = %pid, "Remote process requires Syscall on Host: {}", syscall.name);
                        }
                        _ => {
                            info!(pid = %pid, "Ignored remote status payload.");
                        }
                    }
                }
            }
            SchedulerEvent::ProcessCompleted { pid, output } => {
                info!(pid = %pid, "Process completed locally.");

                // CORE-185: Fire the output sender if a Siren WS handler is waiting.
                if let Some(sender) = self.output_pending.remove(&pid) {
                    let _ = sender.send(output.clone());
                }

                let (parent_pid, pcb_name, pcb_id) = if let Some(pcb) =
                    self.process_table.get_mut(&pid)
                {
                    pcb.registers
                        .temp_vars
                        .insert("final_output".to_string(), output.clone());

                    // CORE-154: If this supervisor still has pending workers, suspend it.
                    let has_pending_workers = self
                        .worker_tracker
                        .get(&pid)
                        .map(|s| !s.is_empty())
                        .unwrap_or(false);

                    if has_pending_workers {
                        pcb.state = ProcessState::WaitingWorkers;
                        warn!(pid = %pid, "CORE-154: Supervisor suspended — workers still pending.");
                    } else {
                        pcb.state = ProcessState::Completed;
                    }

                    let _ = self.persistence.save_pcb(pcb).await;
                    (
                        pcb.parent_pid.clone(),
                        pcb.process_name.clone(),
                        pcb.pid.clone(),
                    )
                } else {
                    (None, String::new(), String::new())
                };

                // CORE-154: Worker completion — update tracker, store report, maybe resume supervisor.
                if let Some(ppid) = &parent_pid {
                    let report = format!("[WORKER: {} | PID: {}]\n{}", pcb_name, pcb_id, output);

                    if let Some(parent_pcb) = self.process_table.get_mut(ppid) {
                        parent_pcb
                            .registers
                            .temp_vars
                            .insert(format!("worker_report_{}", pcb_id), report);
                    }

                    let all_done = if let Some(workers) = self.worker_tracker.get_mut(ppid) {
                        workers.remove(&pcb_id);
                        workers.is_empty()
                    } else {
                        false
                    };

                    if all_done {
                        self.worker_tracker.remove(ppid);
                        let ppid_owned = ppid.clone();
                        self.resume_supervisor(&ppid_owned);
                        info!(supervisor = %ppid_owned, worker = %pcb_id, "CORE-154: All workers done — supervisor re-queued for synthesis.");
                    } else {
                        info!(worker = %pcb_id, supervisor = %ppid, "CORE-154: Worker report stored. Supervisor still waiting.");
                    }
                }

                {
                    let mut lock = self.graph_manager.write().await;
                    let _ = lock.handle_result(NodeResult {
                        node_id: pid.clone(),
                        output,
                        status: DagNodeStatus::Completed,
                    });
                }

                let new_pcbs = {
                    let mut lock = self.graph_manager.write().await;
                    lock.tick()
                };

                for mut pcb in new_pcbs {
                    self.persistence
                        .save_pcb(&pcb)
                        .await
                        .context("Failed to persist DAG next-ready task (local)")?;
                    pcb.state = ProcessState::Ready;
                    self.process_table.insert(pcb.pid.clone(), pcb.clone());
                    self.ready_queue.push(PcbByPriority(pcb));
                }

                self.current_running.retain(|id| id != &pid);
            }
            SchedulerEvent::PreemptCurrent => {
                if let Some(pid) = self.current_running.pop() {
                    warn!(pid = %pid, "Preemption triggered. Cancelling current process.");
                    if let Some(pcb) = self.process_table.get_mut(&pid) {
                        pcb.cancel_token.cancel();
                        pcb.state = ProcessState::Preempted;
                        let _ = self.persistence.save_pcb(pcb).await;
                    }
                }
            }
            SchedulerEvent::TerminateProcess(pid) => {
                info!(pid = %pid, "Manual process termination.");
                self.process_table.remove(&pid);
                self.waiting_queue.remove(&pid);
                self.current_running.retain(|id| id != &pid);
            }
            // CORE-092: HAL Runner watchdog — limpia estado huerfano si el canal cierra
            SchedulerEvent::HalRunnerDied { reason } => {
                warn!("CORE-092: HAL Runner terminated: {}", reason);
                let pids_to_clean: Vec<String> = self.current_running.drain(..).collect();
                for pid in pids_to_clean {
                    error!(
                        pid = %pid,
                        reason = %reason,
                        "Cleaning up orphaned Running process after HAL Runner death"
                    );
                    if let Some(pcb) = self.process_table.get_mut(&pid) {
                        pcb.state = ProcessState::Failed;
                        pcb.registers
                            .temp_vars
                            .insert("failure_reason".to_string(), reason.clone());
                        if let Err(e) = self.persistence.save_pcb(pcb).await {
                            error!(pid = %pid, error = %e, "Failed to persist Failed state after HAL death");
                        }
                    }
                    // El event_broker vive en AppState (ank-http), no en el scheduler.
                    // El WebSocket detectará la desconexión cuando el broadcast channel
                    // ya no reciba eventos y el timeout de inactividad expire.
                    // No se requiere acción adicional aquí — el estado del PCB es suficiente
                    // para que la próxima consulta devuelva state=Failed.
                    info!(pid = %pid, "Scheduler ready to accept new tasks after HAL Runner recovery.");
                }
            }
            SchedulerEvent::ListProcesses(reply_channel) => {
                let processes = self.process_table.values().cloned().collect();
                let _ = reply_channel.send(processes);
            }
            SchedulerEvent::GetStats(reply_tx) => {
                let total_processes = self.process_table.len() as u32;
                let active_workers = self
                    .process_table
                    .values()
                    .filter(|p| {
                        p.role == ProcessRole::Worker
                            && matches!(
                                p.state,
                                ProcessState::Running
                                    | ProcessState::Ready
                                    | ProcessState::WaitingSyscall
                            )
                    })
                    .count() as u32;
                let _ = reply_tx.send(SchedulerStats {
                    total_processes,
                    active_workers,
                });
            }
        }
        Ok(())
    }

    /// CORE-154: Re-queue a supervisor after all its workers have completed.
    /// Injects aggregated worker reports into the supervisor's context so the
    /// next inference cycle can synthesize a final response.
    fn resume_supervisor(&mut self, supervisor_pid: &str) {
        let Some(supervisor) = self.process_table.get_mut(supervisor_pid) else {
            return;
        };

        if supervisor.state != ProcessState::WaitingWorkers {
            return;
        }

        // Aggregate all worker reports stored in temp_vars
        let reports: String = supervisor
            .registers
            .temp_vars
            .iter()
            .filter(|(k, _)| k.starts_with("worker_report_"))
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let original_task = supervisor.memory_pointers.l1_instruction.clone();
        supervisor
            .inlined_context
            .insert("worker_reports".to_string(), reports);
        supervisor
            .inlined_context
            .insert("original_task".to_string(), original_task);
        supervisor.memory_pointers.l1_instruction =
            "Todos tus sub-agentes completaron. Revisá los reportes en el contexto \
             'worker_reports' y elaborá una respuesta final consolidada para el usuario."
                .to_string();
        supervisor.state = ProcessState::Ready;

        let supervisor_clone = supervisor.clone();
        self.ready_queue.push(PcbByPriority(supervisor_clone));
    }

    async fn reconcile(&mut self) -> anyhow::Result<()> {
        let max_concurrency = 4; // CORE-154: Permitir hasta 4 agentes paralelos

        while self.current_running.len() < max_concurrency && !self.ready_queue.is_empty() {
            if let Some(PcbByPriority(pcb)) = self.ready_queue.pop() {
                if pcb.model_pref.is_complex() {
                    if let Some(swarm) = &self.swarm_manager {
                        let nodes = swarm.active_nodes.read().await;
                        let target_node = nodes
                            .values()
                            .find(|n| n.hardware_tier > 1 && n.status == NodeStatus::Ready);

                        if let Some(node) = target_node {
                            info!(pid = %pcb.pid, target = %node.node_id, "High complexity detected. Delegating to Swarm.");

                            let client = self.swarm_client.clone();
                            let node_ip = node.ip_address.clone();
                            let node_port = node.grpc_port;
                            let recovery_tx = self.internal_tx.clone();
                            let swarm_mgr_ref = swarm.clone();

                            tokio::spawn(async move {
                                let event_tx = if let Some(tx) = &recovery_tx {
                                    tx.clone()
                                } else {
                                    let (dummy_tx, _) = mpsc::channel(1);
                                    dummy_tx
                                };

                                if let Err(e) = client
                                    .teleport(&node_ip, node_port, pcb.clone(), event_tx)
                                    .await
                                {
                                    error!(pid = %pcb.pid, error = %e, "Teleportation failed. Initiating Fallback.");

                                    {
                                        let mut nodes = swarm_mgr_ref.active_nodes.write().await;
                                        if let Some(meta) =
                                            nodes.values_mut().find(|n| n.ip_address == node_ip)
                                        {
                                            meta.status = NodeStatus::Suspect;
                                        }
                                    }

                                    if let Some(tx) = recovery_tx {
                                        let _ = tx
                                            .send(SchedulerEvent::DispatchLocal(Box::new(pcb)))
                                            .await;
                                    }
                                }
                            });
                            return Ok(());
                        }
                    }
                }

                self.current_running.push(pcb.pid.clone());
                let pcb_to_run = pcb.clone();
                self.process_table.insert(pcb.pid.clone(), pcb);

                if let Some(tx) = &self.execution_tx {
                    info!(pid = %pcb_to_run.pid, "Execution trigger sent to local runner.");
                    let pid = pcb_to_run.pid.clone();
                    if let Err(e) = tx.try_send(Box::new(pcb_to_run)) {
                        error!(pid = %pid, error = %e, "Failed to dispatch to local runner.");
                        self.current_running.retain(|id| id != &pid);
                        return Err(anyhow::anyhow!("Local runner dispatch failed: {}", e));
                    }
                }
            }
        }
        Ok(())
    }
}

pub type SharedScheduler = Arc<RwLock<CognitiveScheduler>>;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[tokio::test]
    async fn test_priority_scheduling() -> anyhow::Result<()> {
        let mut scheduler = CognitiveScheduler::new(Arc::new(persistence::MockPersistor));

        let p10 = PCB::new("task-high".into(), 10, "mock".into());
        let p5a = PCB::new("task-low-1".into(), 5, "mock".into());
        let p5b = PCB::new("task-low-2".into(), 5, "mock".into());

        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p5a)))
            .await?;
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p10)))
            .await?;
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p5b)))
            .await?;

        let first = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (first)")?;
        assert_eq!(
            first.0.process_name, "task-high",
            "Prioridad 10 debe salir primero"
        );

        let second = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (second)")?;
        assert_eq!(
            second.0.process_name, "task-low-1",
            "FCFS para prioridad 5 (1)"
        );

        let third = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (third)")?;
        assert_eq!(
            third.0.process_name, "task-low-2",
            "FCFS para prioridad 5 (2)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_supervisor_worker_pattern() -> anyhow::Result<()> {
        let mut scheduler = CognitiveScheduler::new(Arc::new(persistence::MockPersistor));

        // Schedule supervisor
        let supervisor = PCB::new("Supervisor".into(), 10, "orchestrate task".into());
        let supervisor_pid = supervisor.pid.clone();
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(supervisor)))
            .await?;

        // Schedule worker with parent_pid pointing to supervisor
        let mut worker = PCB::new("Worker-Programmer".into(), 9, "write code".into());
        worker.parent_pid = Some(supervisor_pid.clone());
        let worker_pid = worker.pid.clone();
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(worker)))
            .await?;

        // Worker must be tracked under supervisor
        assert!(
            scheduler.worker_tracker.contains_key(&supervisor_pid),
            "Supervisor must be in tracker"
        );
        assert!(
            scheduler.worker_tracker[&supervisor_pid].contains(&worker_pid),
            "Worker must be tracked"
        );

        // Supervisor's role should be updated to Supervisor
        let sup_role = scheduler.process_table[&supervisor_pid].role;
        assert_eq!(sup_role, crate::pcb::ProcessRole::Supervisor);

        // Supervisor completes its own inference — still has a pending worker
        scheduler
            .handle_event(SchedulerEvent::ProcessCompleted {
                pid: supervisor_pid.clone(),
                output: "I'm waiting for workers".into(),
            })
            .await?;

        // Supervisor must be suspended, not Completed
        let sup_state = &scheduler.process_table[&supervisor_pid].state;
        assert_eq!(
            *sup_state,
            crate::pcb::ProcessState::WaitingWorkers,
            "Supervisor must wait for workers"
        );

        // Worker completes
        scheduler
            .handle_event(SchedulerEvent::ProcessCompleted {
                pid: worker_pid.clone(),
                output: "fn hello() {}".into(),
            })
            .await?;

        // Tracker should be cleared for this supervisor
        assert!(
            !scheduler.worker_tracker.contains_key(&supervisor_pid),
            "Tracker must be empty after last worker"
        );

        // Supervisor must be re-queued as Ready for synthesis
        let sup_state = &scheduler.process_table[&supervisor_pid].state;
        assert_eq!(
            *sup_state,
            crate::pcb::ProcessState::Ready,
            "Supervisor must be Ready for synthesis"
        );

        // Worker report must be in supervisor's context
        assert!(
            scheduler.process_table[&supervisor_pid]
                .inlined_context
                .contains_key("worker_reports"),
            "Worker reports must be injected into supervisor context"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_hal_runner_died_cleans_current_running() -> anyhow::Result<()> {
        let mut scheduler = CognitiveScheduler::new(Arc::new(persistence::MockPersistor));

        // Simular un proceso en ejecución
        let pcb = PCB::new("running-task".into(), 5, "mock".into());
        let pid = pcb.pid.clone();
        scheduler.process_table.insert(pid.clone(), pcb);
        scheduler.current_running.push(pid.clone());

        // Disparar HalRunnerDied
        scheduler
            .handle_event(SchedulerEvent::HalRunnerDied {
                reason: "test: channel closed".to_string(),
            })
            .await?;

        // current_running debe estar limpio
        assert!(
            scheduler.current_running.is_empty(),
            "current_running debe estar vacío después de HalRunnerDied"
        );

        // El PCB debe estar marcado como Failed
        let pcb = scheduler
            .process_table
            .get(&pid)
            .context("PCB debe existir")?;
        assert_eq!(
            pcb.state,
            crate::pcb::ProcessState::Failed,
            "PCB debe estar en estado Failed"
        );

        Ok(())
    }
}
