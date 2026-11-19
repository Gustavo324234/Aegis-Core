# CORE-259 — CloudProxyDriver: historial de mensajes Vec<ChatMessage> en lugar de String

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Estado:** ✅ Done  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

---

## Contexto

`InferenceDriver::generate_stream` recibía un `prompt: String` y construía siempre un único mensaje `[{ role: "user", content: <todo_concatenado> }]`. Esto impedía historial real, tool call results y el bucle ReAct.

## Lo que se implementó

### Tipos definidos en `chal/mod.rs`

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ChatMessage {
    pub role: ChatRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole { #[default] User, System, Assistant, Tool }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionCallRecord,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionCallRecord {
    pub name: String,
    pub arguments: String,  // JSON string
}

// Para parsear tokens __TOOL_CALL__ del driver
#[derive(serde::Deserialize)]
struct DriverToolCallPayload {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}
```

### Firma del trait actualizada

```rust
async fn generate_stream(
    &self,
    messages: Vec<ChatMessage>,
    grammar: Option<Grammar>,
    tools: Option<Vec<serde_json::Value>>,
) -> GenerateStreamResult;
```

### `build_messages` en `CognitiveHAL`

Retorna `Vec<ChatMessage>` con:
1. Un `ChatMessage { role: System }` con todo el system content (instrucciones + persona + herramientas)
2. `pcb.message_history` (historial de la sesión — CORE-260)
3. Un `ChatMessage { role: User }` con el contexto ensamblado por el VCM

### Diferencias respecto al diseño original

- `content` es `Option<String>` (no `ChatContent` enum) — simplifica la serialización
- `ToolCallRecord` tiene `type_` y `function: FunctionCallRecord` en lugar de `name`/`arguments` directos — sigue el formato nativo de OpenAI
- `DriverToolCallPayload` es el tipo intermedio para parsear tokens `__TOOL_CALL__` del stream

## Criterios de aceptación

- [x] `InferenceDriver::generate_stream` acepta `Vec<ChatMessage>`
- [x] `ChatMessage`, `ChatRole`, `ToolCallRecord`, `FunctionCallRecord` definidos en `chal/mod.rs`
- [x] `build_messages` retorna `Vec<ChatMessage>` con system message separado del user message
- [x] `CloudProxyDriver` serializa el array de mensajes correctamente
- [x] `DummyDriver` actualizado — tests existentes pasan
- [x] `cargo build --workspace` sin errores

## Dependencias

Ninguna — es el ticket base de la Fase 1.
