# EPIC 47 — Agent Protocol v2: Tool Use

**Estado:** PLANNED  
**Fecha de diseño:** 2026-04-29  
**Arquitecto:** Arquitecto IA  
**Repos afectados:** Aegis-Core (kernel + kernel/config/agents)  
**ADR:** ADR-CAA-015  
**Extiende:** EPIC 45 (Cognitive Agent Architecture)

---

## Visión

Reemplazar el protocolo de tokens en texto libre (`[SYS_AGENT_SPAWN(...)]`) por
Tool Use nativo de la API del proveedor. Los agentes dejan de "escribir" acciones
y pasan a "llamar" herramientas. El Orchestrator deja de parsear texto y pasa a
despachar llamadas estructuradas.

**Resultado:** protocolo sin fricción, cero ambigüedad, paralelismo nativo,
~20-30 tokens ahorrados por acción, agent files más cortos y más claros.

---

## Las tres herramientas

### `spawn_agent`
Crea un agente subordinado bajo el agente que llama.

```json
{
  "name": "spawn_agent",
  "parameters": {
    "role":      { "type": "string", "enum": ["project_supervisor", "supervisor", "specialist"] },
    "name":      { "type": "string" },
    "scope":     { "type": "string" },
    "task_type": { "type": "string", "enum": ["code", "analysis", "planning", "creative"] }
  },
  "required": ["role", "scope"]
}
```

### `query_agent`
Consulta información de un proyecto activo sin crear trabajo nuevo.

```json
{
  "name": "query_agent",
  "parameters": {
    "project":  { "type": "string" },
    "question": { "type": "string" }
  },
  "required": ["project", "question"]
}
```

### `report`
Reporta el resultado del trabajo al agente padre. Hace explícito el fin de tarea.

```json
{
  "name": "report",
  "parameters": {
    "status":       { "type": "string", "enum": ["completed", "error", "blocked"] },
    "summary":      { "type": "string" },
    "observations": { "type": "string" }
  },
  "required": ["status", "summary"]
}
```

---

## Flujo con tool use

```
Orchestrator builds LLM request:
  - system_prompt: instructions from agent/*.md
  - tools: [spawn_agent, query_agent, report]
  - messages: [task dispatch]

LLM responds with tool_calls: [
  spawn_agent(role="supervisor", name="Kernel", scope="..."),
  spawn_agent(role="supervisor", name="Shell",  scope="...")
]

Orchestrator receives tool_calls array:
  - spawns both supervisors in parallel via DAG engine
  - waits for both Reports
  - resumes parent agent with consolidated results
```

---

## Compatibilidad y fallback

Ver ADR-CAA-015 para la tabla de compatibilidad por proveedor.

El modo degradado para Ollama sin tool use es detectado automáticamente (CORE-237):
el agente recibe solo tareas atómicas, no puede spawnear ni hacer query,
el reporte es implícito. Visible como `WARN` en logs y en AgentTreeView.

---

## Tickets

| ID | Título | Tipo | Asignado a | Prioridad |
|---|---|---|---|---|
| CORE-234 | AgentOrchestrator: migrar de token parsing a tool use dispatch | feat | Kernel Engineer | Crítica |
| CORE-235 | SyscallExecutor: mapear tool call results a AgentMessage internos | feat | Kernel Engineer | Crítica |
| CORE-236 | ToolRegistry: definición de herramientas + schema por proveedor | feat | Kernel Engineer | Crítica |
| CORE-237 | Ollama fallback: detección de tool use support + modo degradado | feat | Kernel Engineer | Alta |
| CORE-238 | Agent files + PROTOCOL.md: reescritura post tool use | docs | Arquitecto IA | Alta |

## Orden de implementación

```
CORE-236 (schema) → CORE-234 (orchestrator) → CORE-235 (syscall mapping)
                  → CORE-237 (ollama fallback)  [paralelo con 235]
                  → CORE-238 (docs)             [último, cuando kernel estable]
```

---

*Documento creado por Arquitecto IA — 2026-04-29*
