# CORE-235 — feat(ank-core): SyscallExecutor — mapear tool call results a AgentMessage internos

**Tipo:** feat  
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

El `SyscallExecutor` actual fue diseñado para recibir syscalls parseadas desde tokens
de texto. Con la migración a Tool Use (ADR-CAA-015), las acciones de agente llegan como
`ToolCallResult` estructurados desde el `AgentOrchestrator`.

Este ticket adapta el `SyscallExecutor` para recibir tool calls directamente,
eliminando el paso de parsing intermedio.

---

## Cambios requeridos

### 1. Nuevo tipo de entrada: `AgentToolCall`

Definir en `ank-core/src/agents/message.rs` (o un nuevo `tool_call.rs`):

```rust
pub enum AgentToolCall {
    Spawn {
        role:      AgentRole,
        name:      Option<String>,
        scope:     String,
        task_type: Option<TaskType>,
    },
    Query {
        project:  String,
        question: String,
    },
    Report {
        status:       ReportStatus,
        summary:      String,
        observations: Option<String>,
    },
}
```

### 2. Adaptar `SyscallExecutor::execute()`

El método principal debe aceptar `AgentToolCall` en lugar del formato anterior.
Mapear cada variante a la acción interna correspondiente:

- `AgentToolCall::Spawn` → `AgentOrchestrator::spawn_child()`
- `AgentToolCall::Query` → `AgentOrchestrator::route_query()`
- `AgentToolCall::Report` → `AgentOrchestrator::handle_report()`

### 3. Eliminar el path de parsing legacy

Si existe código en `SyscallExecutor` que parsea strings para detectar el tipo
de syscall, eliminarlo. El tipo ahora viene determinado estructuralmente por `AgentToolCall`.

### 4. Serialización del resultado de tool call

El Orchestrator debe enviar de vuelta al LLM el resultado de cada tool call
(como `tool_result` en el mensaje history). El resultado debe ser un string JSON
que confirme la acción:

```json
// spawn_agent result
{ "agent_id": "uuid", "status": "spawned" }

// query_agent result  
{ "answer": "..." }

// report result
{ "acknowledged": true }
```

---

## Criterios de aceptación

- [x] `AgentToolCall` es el único tipo de entrada del `SyscallExecutor` para acciones de agente
- [x] No existe código en `SyscallExecutor` que busque strings de syscall legacy
- [x] El resultado de cada tool call se serializa como JSON y se incluye en el historial del LLM
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

- CORE-236 (ToolRegistry) — los tipos deben ser consistentes
- CORE-234 (Orchestrator) — produce los `AgentToolCall` que este ticket consume
