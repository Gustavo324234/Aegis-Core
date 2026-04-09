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
        let alu_free = scheduler.current_running.is_none();
        let ready_empty = scheduler.ready_queue.is_empty();
        let waiting_empty = scheduler.waiting_queue.is_empty();
        let time_since_activity = Utc::now() - scheduler.last_activity;
        let is_quiet = time_since_activity > self.idle_threshold;

        alu_free && ready_empty && waiting_empty && is_quiet
    }

    /// Construye un PCB de baja prioridad para la tarea de consolidación.
    /// Esta tarea instruye a la ALU a resumir y extraer entidades de los logs crudos.
    pub fn build_consolidation_pcb(&self, raw_text: &str) -> PCB {
        let l1_prompt = format!(
            "CONTEXT CONSOLIDATION MISSION:\n\
            Resume los siguientes eventos recientes en un párrafo denso y extrae entidades clave.\n\
            Responde estrictamente en formato JSON: {{\"summary\": \"...\", \"tags\": [...]}}\n\n\
            EVENTS:\n{}",
            raw_text
        );

        // Prioridad 1: Misión de background, se pausa si llega tráfico del usuario.
        PCB::new("ChronosConsolidator".to_string(), 1, l1_prompt)
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

            // Simulación de texto de prueba para asimilación
            let test_raw_text =
                "ACTIVITY LOG: User modified main.rs. Syscall weather executed. PC was restarted.";
            let pcb = self.build_consolidation_pcb(test_raw_text);

            self.event_tx
                .send(SchedulerEvent::ScheduleTask(Box::new(pcb)))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send consolidation task: {}", e))?;
        }
        Ok(())
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
            sched_w.current_running = Some("proc_1".to_string());
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
        } else {
            anyhow::bail!("Received wrong event type");
        }
        Ok(())
    }
}
