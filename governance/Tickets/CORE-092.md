# CORE-092 — Watchdog del HAL Runner en el Scheduler

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/scheduler/mod.rs` + `kernel/crates/ank-server/src/main.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Estabilidad  
**Estado:** TODO  
**Origen:** REC-004 / big-pickle DEBT-001

---

## Contexto

El `CognitiveScheduler` marca un proceso como `Running` cuando lo despacha via
`execution_tx`. Si el HAL Runner muere inesperadamente (canal cerrado, panic
capturado por Tokio), el scheduler nunca recibe `ProcessCompleted`. El proceso
queda en estado `Running` indefinidamente y el scheduler deja de encolar nuevas
tareas: deadlock silencioso.

---

## Cambios requeridos

1. En `main.rs`, detectar el cierre de `execution_rx` y notificar al scheduler:

   ```rust
   tokio::spawn(async move {
       while let Some(pcb) = execution_rx.recv().await {
           // lógica actual
       }
       // Canal cerrado — notificar al scheduler
       let _ = scheduler_tx.send(SchedulerEvent::HalRunnerDied).await;
   });
   ```

2. Agregar variante `HalRunnerDied` al enum `SchedulerEvent` en `ank-core`.

3. En el handler del scheduler al recibir `HalRunnerDied`:
   - Si hay un proceso en estado `Running`, marcarlo como `Failed` con mensaje
     `"HAL Runner terminated unexpectedly"`.
   - Emitir `TaskEvent::Error` al `event_broker` para ese PID.
   - Limpiar `current_running = None`.

---

## Criterios de aceptación

- [ ] Si `execution_rx` se cierra, el scheduler recibe `HalRunnerDied` y limpia `current_running`
- [ ] Después del evento, el scheduler acepta y encola nuevas tareas normalmente
- [ ] El cliente WebSocket recibe un `TaskEvent::Error` para el PID afectado
- [ ] `cargo build --workspace` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

Ninguna.
