# CORE-261 — CognitiveHAL: bucle ReAct interno — tool call → resultado → LLM

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Estado:** ✅ Done  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/crates/ank-server/src/main.rs`

---

## Contexto

`execute_with_decision` llamaba al LLM una sola vez. Cuando el stream emitía un token `__TOOL_CALL__`, el `StreamInterceptor` en `ank-server` lo capturaba, ejecutaba el syscall, e inyectaba mensajes hardcodeados al usuario. El LLM nunca recibía el resultado de la herramienta.

## Lo que se implementó

### `chal/mod.rs` — `execute_with_decision` con bucle ReAct

Firma actualizada — recibe canal de salida en lugar de retornar stream:

```rust
async fn execute_with_decision(
    &self,
    decision: RoutingDecision,
    pcb: &PCB,
    pid: &str,
    persona: Option<&str>,
    text_tx: tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
) -> Result<(), SystemError>
```

Bucle interno con MAX_ITERATIONS = 10:
1. Llama al driver con el historial completo de mensajes
2. Consume el stream separando tokens de texto de tokens `__TOOL_CALL__`
3. Envía tokens de texto al usuario via `text_tx`
4. Si hay tool calls: inyecta respuesta del asistente + resultado de cada tool al historial y vuelve al paso 1
5. Si no hay tool calls: termina

`route_and_execute` ahora usa `Arc<Self>` y crea el canal internamente:
```rust
let (text_tx, text_rx) = tokio::sync::mpsc::unbounded_channel();
tokio::spawn(async move {
    hal_arc.execute_with_decision(decision, &pcb, &pid, persona, text_tx).await
});
// Convierte text_rx en Stream y lo retorna
```

### `chal/mod.rs` — `execute_tool_call_internal`

Arms implementados:
- `spawn_agent` → llama a `orchestrator.create_project()`
- `query_agent` → llama a `orchestrator.handle_tool_call(AgentToolCall::Query)`
- `call_plugin` → llama a `plugin_manager.execute_plugin()`
- `_` → `{"error": "Unknown tool: X"}`

### `chal/mod.rs` — campo `agent_orchestrator` en `CognitiveHAL`

```rust
pub agent_orchestrator: RwLock<Option<Arc<crate::agents::orchestrator::AgentOrchestrator>>>,
```

Con método `set_orchestrator()` que se llama en `main.rs` después de crear el orchestrator.

### `ank-server/main.rs` — estado actual

El `StreamInterceptor` **no fue eliminado** — sigue existente para syscalls legacy (MAKER, plugins via texto). Sin embargo, el comentario en el código indica que los `__TOOL_CALL__` tokens ya son consumidos internamente por el ReAct loop antes de llegar al interceptor.

El path de error del HAL Runner ya envía `StatusUpdate state=4` al `event_tx` (CORE-244 implementado en el mismo cambio).

### `DriverToolCallPayload` — tipo intermediario

```rust
#[derive(serde::Deserialize)]
struct DriverToolCallPayload {
    id: String,
    name: String,
    arguments: serde_json::Value,
}
```

Se usa para parsear los tokens `__TOOL_CALL__` del driver antes de convertirlos a `ToolCallRecord` (que usa el formato OpenAI con `type_` y `function`).

## Criterios de aceptación

- [x] Cuando el LLM emite un tool call, el HAL lo ejecuta internamente y vuelve a llamar al LLM con el resultado
- [x] El usuario solo ve tokens de texto — nunca strings `__TOOL_CALL__` crudos
- [x] El bucle tiene límite de 10 iteraciones con mensaje amigable al superarlo
- [x] `spawn_agent`, `query_agent`, `call_plugin` funcionan internamente
- [x] Path de error del HAL Runner envía StatusUpdate al WebSocket (CORE-244)
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-259 ✅
- CORE-260 ✅
