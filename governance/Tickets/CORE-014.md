# CORE-014 — ank-http: WebSocket /ws/chat/{tenant_id}

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/ws/chat.rs`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — es la funcionalidad principal del sistema
**Estado:** DONE
**Depende de:** CORE-011

---

## Contexto

El WebSocket de chat es el canal principal entre la UI y el kernel.
La UI React abre una conexión WS, envía prompts en JSON, y recibe
eventos del kernel en streaming.

**Protocolo de autenticación WebSocket:**
La session_key viaja en el header `Sec-WebSocket-Protocol` con el formato
`session-key.<valor>`. Esto es idéntico al comportamiento del BFF Python.

**Referencia:** `Aegis-Shell/bff/main.py` función `websocket_chat_endpoint`

---

## Protocolo de mensajes (mismo que hoy)

### Cliente → Servidor (JSON)

```json
// Enviar prompt
{ "action": "submit", "prompt": "...", "task_type": "chat" }

// Observar tarea existente
{ "action": "watch", "pid": "uuid-..." }
```

### Servidor → Cliente (JSON)

```json
// Confirmación de conexión
{ "event": "syslog", "data": "Aegis Shell established secure bridge for tenant: X" }

// Tarea aceptada
{ "event": "status", "data": "Task accepted. PID: uuid-...", "pid": "uuid-..." }

// Evento del kernel
{ "event": "kernel_event", "data": { "thought": "..." } }
{ "event": "kernel_event", "data": { "output": "..." } }
{ "event": "kernel_event", "data": { "status_update": { "state": "STATE_COMPLETED" } } }

// Error
{ "event": "error", "data": "mensaje de error" }
```

---

## Trabajo requerido

### `src/ws/chat.rs`

```rust
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, State},
    response::IntoResponse,
};
use serde_json::{json, Value};
use crate::{citadel::hash_passphrase, state::AppState};

pub async fn ws_chat_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Extraer session_key del header Sec-WebSocket-Protocol
    let session_key = extract_session_key(&headers);

    ws.protocols(["session-key"])
        .on_upgrade(move |socket| handle_chat(socket, tenant_id, session_key, state))
}

async fn handle_chat(
    mut socket: WebSocket,
    tenant_id: String,
    raw_session_key: Option<String>,
    state: AppState,
) {
    // 1. Validar session_key
    // 2. Autenticar contra Citadel
    // 3. Enviar syslog de bienvenida
    // 4. Loop: recibir mensaje → submit_task → watch_task → streamear eventos
    // 5. Terminar limpiamente en WebSocketDisconnect
}
```

**Flujo interno del loop:**
1. Recibir JSON del cliente
2. Si `action == "watch"`: llamar `ank-core::scheduler` para suscribirse al PID
3. Si `action == "submit"`: llamar `ank-core::scheduler` para crear tarea nueva,
   luego suscribirse al stream de eventos
4. Por cada evento del kernel: serializar a JSON y enviar al cliente
5. Terminar el stream cuando `state == STATE_COMPLETED || STATE_TERMINATED`

---

## Criterios de aceptación

- [ ] La conexión WS se acepta solo con `session-key.<valor>` válido en el protocolo
- [ ] La conexión se rechaza con código 1008 si la auth falla
- [ ] `{ "action": "submit", "prompt": "..." }` crea una tarea y retorna el PID
- [ ] Los eventos del kernel se streamean como `{ "event": "kernel_event", "data": {...} }`
- [ ] El stream termina automáticamente cuando el kernel retorna `STATE_COMPLETED`
- [ ] `{ "action": "watch", "pid": "..." }` se subscribe a una tarea existente
- [ ] La desconexión del cliente no produce panic ni error no manejado
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-Shell/bff/main.py` función `websocket_chat_endpoint` (líneas ~370–480)
