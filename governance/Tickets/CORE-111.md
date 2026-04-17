# CORE-111 — Watchdog para `current_running` cuando el HAL Runner muere

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🔴 ALTA — Estabilidad
**Estado:** TODO
**Origen:** REC-004 / Auditoría multi-modelo 2026-04-16

---

## Contexto

El `CognitiveScheduler` mantiene un campo `current_running: Option<Pid>` que
indica qué proceso está en ejecución. Cuando el HAL Runner (goroutine en
`main.rs`) completa una tarea, envía `SchedulerEvent::ProcessCompleted(pid)` y
el scheduler limpia ese campo.

El problema: si el HAL Runner muere por un panic inesperado o si el canal
`execution_tx` se cierra por cualquier razón, el scheduler nunca recibe
`ProcessCompleted`. El proceso queda marcado como `running` indefinidamente y
el scheduler no despacha ninguna tarea nueva, ya que su lógica de reconcile
verifica `current_running.is_none()` antes de encolar.

**Resultado observable:** el sistema acepta nuevas peticiones vía WebSocket pero
nunca las procesa. El usuario ve "thinking..." sin respuesta, sin error.

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/scheduler/mod.rs`
**Archivo:** `kernel/crates/ank-server/src/main.rs`

### Estrategia: Watchdog via `CancellationToken` + detección de canal cerrado

**1. En `main.rs` — detectar muerte del HAL Runner:**

```rust
let hal_runner_handle = tokio::spawn(async move {
    while let Some(pcb) = execution_rx.recv().await {
        // ... lógica actual del HAL Runner ...
    }
    // Si llegamos aquí, execution_rx se cerró — notificar al scheduler
    warn!("HAL Runner: execution_rx closed, signaling scheduler");
    let _ = scheduler_tx_clone.send(SchedulerEvent::HalRunnerDied).await;
});
```

**2. En `ank-core/src/scheduler/mod.rs` — agregar el evento y manejarlo:**

```rust
pub enum SchedulerEvent {
    // ... eventos existentes ...
    HalRunnerDied,  // Nuevo: el HAL Runner terminó inesperadamente
}

// En handle_event:
SchedulerEvent::HalRunnerDied => {
    if let Some(pid) = self.current_running.take() {
        warn!("HAL Runner died while processing PID {}. Marking as failed.", pid);
        if let Some(pcb) = self.process_table.get_mut(&pid) {
            pcb.write().await.state = ProcessState::Failed;
        }
        // Opcional: reencolar si la tarea es reintentable
    }
    error!("HAL Runner is dead. Scheduler paused until restart.");
}
```

**3. Alternativa más simple — timeout en reconcile:**

Si el proceso lleva más de N segundos en estado `Running` sin recibir
`ProcessCompleted`, marcarlo como fallido:

```rust
fn reconcile(&mut self) {
    if let Some(pid) = &self.current_running {
        if let Some(pcb) = self.process_table.get(pid) {
            let pcb = pcb.read_sync(); // o gestionar async
            if pcb.state == ProcessState::Running {
                let elapsed = pcb.started_at.elapsed();
                if elapsed > Duration::from_secs(300) { // 5 minutos
                    warn!("PID {} stuck in Running for {:?}, marking failed", pid, elapsed);
                    self.current_running = None;
                }
            }
        }
    }
}
```

**Implementar primero el timeout (más simple y defensivo), luego el evento `HalRunnerDied` si el Kernel Engineer lo considera apropiado.**

## Criterios de aceptación

- [ ] Si el HAL Runner termina inesperadamente, el scheduler no queda bloqueado
  indefinidamente con `current_running = Some(pid)`
- [ ] Tareas subsiguientes son despachadas correctamente después del timeout/recovery
- [ ] El estado del proceso fallido queda registrado en el log con nivel `warn`/`error`
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] No regresión en los tests del scheduler

## Dependencias

Ninguna bloqueante. No depende de CORE-110.
