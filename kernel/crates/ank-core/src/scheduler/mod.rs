pub mod compiler;
pub mod graph;
pub mod persistence;

use crate::dag::{DagNodeStatus, ExecutionGraph, GraphManager, NodeResult};
use crate::pcb::{ProcessState, PCB};
use crate::scheduler::persistence::StatePersistor;
use crate::swarm::client::SwarmClient;
use crate::swarm::{NodeStatus, SwarmManager};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, instrument, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelPreference {
    LocalOnly,
    CloudOnly,
    HybridSmart,
}

pub type SharedPCB = Arc<RwLock<PCB>>;

impl ModelPreference {
    /// Determina si la tarea es lo suficientemente compleja como para requerir
    /// un Hardware Tier superior (> 1).
    pub fn is_complex(&self) -> bool {
        match self {
            ModelPreference::CloudOnly => true,
            ModelPreference::HybridSmart => true, // Por defecto, Hybrid intenta lo mejor
            ModelPreference::LocalOnly => false,
        }
    }
}

impl PCB {
    pub fn mock(_pid: &str, priority: u32) -> Self {
        Self::new("MockTask".to_string(), priority, "mock prompt".to_string())
    }
}

/// --- EVENT BUS (Canales MPSC) ---
#[derive(Debug)]
pub enum SchedulerEvent {
    ScheduleTask(Box<PCB>),
    /// Like ScheduleTask but replies with the confirmed PID (which may differ from the
    /// incoming PCB's PID if a collision is resolved) via the oneshot sender.
    ScheduleTaskConfirmed(Box<PCB>, tokio::sync::oneshot::Sender<String>),
    DispatchLocal(Box<PCB>), // Nuevo: Re-encolado forzado para ejecución local
    SyscallCompleted {
        pid: String,
        result: String,
    },
    RemoteEvent(String, ank_proto::v1::TaskEvent), // Nuevo: Evento interceptado de un nodo remoto
    RegisterGraph(Box<ExecutionGraph>),            // Nuevo: Registro y validación de un S-DAG
    ProcessCompleted {
        pid: String,
        output: String,
    },
    PreemptCurrent,
    TerminateProcess(String),
    ListProcesses(tokio::sync::oneshot::Sender<Vec<PCB>>),
}

/// --- COGNITIVE SCHEDULER ---
pub struct CognitiveScheduler {
    pub ready_queue: BinaryHeap<PCB>,
    pub waiting_queue: HashMap<String, PCB>,
    pub process_table: HashMap<String, PCB>,
    pub current_running: Option<String>,
    pub last_activity: DateTime<Utc>,

    // Infraestructura del Enjambre
    pub swarm_manager: Option<Arc<SwarmManager>>,
    pub swarm_client: Arc<SwarmClient>,
    // Canal para auto-enviarse eventos de recovery
    pub internal_tx: Option<mpsc::Sender<SchedulerEvent>>,
    // Grafo de ejecución S-DAG
    pub graph_manager: Arc<RwLock<GraphManager>>,
    // Persistencia de estado (SQLCipher)
    pub persistence: Arc<dyn StatePersistor>,
    // Canal para notificar el inicio de ejecución local (ANK-2505)
    pub execution_tx: Option<mpsc::Sender<Box<PCB>>>,
}

impl CognitiveScheduler {
    pub fn new(persistence: Arc<dyn StatePersistor>) -> Self {
        Self {
            ready_queue: BinaryHeap::new(),
            waiting_queue: HashMap::new(),
            process_table: HashMap::new(),
            current_running: None,
            last_activity: Utc::now(),
            swarm_manager: None,
            swarm_client: Arc::new(SwarmClient),
            internal_tx: None,
            graph_manager: Arc::new(RwLock::new(GraphManager::new())),
            persistence,
            execution_tx: None,
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
                // Prioridad 1: Procesar eventos externos
                Some(event) = event_rx.recv() => {
                    use anyhow::Context;
                    self.handle_event(event).await.context("Error handling scheduler event")?;
                }

                // Prioridad 2: Ciclo de despacho (Reconcile)
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    use anyhow::Context;
                    self.reconcile().await.context("Error during state reconciliation")?;
                }

                // Prioridad 3: GC Pasivo
                _ = gc_interval.tick() => {
                    let now = chrono::Utc::now();
                    let five_mins = chrono::Duration::minutes(5);
                    self.process_table.retain(|_, pcb| {
                        let is_finished = matches!(pcb.state, crate::pcb::ProcessState::Completed | crate::pcb::ProcessState::Failed);
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
                info!(
                    "DEBUG_SCHEDULER: Received ScheduleTask. PID: {}",
                    pcb_box.pid
                );
                info!(pid = %pcb_box.pid, prio = pcb_box.priority, "Task queued (ScheduleTask).");
                let mut pcb = *pcb_box;

                // Persistencia atómica antes de cambiar estado a Ready
                self.persistence
                    .save_pcb(&pcb)
                    .await
                    .context("Atomic persistence failed during ScheduleTask")?;

                pcb.state = ProcessState::Ready;
                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                self.ready_queue.push(pcb);
            }
            SchedulerEvent::ScheduleTaskConfirmed(pcb_box, confirm_tx) => {
                info!(pid = %pcb_box.pid, prio = pcb_box.priority, "Task queued (ScheduleTaskConfirmed).");
                let mut pcb = *pcb_box;

                // Persistencia atómica antes de cambiar estado a Ready
                self.persistence
                    .save_pcb(&pcb)
                    .await
                    .context("Atomic persistence failed during ScheduleTaskConfirmed")?;

                let confirmed_pid = pcb.pid.clone();
                pcb.state = ProcessState::Ready;
                self.process_table.insert(pcb.pid.clone(), pcb.clone());
                self.ready_queue.push(pcb);

                // Notify the caller of the confirmed PID (best-effort; ignore send error)
                let _ = confirm_tx.send(confirmed_pid);
            }
            SchedulerEvent::RegisterGraph(graph_box) => {
                let graph = *graph_box;
                let mut lock = self.graph_manager.write().await;
                crate::scheduler::graph::GraphIntegrator::validate_and_register(&mut lock, graph);

                // Disparar primer tick para arrancar nodos iniciales
                let new_pcbs = lock.tick();
                drop(lock); // Soltamos el lock antes de encolar

                for mut pcb in new_pcbs {
                    self.persistence
                        .save_pcb(&pcb)
                        .await
                        .context("Failed to persist initial DAG task")?;
                    pcb.state = ProcessState::Ready;
                    self.process_table.insert(pcb.pid.clone(), pcb.clone());
                    self.ready_queue.push(pcb);
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
                self.ready_queue.push(pcb);
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
                    self.ready_queue.push(pcb);
                }
            }
            SchedulerEvent::RemoteEvent(pid, remote_event) => {
                info!(pid = %pid, "Received remote execution event from Swarm.");
                // Extraemos el payload de Protobuf y disparamos lógica local
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

                            // Anti-Deadlock: Bloqueo super corto para reportar resultado
                            {
                                let mut lock = self.graph_manager.write().await;
                                let _ = lock.handle_result(NodeResult {
                                    node_id: pid.clone(),
                                    output: result,
                                    status: DagNodeStatus::Completed,
                                });
                            }

                            // Extraemos nuevos procesos listos
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
                                self.ready_queue.push(pcb);
                            }
                        }
                        ank_proto::v1::task_event::Payload::Syscall(syscall) => {
                            info!(pid = %pid, "Remote process requires Syscall executing on Host: {}", syscall.name);
                        }
                        _ => {
                            info!(pid = %pid, "Ignored remote status payload.");
                        }
                    }
                }
            }
            SchedulerEvent::ProcessCompleted { pid, output } => {
                info!(pid = %pid, "Process completed locally. Notifying DAG.");
                if let Some(pcb) = self.process_table.get_mut(&pid) {
                    pcb.registers
                        .temp_vars
                        .insert("final_output".to_string(), output.clone());

                    self.persistence
                        .save_pcb(pcb)
                        .await
                        .context("Failed to persist Local Completed state")?;
                    pcb.state = ProcessState::Completed;
                }

                // Anti-Deadlock: Bloqueo super corto
                {
                    let mut lock = self.graph_manager.write().await;
                    let _ = lock.handle_result(NodeResult {
                        node_id: pid.clone(),
                        output,
                        status: DagNodeStatus::Completed,
                    });
                }

                // Anti-Deadlock: Nuevo bloqueo para ver si hay tareas hijas listas
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
                    self.ready_queue.push(pcb);
                }
            }
            SchedulerEvent::PreemptCurrent => {
                if let Some(pid) = self.current_running.take() {
                    warn!(pid = %pid, "Hard preemption triggered. Interrupting ALU.");
                    // FUTURE(ANK-2501): Send interrupt signal to cHAL to cancel ongoing inference
                }
            }
            SchedulerEvent::TerminateProcess(pid) => {
                info!(pid = %pid, "Manual process termination.");
                self.process_table.remove(&pid);
                self.waiting_queue.remove(&pid);
                if self.current_running.as_ref() == Some(&pid) {
                    self.current_running = None;
                }
            }
            SchedulerEvent::ListProcesses(reply_channel) => {
                let processes = self.process_table.values().cloned().collect();
                let _ = reply_channel.send(processes);
            }
        }
        Ok(())
    }

    /// Despacha procesos de la cola de Listos a la "CPU" (ALU/LLM) local o al Swarm si es complejo.
    async fn reconcile(&mut self) -> anyhow::Result<()> {
        if self.current_running.is_none() && !self.ready_queue.is_empty() {
            if let Some(pcb) = self.ready_queue.pop() {
                // LÓGICA DE TELEPORTACIÓN
                if pcb.model_pref.is_complex() {
                    if let Some(swarm) = &self.swarm_manager {
                        let nodes = swarm.active_nodes.read().await;
                        // Buscamos un nodo con Tier > 1 que esté Ready
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

                            // Spawn de Teleportación para no bloquear el bucle del Scheduler
                            tokio::spawn(async move {
                                // Extraemos el canal de eventos de la instancia local si existe,
                                // sino usamos un dummy channel como resiliencia.
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

                                    // Marcar nodo como sospechoso
                                    {
                                        let mut nodes = swarm_mgr_ref.active_nodes.write().await;
                                        if let Some(meta) =
                                            nodes.values_mut().find(|n| n.ip_address == node_ip)
                                        {
                                            meta.status = NodeStatus::Suspect;
                                        }
                                    }

                                    // RESILIENCIA EXTREMA: Devolver el PCB a la cola local
                                    if let Some(tx) = recovery_tx {
                                        let _ = tx
                                            .send(SchedulerEvent::DispatchLocal(Box::new(pcb)))
                                            .await;
                                    }
                                }
                            });
                            return Ok(()); // Despachado al Swarm (asíncronamente)
                        }
                    }
                }

                // FALLBACK O EJECUCIÓN SIMPLE: Despacho local
                self.current_running = Some(pcb.pid.clone());
                let pcb_to_run = pcb.clone();
                self.process_table.insert(pcb.pid.clone(), pcb);

                // Disparar Trigger de Ejecución si el canal está configurado
                if let Some(tx) = &self.execution_tx {
                    info!(pid = %pcb_to_run.pid, "Execution trigger sent to local runner.");
                    let pid = pcb_to_run.pid.clone();
                    if let Err(e) = tx.try_send(Box::new(pcb_to_run)) {
                        error!(pid = %pid, error = %e, "SCHEDULER ERROR: Failed to dispatch to local runner. Queue full or runner dead.");
                        // Rollback: No marcar como running si no se pudo enviar
                        self.current_running = None;
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

        // Inyectamos fuera de orden
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p5a)))
            .await?;
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p10)))
            .await?;
        scheduler
            .handle_event(SchedulerEvent::ScheduleTask(Box::new(p5b)))
            .await?;

        // Verificamos orden de salida
        let first = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (first)")?;
        assert_eq!(
            first.process_name, "task-high",
            "Prioridad 10 debe salir primero"
        );

        let second = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (second)")?;
        assert_eq!(
            second.process_name, "task-low-1",
            "FCFS para prioridad 5 (1)"
        );

        let third = scheduler
            .ready_queue
            .pop()
            .context("Ready queue should not be empty (third)")?;
        assert_eq!(
            third.process_name, "task-low-2",
            "FCFS para prioridad 5 (2)"
        );
        Ok(())
    }
}
