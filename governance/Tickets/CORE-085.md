# CORE-085 — Fix: Scheduler no conecta al HAL — `execution_tx` siempre es `None`

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

El `CognitiveScheduler` despacha tareas a través de `execution_tx`:

```rust
// En reconcile():
if let Some(tx) = &self.execution_tx {
    info!(pid = %pcb_to_run.pid, "Execution trigger sent to local runner.");
    tx.try_send(Box::new(pcb_to_run))?;
}
// Si execution_tx es None → el PCB se saca de la queue pero nunca se ejecuta
```

En `ank-server/src/main.rs`, el scheduler se crea y arranca así:

```rust
let scheduler = CognitiveScheduler::new(Arc::clone(&persistence));
tokio::spawn(async move {
    scheduler.start(scheduler_rx, scheduler_tx_clone).await
});
```

**`execution_tx` nunca se setea.** `CognitiveScheduler::new()` lo inicializa
como `None`, y ningún código en `main.rs` llama a `scheduler.execution_tx = Some(...)`.

**Consecuencia:** Cuando el WebSocket de chat recibe un prompt y lo encola al
scheduler via `SchedulerEvent::ScheduleTaskConfirmed`, el scheduler:
1. ✅ Acepta el PCB
2. ✅ Lo pone en `ready_queue`
3. ✅ Lo saca con `pop()` en `reconcile()`
4. ✅ Setea `current_running = Some(pid)`
5. ❌ No lo envía a ningún runner porque `execution_tx` es `None`
6. ❌ El proceso queda en `process_table` con state `Running` para siempre
7. ❌ El `event_broker` nunca recibe eventos → el WebSocket queda colgado

**Esto significa que el chat nunca genera respuestas.** El flujo completo
de inferencia está desconectado del scheduler.

---

## Arquitectura correcta

El runner de ejecución debe:
1. Recibir PCBs del scheduler via `execution_tx`
2. Llamar a `hal.route_and_execute(shared_pcb)`
3. Streamear el resultado al `event_broker` (que el WS escucha)
4. Notificar al scheduler con `SchedulerEvent::ProcessCompleted`

Este patrón existía en los repos legacy — se perdió durante la consolidación.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-server/src/main.rs`

### 1. Crear el canal de ejecución

```rust
let (execution_tx, mut execution_rx) = mpsc::channel::<Box<PCB>>(64);
```

### 2. Setear `execution_tx` en el scheduler antes de arrancarlo

```rust
let mut scheduler = CognitiveScheduler::new(Arc::clone(&persistence));
scheduler.execution_tx = Some(execution_tx);
tokio::spawn(async move {
    scheduler.start(scheduler_rx, scheduler_tx_clone).await
});
```

### 3. Spawn del runner de ejecución

```rust
let hal_runner = Arc::clone(&hal);
let event_broker_runner = Arc::clone(&event_broker);
let scheduler_tx_runner = scheduler_tx.clone();

tokio::spawn(async move {
    while let Some(pcb) = execution_rx.recv().await {
        let pid = pcb.pid.clone();
        let tenant_id = pcb.tenant_id.clone().unwrap_or_default();
        let shared_pcb = Arc::new(RwLock::new(*pcb));

        // Obtener o crear el sender del event_broker para este PID
        let event_tx = {
            let mut broker = event_broker_runner.write().await;
            broker.entry(pid.clone())
                .or_insert_with(|| {
                    let (tx, _) = tokio::sync::broadcast::channel(256);
                    tx
                })
                .clone()
        };

        // Ejecutar en el HAL
        let hal_read = hal_runner.read().await;
        match hal_read.route_and_execute(Arc::clone(&shared_pcb)).await {
            Ok(mut stream) => {
                use tokio_stream::StreamExt;
                while let Some(token_result) = stream.next().await {
                    match token_result {
                        Ok(token) => {
                            let event = ank_proto::v1::TaskEvent {
                                pid: pid.clone(),
                                payload: Some(ank_proto::v1::task_event::Payload::Output(token)),
                            };
                            let _ = event_tx.send(event);
                        }
                        Err(e) => {
                            let event = ank_proto::v1::TaskEvent {
                                pid: pid.clone(),
                                payload: Some(ank_proto::v1::task_event::Payload::Error(
                                    e.to_string()
                                )),
                            };
                            let _ = event_tx.send(event);
                            break;
                        }
                    }
                }
                // Notificar completion al scheduler
                let _ = scheduler_tx_runner.send(SchedulerEvent::ProcessCompleted {
                    pid: pid.clone(),
                    output: "stream_complete".to_string(),
                }).await;
                // Enviar STATUS_COMPLETED al WS
                let done_event = ank_proto::v1::TaskEvent {
                    pid: pid.clone(),
                    payload: Some(ank_proto::v1::task_event::Payload::StatusUpdate(
                        ank_proto::v1::ProcessStateUpdate { state: 4 } // STATE_COMPLETED
                    )),
                };
                let _ = event_tx.send(done_event);
            }
            Err(e) => {
                tracing::error!(pid = %pid, "HAL execution failed: {}", e);
                let event = ank_proto::v1::TaskEvent {
                    pid: pid.clone(),
                    payload: Some(ank_proto::v1::task_event::Payload::Error(e.to_string())),
                };
                let _ = event_tx.send(event);
                let _ = scheduler_tx_runner.send(SchedulerEvent::ProcessCompleted {
                    pid,
                    output: format!("error: {}", e),
                }).await;
            }
        }
    }
});
```

---

## Criterios de aceptación

- [ ] `execution_tx` se setea en el scheduler antes de arrancarlo
- [ ] Un prompt enviado via WebSocket genera tokens de respuesta en el browser
- [ ] El evento `STATUS_COMPLETED` (state = 4) llega al browser al terminar
- [ ] Si el HAL retorna error, el browser recibe un evento `error`
- [ ] `cargo build` pasa sin errores

---

## Dependencias

Este es el **ticket más crítico de toda la auditoría** — sin él, el chat
nunca funciona aunque todo lo demás esté bien. Implementar primero.

Requiere que el `CognitiveRouter` tenga al menos una key configurada
(CORE-075 para que la config persista, CORE-083 para que se pueda agregar).
