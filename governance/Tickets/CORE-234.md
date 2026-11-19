# CORE-234 — feat(ank-core): AgentOrchestrator — migrar de token parsing a tool use dispatch

**Tipo:** feat  
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

El `AgentOrchestrator` actual (CORE-193/CORE-198) detecta acciones de agente parseando
tokens en texto libre del output del LLM (`[SYS_AGENT_SPAWN(...)]`, `[SYS_AGENT_QUERY(...)]`).

Según ADR-CAA-015, este mecanismo se reemplaza por Tool Use nativo de la API del proveedor.
El Orchestrator deja de parsear texto y pasa a registrar herramientas en cada llamada al LLM,
recibiendo tool calls estructuradas en la respuesta.

Ver: `governance/ADR-CAA-015.md`, `governance/EPIC_47_AGENT_PROTOCOL_V2.md`

---

## Cambios requeridos

### 1. Eliminar el token parser
En `ank-core/src/agents/orchestrator.rs`, eliminar toda lógica que busque
`SYS_AGENT_SPAWN` o `SYS_AGENT_QUERY` en el texto de respuesta del LLM.

### 2. Registrar herramientas en cada llamada al LLM
Antes de enviar una llamada de inferencia para un agente, inyectar el array de
herramientas del `ToolRegistry` (CORE-236) en el request.

El set de herramientas a registrar depende del rol del agente:
- `ChatAgent` → `[spawn_agent, query_agent]`
- `ProjectSupervisor` → `[spawn_agent, query_agent, report]`
- `Supervisor` → `[spawn_agent, query_agent, report]`
- `Specialist` → `[report]` únicamente — los specialists no pueden spawnear ni hacer query

### 3. Procesar tool_calls en la respuesta
Si la respuesta del LLM contiene `tool_calls` (o el equivalente del proveedor),
enrutar cada llamada al handler correspondiente:

- `spawn_agent` → crear `AgentNode` y hacer `Dispatch` (igual que antes, pero sin parsing)
- `query_agent` → rutear query hacia abajo en el árbol
- `report` → registrar resultado, cambiar estado del nodo, notificar al padre

Si la respuesta no contiene `tool_calls` (respuesta de texto puro):
- Para agentes que deberían reportar: tratar el texto como `report(status="completed", summary=<texto>)`
- Loguear como `WARN: agent responded with text instead of tool call`

### 4. Paralelismo en spawn
Cuando la respuesta contiene múltiples `spawn_agent` en el mismo turn,
ejecutarlos en paralelo via el DAG engine existente (no secuencialmente).

---

## Criterios de aceptación

- [ ] El Orchestrator no contiene ninguna referencia a `SYS_AGENT_SPAWN` ni `SYS_AGENT_QUERY` como strings a parsear
- [ ] Cada llamada de inferencia a un agente incluye las herramientas correspondientes a su rol
- [ ] Un `spawn_agent` recibido como tool call crea un AgentNode correctamente (verificable via `GET /api/agents/tree`)
- [ ] Dos `spawn_agent` en el mismo turn se ejecutan en paralelo (verificable en logs con timestamps)
- [ ] Un `report(status="error", ...)` cambia el estado del nodo a `FAILED` y notifica al padre
- [ ] `cargo build` pasa sin warnings nuevos

---

## Dependencias

- CORE-236 debe estar completo antes de iniciar este ticket (el ToolRegistry debe existir)
- CORE-235 se implementa en paralelo o después
