# CORE-260 — PCB: historial de mensajes + SessionHistoryCache

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Estado:** ✅ Done  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/pcb.rs`
- `kernel/crates/ank-http/src/ws/chat.rs`

---

## Contexto

Cada request creaba un PCB nuevo sin contexto del turno anterior. El LLM empezaba de cero en cada mensaje.

## Lo que se implementó

### `pcb.rs` — campo `message_history`

```rust
/// CORE-260: Historial de mensajes de la sesión actual (máx. 20).
#[serde(default)]
pub message_history: Vec<crate::chal::ChatMessage>,
```

Inicializado como `Vec::new()` en `PCB::new()`. Serializable con `#[serde(default)]` para compatibilidad con PCBs existentes.

### `ws/chat.rs` — `SessionHistoryCache`

Cache en memoria por conexión WebSocket:

```rust
// Al inicio del handler handle_chat()
let session_history: Arc<tokio::sync::Mutex<VecDeque<ChatMessage>>> =
    Arc::new(tokio::sync::Mutex::new(VecDeque::new()));
```

Al crear el PCB para cada mensaje:
```rust
let user_message_text = prompt.clone();
// ...
{
    let history = session_history.lock().await;
    pcb.message_history = history.iter().cloned().collect();
}
```

Al completar (después de `stream_with_receiver`):
```rust
if !full_response.is_empty() {
    let mut history = session_history.lock().await;
    history.push_back(ChatMessage { role: ChatRole::User, content: Some(user_message_text.clone()), ..Default::default() });
    history.push_back(ChatMessage { role: ChatRole::Assistant, content: Some(full_response.clone()), ..Default::default() });
    while history.len() > 20 {
        history.pop_front();
    }
}
```

### `build_messages` en `chal/mod.rs` — usa el historial

```rust
// CORE-260: Historial previo de la sesión
messages.extend(pcb.message_history.clone());
```

Este extend va después del system message y antes del mensaje del usuario actual.

## Criterios de aceptación

- [x] `PCB.message_history` existe y se popula al crear el PCB
- [x] En la misma sesión WebSocket, el LLM recibe el historial de los últimos 20 mensajes
- [x] Al reconectar (nuevo WebSocket), el historial empieza vacío
- [x] Truncado automático a 20 mensajes
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-259 ✅
