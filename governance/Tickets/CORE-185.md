# CORE-185 — feat: TTS pipeline en WebSocket Siren — sintetizar respuesta y enviar chunks al frontend

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-http + ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-186 (TTS engine local — debe implementarse primero o en paralelo)

---

## Contexto

El WebSocket Siren (`kernel/crates/ank-http/src/ws/siren.rs`) procesa audio entrante,
transcribe con STT, y schedula una tarea en el scheduler. Pero después de schedulear
la tarea, el handler devuelve `STT_DONE` y **se queda esperando el siguiente mensaje
de audio**. Nunca espera el resultado del scheduler ni envía TTS de vuelta.

El frontend espera recibir `tts_audio_chunk` events por el mismo WebSocket Siren.
Actualmente esos eventos **nunca se mandan** porque el kernel no tiene el mecanismo
para:
1. Esperar la respuesta del proceso schedulado
2. Llamar `siren_router.resolve().synthesize(respuesta)` con esa respuesta
3. Encodear el audio en Base64 y mandarlo como `tts_audio_chunk` por el socket

Este ticket implementa ese pipeline completo.

---

## Causa raíz

En `handle_siren()`, después de:

```rust
state.scheduler_tx.send(SchedulerEvent::ScheduleTask(pcb)).await
```

...el código manda `STT_DONE` y vuelve a `socket.next().await`. No hay ningún
mecanismo para recibir el output del proceso. El scheduler procesa la tarea de forma
asíncrona y emite el resultado por el WebSocket de chat (`/ws/chat/{tenant_id}`),
no por Siren.

---

## Arquitectura del fix

El scheduler ya tiene un mecanismo de `oneshot` channel para retornar outputs
sincrónicamente: `SchedulerEvent::ScheduleTaskConfirmed(pcb, tx)` donde `tx` es
un `oneshot::Sender<String>` que recibe el output final del proceso.

El fix usa este mecanismo para recibir el output directamente en el handler de Siren,
sintetizarlo con TTS y mandarlo en chunks.

**Flujo completo post-fix:**

```
1. Audio PCM llega por sirenWs (binary frames)
2. VAD_END_SIGNAL → process_audio() → transcript ✅ (ya existe)
3. STT_DONE → frontend muestra mensaje del usuario ✅ (ya existe)
4. NUEVO: ScheduleTaskConfirmed con oneshot channel → espera output del LLM
5. NUEVO: siren_router.resolve(tenant_id).synthesize(output) → Vec<u8> PCM
6. NUEVO: chunk el audio en bloques de 8KB, encodear Base64, mandar como tts_audio_chunk
7. NUEVO: mandar TTS_DONE cuando terminen todos los chunks
```

---

## Cambios Requeridos

### `kernel/crates/ank-http/src/ws/siren.rs`

Reemplazar el bloque de scheduleo actual por la versión con oneshot:

```rust
// Importar al inicio del archivo
use tokio::sync::oneshot;

// DENTRO del bloque VAD_END_SIGNAL, reemplazar:

// ANTES
let mut pcb = ank_core::PCB::new("Voice Task".to_string(), 5, transcript.clone());
pcb.tenant_id = Some(tenant_id.clone());
pcb.session_key = Some(session_key.clone());
pcb.task_type = ank_core::pcb::TaskType::Chat;
let pid = pcb.pid.clone();

if let Err(e) = state
    .scheduler_tx
    .send(ank_core::SchedulerEvent::ScheduleTask(Box::new(pcb)))
    .await
{
    error!("Failed to schedule STT transcript: {}", e);
}

// DESPUÉS
let mut pcb = ank_core::PCB::new("Voice Task".to_string(), 5, transcript.clone());
pcb.tenant_id = Some(tenant_id.clone());
pcb.session_key = Some(session_key.clone());
pcb.task_type = ank_core::pcb::TaskType::Chat;
let pid = pcb.pid.clone();

let (output_tx, output_rx) = oneshot::channel::<String>();

if let Err(e) = state
    .scheduler_tx
    .send(ank_core::SchedulerEvent::ScheduleTaskConfirmed(
        Box::new(pcb),
        output_tx,
    ))
    .await
{
    error!("Failed to schedule STT transcript: {}", e);
    continue;
}

// Esperar output del LLM con timeout de 60 segundos
let llm_output = match tokio::time::timeout(
    std::time::Duration::from_secs(60),
    output_rx,
).await {
    Ok(Ok(output)) => output,
    Ok(Err(_)) => {
        warn!("Siren: LLM output channel closed without sending.");
        continue;
    }
    Err(_) => {
        warn!("Siren: LLM output timeout after 60s for pid={}", pid);
        continue;
    }
};

// Sintetizar TTS
let engine = match state.siren_router.resolve(&tenant_id).await {
    Ok(e) => e,
    Err(e) => {
        warn!("Siren: No TTS engine available: {}", e);
        continue;
    }
};

match engine.synthesize(llm_output).await {
    Ok(pcm_bytes) => {
        // Mandar en chunks de 8KB para streaming gapless en el frontend
        const CHUNK_SIZE: usize = 8 * 1024;
        let sample_rate = 22050u32;

        for chunk in pcm_bytes.chunks(CHUNK_SIZE) {
            use base64::Engine as _;
            let b64 = base64::engine::general_purpose::STANDARD.encode(chunk);
            let msg = json!({
                "event": "siren_event",
                "data": {
                    "tts_audio_chunk": b64,
                    "sample_rate": sample_rate
                }
            }).to_string();

            if socket.send(Message::Text(msg)).await.is_err() {
                break; // Socket cerrado
            }
        }

        // Señalar fin del TTS
        let _ = socket.send(Message::Text(
            json!({
                "event": "siren_event",
                "data": { "event_type": "TTS_DONE" }
            }).to_string()
        )).await;
    }
    Err(e) => {
        warn!("Siren: TTS synthesis failed: {}", e);
        // No mandar error al frontend — la respuesta en texto ya se mostró
    }
}
```

### `Cargo.toml` de `ank-http`

Verificar que `base64` esté en las dependencias. Si no está:

```toml
base64 = "0.22"
```

---

## Nota sobre `ScheduleTaskConfirmed`

Buscar en `ank-core/src/scheduler/mod.rs` la definición de `SchedulerEvent`. Si
`ScheduleTaskConfirmed` ya existe con un `oneshot::Sender<String>`, usarlo directamente.
Si no existe, agregarlo:

```rust
// En el enum SchedulerEvent (ank-core/src/scheduler/mod.rs)
ScheduleTaskConfirmed(Box<PCB>, tokio::sync::oneshot::Sender<String>),
```

Y en el handler del scheduler, cuando el proceso completa, enviar el output por el sender:

```rust
// En handle_process_completed o similar
if let Some(sender) = pcb.output_sender.take() {
    let _ = sender.send(output.clone());
}
```

El Kernel Engineer debe buscar cómo el scheduler actualmente retorna outputs
(buscar `ProcessCompleted` en `scheduler/mod.rs`) y adaptar el mecanismo existente.

---

## Criterios de Aceptación

- [ ] Después de que el LLM responde, el sirenWs recibe `tts_audio_chunk` events con audio Base64
- [ ] El `TTSPlayer` del frontend reproduce el audio sin interrupciones (gapless)
- [ ] Se manda `TTS_DONE` al terminar todos los chunks
- [ ] Si `synthesize()` falla (engine no disponible), el proceso continúa sin crash — solo se omite el TTS
- [ ] Timeout de 60s si el LLM no responde — no queda el handler colgado
- [ ] `cargo build` sin warnings (`-D warnings`)
- [ ] No usar `unwrap()` ni `expect()` en código nuevo

---

## Commit message

```
feat(ank-http): CORE-185 TTS pipeline en WebSocket Siren — sintetizar respuesta y enviar chunks
```

---

*Arquitecto IA — 2026-04-25*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: TTS pipeline en WS Siren implementado y tageado: ws/siren.rs:182 'CORE-185: Wait for LLM output then synthesize and stream TTS chunks' (tts_audio_chunk base64).
