# CORE-240 — fix(ank-core): conectar ToolRegistry → CloudProxyDriver → API request

**Tipo:feat
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

El Epic 47 implementó correctamente el `ToolRegistry` (CORE-236) y el `AgentToolCall`
dispatch en el `SyscallExecutor` (CORE-235). Sin embargo, el puente entre el
`ToolRegistry` y el request HTTP al LLM nunca se construyó.

**Evidencia en producción (2026-05-01):** el modelo sigue emitiendo
`[SYS_AGENT_SPAWN(...)]` como texto libre porque nunca recibe las herramientas
en el request. Los logs muestran `AgentToolCall dispatch ready` pero el
`ChatCompletionRequest` que llega a la API de Groq/Anthropic/etc no incluye
el campo `tools`.

**Tres gaps específicos en el código:**

### Gap 1 — `ChatCompletionRequest` no tiene campo `tools`
En `kernel/crates/ank-core/src/chal/drivers/cloud.rs`:
```rust
#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    // ← tools nunca se agregó
}
```

### Gap 2 — `generate_stream()` no recibe tools
La firma actual:
```rust
async fn generate_stream(
    &self,
    prompt: String,
    grammar: Option<Grammar>,
) -> GenerateStreamResult
```
No hay parámetro para tools. El driver no puede saber qué herramientas inyectar.

### Gap 3 — `execute_with_decision()` no consulta el ToolRegistry
En `kernel/crates/ank-core/src/chal/mod.rs`, `execute_with_decision()` llama:
```rust
driver.generate_stream(final_prompt.clone(), None).await
```
Nunca consulta `ToolRegistry::tools_for(role, provider)`.

---

## Cambios requeridos

### 1. Agregar `tools` a `ChatCompletionRequest`

```rust
#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}
```

### 2. Extender `InferenceDriver::generate_stream()` con tools

Cambiar la firma del trait `InferenceDriver` en `chal/mod.rs`:

```rust
async fn generate_stream(
    &self,
    prompt: String,
    grammar: Option<Grammar>,
    tools: Option<Vec<serde_json::Value>>,  // ← nuevo parámetro
) -> GenerateStreamResult;
```

Actualizar todas las implementaciones del trait:
- `CloudProxyDriver` — usar el parámetro e inyectarlo en `ChatCompletionRequest`
- `DummyDriver` — ignorar el parámetro
- Cualquier otro driver existente — ignorar el parámetro

### 3. `execute_with_decision()` consulta el ToolRegistry

En `CognitiveHAL::execute_with_decision()`, antes de llamar al driver, obtener
las tools según el rol del agente en el PCB:

```rust
// Obtener tools del ToolRegistry si el PCB es un agente del árbol
let tools = if let Some(agent_id) = &pcb.agent_id {
    // Obtener el rol del agente desde el AgentTree
    let role = orchestrator.get_agent_role(agent_id).await
        .unwrap_or(AgentRole::ChatAgent);
    let provider = ProviderKind::from_str(&decision.provider);
    Some(ToolRegistry::tools_for(&role, &provider))
} else {
    None
};

match driver.generate_stream(final_prompt.clone(), None, tools).await {
```

### 4. Procesar `tool_calls` en la respuesta del stream

El `CloudProxyDriver` actualmente solo lee `delta.content` del stream SSE.
Cuando el modelo invoca una herramienta, la respuesta incluye `delta.tool_calls`
en lugar de `delta.content`.

Extender el parsing del stream para detectar tool calls:

```rust
#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallChunk>>,  // ← nuevo
}

#[derive(Deserialize)]
struct ToolCallChunk {
    index: u32,
    id: Option<String>,
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<FunctionCall>,
}

#[derive(Deserialize)]
struct FunctionCall {
    name: Option<String>,
    arguments: Option<String>,  // JSON string, acumulado en chunks
}
```

El stream de tool calls llega fragmentado — el `arguments` JSON se acumula
en múltiples chunks. El driver debe acumularlos y emitir el tool call completo
solo cuando `finish_reason == "tool_calls"`.

El tool call completo se emite como un token especial que el `SyscallExecutor`
puede identificar:

```
__TOOL_CALL__{"name":"spawn_agent","arguments":{"role":"project_supervisor","scope":"..."}}
```

El `SyscallExecutor` detecta el prefijo `__TOOL_CALL__` y deserializa el JSON
como `AgentToolCall` en lugar de tratarlo como texto de respuesta.

---

## Criterios de aceptación

- [ ] Con una key de Groq y el Chat Agent activo, el request a la API incluye el campo `tools` con `spawn_agent` y `query_agent`
- [ ] Cuando el usuario dice "trabajemos en Aegis", el modelo invoca `spawn_agent` como tool call — no emite `[SYS_AGENT_SPAWN(...)]` en texto
- [ ] El token `[SYS_AGENT_SPAWN(...)]` nunca aparece en el output visible al usuario
- [ ] El `SyscallExecutor` recibe y ejecuta el `AgentToolCall::Spawn` correctamente
- [ ] Los logs muestran el AgentNode creado con su ID y rol
- [ ] `cargo build` pasa sin warnings nuevos

---

## Dependencias

- CORE-236 (ToolRegistry) — debe estar completo ✅
- CORE-234 (Orchestrator) — debe estar completo ✅
- CORE-235 (SyscallExecutor AgentToolCall) — debe estar completo ✅

Este ticket cierra el loop del Epic 47.
