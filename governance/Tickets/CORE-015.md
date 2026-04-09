# CORE-015 — ank-http: WebSocket /ws/siren/{tenant_id}

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/ws/siren.rs`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟡 Media
**Estado:** COMPLETED
**Depende de:** CORE-014

---

## Contexto

El WebSocket Siren es el canal de audio bidireccional.
El cliente envía bytes PCM (16kHz, 16-bit) y recibe eventos de audio TTS de vuelta.

**Referencia:** `Aegis-Shell/bff/main.py` función `websocket_siren_endpoint`

---

## Protocolo

- **Cliente → Servidor:** bytes binarios (audio PCM 16kHz 16-bit)
- **Servidor → Cliente:** JSON `{ "event": "siren_event", "data": {...} }`
- **Auth:** mismo mecanismo que `/ws/chat/` — `Sec-WebSocket-Protocol: session-key.<valor>`

## Flujo interno

1. Autenticar igual que chat
2. Recibir chunks de audio del cliente
3. Construir `AudioChunk` proto con `sequence_number` incremental
4. Enviar al `SirenService` del kernel vía stream bidireccional gRPC
5. Retransmitir eventos Siren al cliente como JSON

---

## Criterios de aceptación

- [ ] La conexión se rechaza si la auth falla
- [ ] Bytes binarios recibidos se convierten en `AudioChunk` correctamente
- [ ] Eventos Siren se retransmiten al cliente como JSON
- [ ] La desconexión limpia el stream gRPC sin errores
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-Shell/bff/main.py` función `websocket_siren_endpoint` (líneas ~480–540)
