# CORE-244 — HAL Runner: enviar StatusUpdate al event_broker en el path de error

**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Épica:** Bugs de estabilidad (smoke test)  
**Asignado a:** Kernel Engineer  
**Componente:** `kernel/crates/ank-server/src/main.rs`

---

## Contexto

Durante el smoke test del 2026-05-03, el usuario enviaba mensajes pero no recibía ninguna respuesta en la UI — ni el output del modelo ni el mensaje de error.

El kernel procesaba los requests correctamente (logs muestran `ProcessCompleted`), pero la UI quedaba en silencio.

## Causa raíz

En el HAL Runner (`main.rs`), el path de **éxito** envía dos cosas al `event_tx` (broadcast del `event_broker`):
1. Los tokens de output uno a uno.
2. Un `StatusUpdate { state: 4 }` (COMPLETED) al final, que hace `break` en el `stream_with_receiver` del WebSocket.

El path de **error** solo notifica al scheduler pero **nunca envía el `StatusUpdate`** al `event_tx`:

```rust
// main.rs — HAL Runner, path de error
Err(e) => {
    error!(pid = %pid, "HAL Runner failed: {}", e);
    let _ = scheduler_tx_runner
        .send(SchedulerEvent::ProcessCompleted {
            pid,
            output: format!("error: {}", e),
        })
        .await;
    // ← FALTA: enviar StatusUpdate al event_tx
    // ← FALTA: enviar el mensaje de error como Output al event_tx
}
```

Como consecuencia, `stream_with_receiver()` en `ws/chat.rs` queda bloqueado esperando eventos que nunca llegan. El usuario no ve nada — ni respuesta ni error.

## Cambios requeridos

En `kernel/crates/ank-server/src/main.rs`, en el bloque `Err(e)` del HAL Runner, agregar:

```rust
Err(e) => {
    error!(pid = %pid, "HAL Runner failed: {}", e);

    // 1. Notificar al scheduler
    let _ = scheduler_tx_runner
        .send(SchedulerEvent::ProcessCompleted {
            pid: pid.clone(),
            output: format!("error: {}", e),
        })
        .await;

    // 2. Enviar el mensaje de error al WebSocket como Output
    let error_msg = format!("Lo siento, ocurrió un error al procesar tu mensaje.");
    let _ = event_tx.send(ank_proto::v1::TaskEvent {
        pid: pid.clone(),
        timestamp: None,
        payload: Some(ank_proto::v1::task_event::Payload::Output(error_msg)),
    });

    // 3. Enviar StatusUpdate COMPLETED para desbloquear stream_with_receiver
    let _ = event_tx.send(ank_proto::v1::TaskEvent {
        pid: pid.clone(),
        timestamp: None,
        payload: Some(ank_proto::v1::task_event::Payload::StatusUpdate(
            Box::new(ank_proto::v1::Pcb {
                state: 4, // STATE_COMPLETED — el stream_with_receiver hace break aquí
                ..Default::default()
            }),
        )),
    });
}
```

**Nota:** El mensaje al usuario debe ser genérico y amigable. El error técnico ya se loguea en el servidor — no hace falta exponerlo en la UI.

## Criterios de aceptación

- [ ] Cuando el HAL falla (cualquier error de `route_and_execute`), el WebSocket recibe un mensaje de error visible en la UI.
- [ ] El `stream_with_receiver` termina inmediatamente después del error — no queda colgado.
- [ ] El mensaje de error en la UI es genérico y amigable (no expone stack traces ni detalles internos).
- [ ] El log de error en el servidor sigue registrando el error técnico completo.
- [ ] `cargo build` sin errores de compilación.

## Dependencias

Ninguna.

---

## Resolución (2026-06-12)
Verificado 2026-06-12: ya implementado en main — el path de error del HAL Runner (ank-server/main.rs) envía Output + StatusUpdate{state:4} al event_tx. Reconciliación de governance.
