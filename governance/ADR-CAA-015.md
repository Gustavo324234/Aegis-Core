# ADR-CAA-015 — Tool Use over Text Tokens for Agent Actions

**Estado:** ACCEPTED  
**Fecha:** 2026-04-29  
**Arquitecto:** Arquitecto IA  
**Epic:** EPIC 47 — Agent Protocol v2 (Tool Use)  
**Reemplaza:** Diseño de tokens en texto (`[SYS_AGENT_SPAWN(...)]`) documentado en PROTOCOL.md v1

---

## Contexto

La implementación actual del protocolo de agentes (CORE-198, CORE-199) utiliza tokens
en texto libre embebidos en el output del LLM para señalizar acciones al Orchestrator:

```
[SYS_AGENT_SPAWN(role="supervisor", name="Kernel", scope="...")]
[SYS_AGENT_QUERY(project="aegis", question="...")]
```

El Orchestrator parsea estos tokens con regex antes de procesar el resto del texto.

Este enfoque fue elegido por simplicidad de implementación inicial, pero presenta
problemas estructurales que afectan la confiabilidad y eficiencia del sistema.

---

## Problemas del enfoque actual

### 1. Fragilidad de parsing
Los LLMs no garantizan formato exacto. Un modelo puede producir:
- `[sys_agent_spawn(...)]` (lowercase)
- `[ SYS_AGENT_SPAWN (...) ]` (espacios)
- `I'll call [SYS_AGENT_SPAWN(...)]` (token embebido en texto)
- Explicar el token en lugar de emitirlo

Cualquiera de estos casos rompe el parser o produce comportamiento silenciosamente incorrecto.

### 2. Overhead de tokens
El modelo gasta tokens escribiendo la sintaxis del token: corchetes, nombre del comando,
nombres de parámetros, comillas. Para un spawn típico, ~20-30 tokens son puro overhead
de formato que no aporta razonamiento.

### 3. Mezcla de razonamiento y acción
El modelo "habla" y "actúa" en el mismo stream. Esto produce patrones como:
> "Necesito crear un supervisor para el módulo de auth. Lo haré ahora:
> [SYS_AGENT_SPAWN(role="supervisor"...)]"

Los tokens de razonamiento previos a la acción son innecesarios y consumen contexto.

### 4. Paralelismo forzado a workaround
Para spawnear múltiples agentes en paralelo, el diseño con tokens requiere que el
Orchestrator acumule todos los tokens del turn antes de actuar. Con tool use, la API
devuelve un array de llamadas que se ejecutan en paralelo de forma nativa.

### 5. Ambigüedad de reporte
Con tokens, el "reporte" es implícito: el agente simplemente termina su respuesta.
El Orchestrator interpreta el fin del stream como señal de completitud. No hay señal
explícita de status (completado / error / bloqueado).

---

## Decisión

**Se adopta Tool Use (Function Calling) como mecanismo exclusivo de comunicación
entre agentes y el Orchestrator.**

Los agentes dejan de emitir tokens en texto para señalizar acciones. En su lugar,
el Orchestrator registra un conjunto fijo de herramientas en cada llamada al LLM.
El modelo invoca estas herramientas usando el mecanismo nativo de la API del proveedor.

### Las tres herramientas del protocolo

```json
spawn_agent:
  role:      "project_supervisor" | "supervisor" | "specialist"  (required)
  name:      string                                               (optional)
  scope:     string                                               (required)
  task_type: "code" | "analysis" | "planning" | "creative"       (optional)

query_agent:
  project:  string   (required)
  question: string   (required)

report:
  status:       "completed" | "error" | "blocked"   (required)
  summary:      string                               (required)
  observations: string                               (optional)
```

`spawn_agent` y `query_agent` reemplazan `SYS_AGENT_SPAWN` y `SYS_AGENT_QUERY`.
`report` es una herramienta nueva que hace explícito el reporte del agente,
eliminando la ambigüedad del "fin de stream como señal".

### Parallelism
El Orchestrator puede recibir múltiples `spawn_agent` en el mismo turn.
Los ejecuta en paralelo via el DAG engine existente (ADR-CAA-004).

---

## Compatibilidad con proveedores

| Proveedor | Tool Use | Notas |
|---|---|---|
| Anthropic | ✅ | `tools` array en API |
| OpenAI | ✅ | `tools` / `functions` |
| Groq | ✅ | Compatible con formato OpenAI |
| Gemini | ✅ | `tools` con `function_declarations` |
| xAI (Grok) | ✅ | Compatible con formato OpenAI |
| OpenRouter | ✅ | Pasa tool definitions al modelo subyacente |
| Ollama | ⚠️ | Depende del modelo. Modelos >= 7B con soporte de tools: llama3.1, qwen2.5, mistral-nemo |

### Fallback para Ollama sin tool use
Si el modelo activo no soporta tool use (detectable por error 400 de la API),
el Orchestrator cae al modo degradado:
- El agente solo puede ejecutar tareas atómicas (sin spawn, sin query)
- El Orchestrator no registra herramientas en la llamada
- El reporte es implícito (fin de stream)
- Este modo se loguea como `WARN` y es visible en el AgentTreeView

---

## Consecuencias

### Positivas
- Parser de regex eliminado del Orchestrator
- Los agent files (`*.md`) se simplifican: no necesitan documentar sintaxis, solo semántica
- Paralelismo nativo sin workarounds
- Reporte explícito con status estructurado
- Ahorro de ~20-30 tokens por acción de agente
- Comportamiento más predecible y testeable

### Negativas
- Requiere refactor de `AgentOrchestrator` (CORE-234)
- Requiere refactor de `SyscallExecutor` para mapear tool calls a syscalls (CORE-235)
- Los tickets CORE-198 y CORE-199 (implementados con tokens) quedan obsoletos y deben ser reemplazados
- Los agent files deben ser reescritos para eliminar referencias a tokens (CORE-238)
- El PROTOCOL.md debe ser reescrito (CORE-238)

---

## Tickets derivados

| ID | Título |
|---|---|
| CORE-234 | AgentOrchestrator: migrar de token parsing a tool use dispatch |
| CORE-235 | SyscallExecutor: mapear tool call results a AgentMessage internos |
| CORE-236 | ToolRegistry: definición de las 3 herramientas + schema JSON por proveedor |
| CORE-237 | Ollama fallback: detección de tool use support + modo degradado |
| CORE-238 | Agent files + PROTOCOL.md: reescritura post tool use |

---

*Decisión tomada por Arquitecto IA — 2026-04-29*
