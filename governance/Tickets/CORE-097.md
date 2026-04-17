# CORE-097 — Preemption Real en CognitiveScheduler via CancellationToken

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/scheduler/mod.rs` + `kernel/crates/ank-server/src/main.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P2 — Fairness  
**Estado:** TODO  
**Origen:** REC-009 / big-pickle DEBT-002

---

## Contexto

`handle_event(PreemptCurrent)` está marcado como TODO. El scheduler tiene un sistema
de prioridades pero sin preemption real: una tarea de prioridad baja puede bloquear
indefinidamente a una de prioridad alta que llega mientras la primera se ejecuta.
El HAL Runner corre cada tarea hasta completarla sin punto de cancelación.

---

## Cambios requeridos

1. Agregar un `CancellationToken` (de `tokio_util::sync`) al `AppState` o como campo
   del `PCB`:

   ```rust
   use tokio_util::sync::CancellationToken;

   pub struct PCB {
       // ... campos existentes
       pub cancel_token: CancellationToken,
   }
   ```

2. En el HAL Runner, verificar el token entre chunks del stream SSE:

   ```rust
   while let Some(chunk) = stream.next().await {
       if pcb.cancel_token.is_cancelled() {
           tracing::info!("PCB {} preempted", pcb.pid);
           break;
       }
       // procesar chunk
   }
   ```

3. En el scheduler, al manejar `PreemptCurrent`:
   - Llamar `current_pcb.cancel_token.cancel()`
   - Marcar el proceso actual como `Preempted` (nuevo estado, no `Failed`)
   - Reencolar la tarea preemptada al frente de `ready_queue` si la prioridad
     original lo justifica, o descartarla según la política definida

4. Agregar variante `Preempted` al enum de estados del PCB.

5. Emitir `TaskEvent::Preempted` al `event_broker` para que la UI pueda
   notificar al usuario que su tarea fue interrumpida.

---

## Criterios de aceptación

- [ ] `handle_event(PreemptCurrent)` ya no es un TODO — cancela la inferencia en curso
- [ ] El HAL Runner verifica el `CancellationToken` en cada iteración del stream
- [ ] El proceso preemptado emite `TaskEvent::Preempted` al event_broker
- [ ] `cargo build --workspace` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

- `tokio-util` debe estar en el workspace (verificar que ya está como dependencia transitiva
  o agregar explícitamente).
