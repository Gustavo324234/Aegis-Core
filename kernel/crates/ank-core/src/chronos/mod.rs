use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::pcb::PCB;
use crate::scheduler::{SchedulerEvent, SharedScheduler};
use crate::vcm::swap::LanceSwapManager;

/// --- CHRONOS DAEMON ---
/// Responsable de la asimilación de memoria a largo plazo y consolidación semántica.
pub struct ChronosDaemon {
    /// Referencia al Scheduler para monitorear inactividad e inyectar tareas.
    scheduler: SharedScheduler,
    /// Referencia al Swap Manager para persistencia semántica (L3).
    #[allow(dead_code)]
    swap_manager: Arc<LanceSwapManager>,
    /// Canal de comunicación para enviar eventos al Scheduler.
    event_tx: mpsc::Sender<SchedulerEvent>,
    /// Tiempo de inactividad necesario para iniciar la consolidación.
    idle_threshold: chrono::Duration,
    /// Tiempo mínimo entre tareas de consolidación para evitar saturación.
    cooldown: chrono::Duration,
    /// Timestamp de la última consolidación ejecutada.
    last_consolidation: Arc<RwLock<DateTime<Utc>>>,
}

impl ChronosDaemon {
    pub fn new(
        scheduler: SharedScheduler,
        swap_manager: Arc<LanceSwapManager>,
        event_tx: mpsc::Sender<SchedulerEvent>,
        idle_minutes: i64,
        cooldown_hours: i64,
    ) -> Self {
        Self {
            scheduler,
            swap_manager,
            event_tx,
            idle_threshold: chrono::Duration::minutes(idle_minutes),
            cooldown: chrono::Duration::hours(cooldown_hours),
            last_consolidation: Arc::new(RwLock::new(Utc::now() - chrono::Duration::days(1))), // Init in the past
        }
    }

    /// Inicia el bucle infinito del demonio en un hilo de background.
    pub fn start(self) {
        tokio::spawn(async move {
            info!("Chronos Daemon started. Monitoring for idle states...");

            loop {
                // Dormir antes de la siguiente revisión (30 segundos por defecto)
                sleep(Duration::from_secs(30)).await;

                if let Err(e) = self.run_step().await {
                    error!("Chronos execution step failed: {}", e);
                }
            }
        });
    }

    /// Determina si el sistema está en estado IDLE (Inactivo).
    /// El estado IDLE se alcanza si no hay tareas en ejecución, colas vacías,
    /// y ha pasado suficiente tiempo desde la última actividad.
    pub async fn check_idle_state(&self) -> bool {
        let scheduler = self.scheduler.read().await;

        // Reglas de Inactividad SRE:
        let alu_free = scheduler.current_running.is_empty();
        let ready_empty = scheduler.ready_queue.is_empty();
        let waiting_empty = scheduler.waiting_queue.is_empty();
        let time_since_activity = Utc::now() - scheduler.last_activity;
        let is_quiet = time_since_activity > self.idle_threshold;

        alu_free && ready_empty && waiting_empty && is_quiet
    }

    /// Construye un PCB de baja prioridad para la tarea de consolidación.
    /// Esta tarea instruye a la ALU a resumir y extraer entidades y hechos personales.
    pub fn build_consolidation_pcb(&self, tenant_id: &str, raw_text: &str) -> PCB {
        let l1_prompt = format!(
            "Analizá los siguientes fragmentos recientes de conversación con el usuario y extraé hechos personales implícitos o explícitos que valga la pena recordar a largo plazo (gustos, horarios, familia, salud, proyectos, preferencias).\n\
            Respondé estrictamente en formato JSON: {{\"summary\": \"...\", \"tags\": [\"...\"], \"facts\": [{{\"category\": \"preferencias|rutina|salud|trabajo|general\", \"fact\": \"...\"}}]}}\n\n\
            CONVERSATION / EVENTS:\n{}",
            raw_text
        );

        // Prioridad 1: Misión de background, se pausa si llega tráfico del usuario.
        let mut pcb = PCB::new("ChronosConsolidator".to_string(), 1, l1_prompt);
        pcb.tenant_id = Some(tenant_id.to_string());
        pcb
    }

    /// Punto de entrada para la ejecución del demonio (Loop lógico).
    pub async fn run_step(&self) -> anyhow::Result<()> {
        // 1. Verificar Cooldown (Prevenir saturación)
        {
            let last = self.last_consolidation.read().await;
            if Utc::now() - *last < self.cooldown {
                return Ok(()); // En periodo de enfriamiento
            }
        }

        // 2. Verificar estado IDLE
        if self.check_idle_state().await {
            info!("Kernel Idle & Cooldown expired. Chronos inyecting consolidation task...");

            // Actualizar timestamp de consolidación antes de inyectar
            {
                let mut last = self.last_consolidation.write().await;
                *last = Utc::now();
            }

            let (tenant_id, raw_text) = Self::load_latest_tenant_chat_log();
            let pcb = self.build_consolidation_pcb(&tenant_id, &raw_text);

            self.event_tx
                .send(SchedulerEvent::ScheduleTask(Box::new(pcb)))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send consolidation task: {}", e))?;
        }
        Ok(())
    }

    /// Lee el historial real más reciente (`users/<tenant>/workspace/chat_history.log`)
    /// para alimentar la consolidación nocturna de Chronos (`CORE-376`).
    fn load_latest_tenant_chat_log() -> (String, String) {
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let users_dir = std::path::Path::new(&base_dir).join("users");
        let mut newest: Option<(std::time::SystemTime, String, std::path::PathBuf)> = None;

        if let Ok(entries) = std::fs::read_dir(&users_dir) {
            for entry in entries.flatten() {
                let tenant = entry.file_name().to_string_lossy().to_string();
                if tenant.starts_with("test_") {
                    continue;
                }
                let candidate = entry.path().join("workspace").join("chat_history.log");
                if let Ok(meta) = std::fs::metadata(&candidate) {
                    if meta.len() > 0 {
                        let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        if newest.as_ref().map(|(t, _, _)| mtime > *t).unwrap_or(true) {
                            newest = Some((mtime, tenant, candidate));
                        }
                    }
                }
            }
        }

        if let Some((_, tenant, path)) = newest {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().rev().take(40).collect();
                let recent = lines.into_iter().rev().collect::<Vec<&str>>().join("\n");
                if !recent.trim().is_empty() {
                    return (tenant, recent);
                }
            }
        }

        (
            "default".to_string(),
            "ACTIVITY LOG: No recent user conversation turns pending consolidation.".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{persistence, CognitiveScheduler};
    use anyhow::Context;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_idle_detection_logic() {
        let scheduler = Arc::new(RwLock::new(CognitiveScheduler::new(Arc::new(
            persistence::MockPersistor,
        ))));
        let swap = Arc::new(LanceSwapManager::new("./tmp/test_swap"));
        let (tx, _rx) = mpsc::channel(32);

        // Umbral de 1 minuto, cooldown 1 hora
        let chronos = ChronosDaemon::new(scheduler.clone(), swap, tx, 1, 1);

        // Caso 1: Nuevo scheduler (recién creado, last_activity es now())
        // No debería ser idle porque 0 mins < 1 min umbral
        assert!(!chronos.check_idle_state().await);

        // Caso 2: Forzamos la inactividad moviendo el reloj hacia atrás
        {
            let mut sched_w = scheduler.write().await;
            sched_w.last_activity = Utc::now() - chrono::Duration::minutes(2);
        }
        assert!(chronos.check_idle_state().await);

        // Caso 3: Inyectamos una tarea (deja de ser idle)
        {
            let mut sched_w = scheduler.write().await;
            sched_w.current_running.push("proc_1".to_string());
        }
        assert!(!chronos.check_idle_state().await);
    }

    #[tokio::test]
    async fn test_chronos_scheduling_injection() -> anyhow::Result<()> {
        let scheduler = Arc::new(RwLock::new(CognitiveScheduler::new(Arc::new(
            persistence::MockPersistor,
        ))));
        let swap = Arc::new(LanceSwapManager::new("./tmp/test_swap_inj"));
        let (tx, mut rx) = mpsc::channel(32);

        let chronos = ChronosDaemon::new(scheduler.clone(), swap, tx, 0, 0); // 0 threshold for immediate action

        // Obligamos a last_activity a estar un poco en el pasado para asegurar IDLE
        {
            let mut sched_w = scheduler.write().await;
            sched_w.last_activity = Utc::now() - chrono::Duration::seconds(1);
        }

        chronos
            .run_step()
            .await
            .context("Run step should succeed")?;

        // Verificamos que se envió el evento de ScheduleTask
        let event = rx.try_recv().context("Should have received an event")?;
        if let SchedulerEvent::ScheduleTask(pcb) = event {
            assert_eq!(pcb.process_name, "ChronosConsolidator");
            assert_eq!(pcb.priority, 1);
            assert_eq!(pcb.tenant_id.as_deref(), Some("default"));
        } else {
            anyhow::bail!("Received wrong event type");
        }
        Ok(())
    }

    #[test]
    fn test_core_376_build_consolidation_pcb_with_tenant() {
        let scheduler = Arc::new(RwLock::new(CognitiveScheduler::new(Arc::new(
            persistence::MockPersistor,
        ))));
        let swap = Arc::new(LanceSwapManager::new("./tmp/test_swap_pcb"));
        let (tx, _rx) = mpsc::channel(32);
        let chronos = ChronosDaemon::new(scheduler, swap, tx, 1, 1);

        let pcb = chronos.build_consolidation_pcb("alex", "Usuario dijo que le gusta el café.");
        assert_eq!(pcb.tenant_id.as_deref(), Some("alex"));
        assert!(pcb.memory_pointers.l1_instruction.contains("facts"));
        assert!(pcb
            .memory_pointers
            .l1_instruction
            .contains("Usuario dijo que le gusta el café."));
    }
}
