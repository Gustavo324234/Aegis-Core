# CORE-077 — Fix: `ws/siren.rs` es un mock — conectar al SirenRouter real

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

El WebSocket `/ws/siren/{tenant_id}` en `kernel/crates/ank-http/src/ws/siren.rs`
recibe audio PCM del browser pero **no lo procesa**. El handler es un stub
con una respuesta mock de debug:

```rust
Ok(Message::Binary(_data)) => {
    sequence_number += 1;
    // En un binario unificado, aquí llamaríamos al componente de procesamiento de audio
    // Para ahora, logeamos y devolvemos un evento de procesamiento mock
    // (Referencia CORE-015: Construir AudioChunk proto y enviar al SirenService)

    if sequence_number.is_multiple_of(50) {
        let event = json!({
            "event": "siren_event",
            "data": { "event_type": "AUDIO_PROCESSED", ... }
        });
        // ...
    }
}
```

El `SirenRouter` existe en `ank-core::router::siren` y está instanciado en
`AppState`. El `AnkSirenService` con routing dinámico fue implementado en
Epic 2604 (ANK-2604-004). El WebSocket HTTP simplemente nunca fue conectado
a ese pipeline durante la consolidación.

**Resultado:** La funcionalidad de voz (STT → LLM → TTS) está completamente
rota. La UI muestra el botón de voz pero el audio nunca se procesa.

---

## Referencia de arquitectura

El pipeline de voz existente en los repos legacy (Aegis-ANK/Aegis-Shell):

```
Browser PCM (Int16, 16kHz)
    │  WebSocket binary frames
    ▼
ws/siren.rs  ←── AQUÍ ESTÁ EL STUB
    │  Construir AudioChunk
    ▼
SirenRouter (ank-core::router::siren)
    │  Resolver engine por tenant
    ▼
AnkSirenService
    │  STT (Whisper / VAD)
    ▼  TTS (Voxtral / Mock)
Eventos de respuesta → WebSocket → Browser
```

Los eventos que la UI espera (ver `useAegisStore.ts`):
```typescript
// Tipos de evento esperados:
"VAD_START"   → set status = 'listening'
"STT_START"   → set status = 'transcribing'
"STT_DONE"    → { transcript, pid } — envía al chat pipeline
"STT_ERROR"   → set status = 'error'
// TTS:
tts_audio_chunk + sample_rate → ttsPlayer.playChunk(...)
```

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/ws/siren.rs`

### Paso 1 — Obtener el SirenRouter del AppState

```rust
// El AppState ya tiene:
pub siren_router: Arc<ank_core::router::SirenRouter>,
```

### Paso 2 — Reemplazar el loop de mock

El loop debe:

1. Acumular chunks PCM del browser en un buffer
2. Detectar fin de stream (frame `VAD_END_SIGNAL` en texto, o silencio según
   configuración del SirenRouter)
3. Llamar al pipeline STT del SirenRouter con el buffer acumulado
4. Enviar eventos `VAD_START`, `STT_START`, `STT_DONE` a la UI
5. Con la transcripción, enviar al `scheduler_tx` igual que hace `ws/chat.rs`
6. Enviar chunks TTS de vuelta al browser vía WebSocket

### Paso 3 — Flujo mínimo viable (prioridad para desbloquear smoke test)

Implementar el path más simple que produce output real:

```rust
// Al recibir VAD_END_SIGNAL (texto) o buffer suficientemente grande:
// 1. Enviar VAD_START a UI
// 2. Pasar buffer a siren_router.process_audio(tenant_id, pcm_buffer)
// 3. Enviar STT_START a UI
// 4. Recibir transcript
// 5. Enviar STT_DONE { transcript, pid } a UI
// 6. Inyectar transcript en scheduler_tx como PCB normal
// 7. Streamear respuesta TTS de vuelta (chunks de audio)
```

### Referencia de implementación

Consultar:
- `Aegis-ANK` (legacy, solo lectura): implementación original del Siren gRPC handler
- `kernel/crates/ank-core/src/router/siren.rs`: `SirenRouter` trait y métodos disponibles
- `kernel/crates/ank-server/src/server.rs`: cómo `AnkSirenService` usa el router

---

## Criterios de aceptación

- [ ] No hay código mock ni comentario `// Mock response para debug` en `siren.rs`
- [ ] Al recibir frames PCM y luego `VAD_END_SIGNAL`, el servidor envía `VAD_START`
- [ ] El servidor envía `STT_DONE` con `transcript` no vacío para audio real
- [ ] El transcript llega al scheduler y genera una respuesta del LLM
- [ ] `cargo build` pasa sin errores

---

## Dependencias

- `SirenRouter` debe tener método `process_audio` o equivalente disponible
- Verificar que `ank-core/src/router/siren.rs` expone la interfaz necesaria
- Si el método no existe, crear el stub mínimo en `siren.rs` con `todo!()` documentado
  y abrir ticket hijo para la implementación completa

## Nota de alcance

Si el pipeline STT completo (Whisper) es demasiado complejo para este ticket,
es aceptable implementar un **path mínimo** que:
1. Reciba el audio
2. Envíe `VAD_START` + `STT_START`
3. Retorne el transcript como texto fijo `"[audio received - STT pending]"`
4. Inyecte ese texto al scheduler

Esto desbloquea el smoke test de la UI sin requerir Whisper instalado.
Documentar como `LIM-004` en `AEGIS_CONTEXT.md` si se elige esta opción.
