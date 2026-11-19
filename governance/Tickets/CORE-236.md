# CORE-236 — feat(ank-core): ToolRegistry — definición de herramientas + schema por proveedor

**Tipo:** feat  
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

El protocolo Tool Use requiere enviar la definición de herramientas disponibles
en cada llamada al LLM. Cada proveedor del catálogo de Aegis (Anthropic, OpenAI,
Groq, Gemini, xAI, OpenRouter) usa un formato de definición distinto pero
semánticamente equivalente.

El `ToolRegistry` centraliza las definiciones y es responsable de serializar
el set correcto de herramientas para cada proveedor.

---

## Las tres herramientas del protocolo

### `spawn_agent`
```json
{
  "name": "spawn_agent",
  "description": "Create a subordinate agent to handle a specific task or domain.",
  "parameters": {
    "type": "object",
    "properties": {
      "role":      { "type": "string", "enum": ["project_supervisor", "supervisor", "specialist"],
                     "description": "The role of the new agent." },
      "name":      { "type": "string",
                     "description": "Human-readable identifier. Required for project_supervisor." },
      "scope":     { "type": "string",
                     "description": "Task or domain description. Injected into the agent's instructions." },
      "task_type": { "type": "string", "enum": ["code", "analysis", "planning", "creative"],
                     "description": "Cognitive nature of the task. Used by the CMR to select a model." }
    },
    "required": ["role", "scope"]
  }
}
```

### `query_agent`
```json
{
  "name": "query_agent",
  "description": "Query an active project for information without creating any work.",
  "parameters": {
    "type": "object",
    "properties": {
      "project":  { "type": "string", "description": "Project name to query." },
      "question": { "type": "string", "description": "The specific question to answer." }
    },
    "required": ["project", "question"]
  }
}
```

### `report`
```json
{
  "name": "report",
  "description": "Report the result of your work to your parent agent.",
  "parameters": {
    "type": "object",
    "properties": {
      "status":       { "type": "string", "enum": ["completed", "error", "blocked"],
                        "description": "Outcome of the task." },
      "summary":      { "type": "string",
                        "description": "Concise summary of what was done or what failed." },
      "observations": { "type": "string",
                        "description": "Optional findings relevant to the parent agent." }
    },
    "required": ["status", "summary"]
  }
}
```

---

## Cambios requeridos

### 1. Nuevo módulo `ank-core/src/agents/tool_registry.rs`

```rust
pub struct ToolRegistry;

impl ToolRegistry {
    /// Returns the tool definitions for a given agent role,
    /// serialized in the format expected by the given provider.
    pub fn tools_for(role: &AgentRole, provider: &ProviderKind) -> Vec<ToolDefinition>;
}
```

El set de herramientas por rol:
- `ChatAgent` → `[spawn_agent, query_agent]`
- `ProjectSupervisor` | `Supervisor` → `[spawn_agent, query_agent, report]`
- `Specialist` → `[report]`

### 2. Adaptadores por proveedor

Implementar serialización para cada `ProviderKind` del catálogo existente:

| Proveedor | Formato |
|---|---|
| Anthropic | `{ tools: [{ name, description, input_schema }] }` |
| OpenAI / Groq / xAI / OpenRouter | `{ tools: [{ type: "function", function: { name, description, parameters } }] }` |
| Gemini | `{ tools: [{ functionDeclarations: [{ name, description, parameters }] }] }` |
| Ollama | `{ tools: [{ type: "function", function: { name, description, parameters } }] }` |

### 3. Integración con `CognitiveHAL`

En `CognitiveHAL::build_request()`, si el PCB corresponde a un agente del árbol,
obtener las herramientas del `ToolRegistry` e inyectarlas en el request.

---

## Criterios de aceptación

- [ ] `ToolRegistry::tools_for(AgentRole::Specialist, ProviderKind::Anthropic)` retorna solo `[report]`
- [ ] `ToolRegistry::tools_for(AgentRole::Supervisor, ProviderKind::OpenAI)` retorna `[spawn_agent, query_agent, report]`
- [ ] El JSON serializado para Anthropic es válido según la API de Anthropic (verificable en logs DEBUG)
- [ ] El JSON serializado para OpenAI es válido según la API de OpenAI
- [ ] `cargo build` pasa sin warnings nuevos

---

## Dependencias

Este ticket es bloqueante para CORE-234 y CORE-235.
