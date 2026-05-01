# Prompt — Kernel Engineer — EPIC 47: Agent Protocol v2 (Tool Use)

**Para:** Claude Code Max (Kernel Engineer)  
**Repo:** Aegis-Core  
**Branch:** `feat/epic-47-agent-tool-use`  
**Fecha:** 2026-05-01

---

## Contexto

El sistema de agentes de Aegis actualmente usa tokens en texto libre para comunicar
acciones al Orchestrator (`[SYS_AGENT_SPAWN(...)]`). En producción hemos validado
que esto causa tres problemas reales:

1. El modelo mezcla el token con texto conversacional — el usuario lo ve en la UI
2. El modelo puede malformar el token o usar el rol incorrecto sin que el schema lo valide
3. No hay forma de distinguir "hablar" de "actuar" en el mismo stream de texto

El Epic 47 reemplaza este mecanismo por **Tool Use nativo** de la API del proveedor.
Los agentes dejan de emitir tokens y pasan a invocar herramientas estructuradas.
El Orchestrator deja de parsear texto y despacha tool calls directamente.

**ADR de referencia:** `governance/ADR-CAA-015.md`  
**Epic completo:** `governance/EPIC_47_AGENT_PROTOCOL_V2.md`

---

## Orden de implementación obligatorio

```
CORE-236 → CORE-234 → CORE-235 → CORE-237
```

Implementar en ese orden. Cada ticket depende del anterior.

---

## CORE-236 — ToolRegistry

**Ticket completo:** `governance/Tickets/CORE-236.md`

Crear `kernel/crates/ank-core/src/agents/tool_registry.rs`.

Las tres herramientas del protocolo y su schema están definidos en el ticket.
El set de herramientas por rol:
- `ChatAgent` → `[spawn_agent, query_agent]`
- `ProjectSupervisor` | `Supervisor` → `[spawn_agent, query_agent, report]`
- `Specialist` → `[report]` únicamente

Implementar adaptadores de serialización para cada proveedor:
- Anthropic → `{ tools: [{ name, description, input_schema }] }`
- OpenAI / Groq / xAI / OpenRouter → `{ tools: [{ type: "function", function: { name, description, parameters } }] }`
- Gemini → `{ tools: [{ functionDeclarations: [{ name, description, parameters }] }] }`
- Ollama → igual que OpenAI

Este módulo debe exponer `ToolRegistry::tools_for(role, provider) -> Vec<ToolDefinition>`.

---

## CORE-234 — AgentOrchestrator: migrar a tool use dispatch

**Ticket completo:** `governance/Tickets/CORE-234.md`

En `kernel/crates/ank-core/src/agents/orchestrator.rs`:

1. **Eliminar** todo código que busque `SYS_AGENT_SPAWN` o `SYS_AGENT_QUERY` como strings en el output del LLM.

2. **Inyectar herramientas** en cada llamada de inferencia para agentes del árbol. Usar `ToolRegistry::tools_for(role, provider)`. El set depende del rol del agente:
   - `ChatAgent` → `[spawn_agent, query_agent]`
   - `ProjectSupervisor` | `Supervisor` → `[spawn_agent, query_agent, report]`
   - `Specialist` → `[report]`

3. **Procesar tool_calls** en la respuesta del LLM:
   - `spawn_agent` → crear `AgentNode`, hacer `Dispatch`
   - `query_agent` → rutear query hacia abajo en el árbol
   - `report` → registrar resultado, cambiar estado del nodo, notificar al padre

4. **Paralelismo:** si la respuesta contiene múltiples `spawn_agent` en el mismo turn, ejecutarlos en paralelo via el DAG engine existente.

5. **Fallback de texto:** si la respuesta no contiene `tool_calls`, tratar el texto como `report(status="completed", summary=<texto>)` y loguear `WARN: agent responded with text instead of tool call`.

---

## CORE-235 — SyscallExecutor: AgentToolCall

**Ticket completo:** `governance/Tickets/CORE-235.md`

En `kernel/crates/ank-core/src/agents/message.rs` (o nuevo `tool_call.rs`):

Definir:

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

Adaptar `SyscallExecutor::execute()` para recibir `AgentToolCall` en lugar del formato anterior.

El Orchestrator debe enviar de vuelta al LLM el resultado de cada tool call como `tool_result`:
```json
// spawn_agent result
{ "agent_id": "uuid", "status": "spawned" }
// query_agent result
{ "answer": "..." }
// report result
{ "acknowledged": true }
```

---

## CORE-237 — Ollama fallback

**Ticket completo:** `governance/Tickets/CORE-237.md`

Agregar `tool_use_support: ToolUseSupport` (enum: `Unknown | Supported | Degraded`) a las capacidades del proveedor en el CMR.

En `CognitiveHAL`, para provider Ollama:
- Si `Unknown` → intentar con tools, observar resultado
- Si falla (400 o respuesta sin tool_calls cuando se esperaba) → marcar `Degraded`, reintentar sin tools
- Si ok → marcar `Supported`

En modo `Degraded`:
- No registrar herramientas en la llamada
- Solo asignar tareas atómicas al agente
- Loguear `WARN: ollama model X degraded mode`
- El `AgentTreeView` debe mostrar `[degraded]` en el nodo

---

## Reglas para esta implementación

- Solo `cargo build` al finalizar cada ticket — no `cargo test`, no `cargo clippy`
- No hacer `git push` ni crear PRs — Tavo maneja el git
- No modificar archivos fuera de `kernel/crates/ank-core/src/`
- No tocar `kernel/config/agents/*.md` — esos se actualizan en CORE-238 por el Arquitecto IA
- Si algo en el ticket es ambiguo o requiere una decisión de diseño, detenerse y reportar — no asumir

---

## Verificación final

Al terminar CORE-237, el sistema debe comportarse así:

Con un modelo que soporta tool use (Groq, Anthropic, OpenAI):
- El Chat Agent recibe `[spawn_agent, query_agent]` como herramientas disponibles
- Cuando el usuario dice "trabajemos en Aegis", el modelo invoca `spawn_agent(role="project_supervisor", name="Aegis", scope="...")` como tool call estructurado — no como texto
- El token `[SYS_AGENT_SPAWN(...)]` nunca aparece en el output visible al usuario
- El Orchestrator recibe la tool call, crea el ProjectSupervisor, le hace Dispatch

Con Ollama sin tool use:
- El sistema detecta el modo degradado y loguea el WARN
- El agente funciona en modo atómico sin spawn

`cargo build` pasa en `kernel/crates/ank-core` sin warnings nuevos.
