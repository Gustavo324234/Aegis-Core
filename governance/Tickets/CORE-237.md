# CORE-237 — feat(ank-core): Ollama fallback — detección de tool use support + modo degradado

**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

No todos los modelos disponibles via Ollama soportan tool use. Los modelos pequeños
(< 7B parámetros) o modelos sin fine-tuning específico pueden devolver error 400
o ignorar silenciosamente las tool definitions.

El sistema debe detectar esta situación y operar en modo degradado en lugar de fallar.

---

## Comportamiento esperado

### Detección
Al hacer la primera llamada de inferencia con tools a un modelo Ollama:
- Si la respuesta es exitosa y contiene `tool_calls` → modo normal
- Si la respuesta es error 400 con mensaje relacionado a tools → modo degradado
- Si la respuesta es texto puro sin `tool_calls` (esperaba un spawn/query) → modo degradado

La detección se realiza una vez por sesión por modelo. El resultado se cachea
en el `ProviderRegistry` del CMR.

### Modo degradado
Cuando un agente Ollama está en modo degradado:
- El Orchestrator **no registra herramientas** en la llamada al LLM
- El agente solo recibe tareas atómicas (el Orchestrator no le envía tareas que requieren spawn)
- Si el agente recibe una tarea que requeriría spawn, el Orchestrator la rechaza con `BLOCKED`
  y sube el bloqueo al padre
- El reporte es implícito: fin de stream = `report(status="completed", summary=<texto>)`

### Visibilidad
- Log `WARN` al entrar en modo degradado: `ollama model <name> does not support tool use — degraded mode`
- El `AgentTreeView` muestra el nodo con indicador `[degraded]` junto al nombre del modelo

---

## Cambios requeridos

### 1. Campo `tool_use_support` en `ProviderCapabilities`

En el struct que representa las capacidades de un proveedor/modelo en el CMR,
agregar:

```rust
pub tool_use_support: ToolUseSupport,
// enum ToolUseSupport { Unknown, Supported, Degraded }
```

### 2. Detección activa en `CognitiveHAL`

En el path de inferencia para Ollama, envolver la llamada con detección:
- Si `tool_use_support == Unknown` → intentar con tools, observar resultado
- Si falla → marcar como `Degraded`, reintentar sin tools
- Si ok → marcar como `Supported`

### 3. `AgentOrchestrator` respeta el modo degradado

Antes de asignar una tarea a un agente, verificar si está en modo degradado.
Si la tarea requiere spawn y el agente está degradado, no asignar — reportar `BLOCKED` al padre.

---

## Criterios de aceptación

- [x] Con un modelo Ollama sin tool use, el sistema no crashea ni retorna 500
- [x] El log muestra `WARN: ollama model X degraded mode` en la primera detección
- [x] El `AgentTreeView` muestra `[degraded]` para el nodo afectado
- [x] Una tarea que requiere spawn asignada a un agente degradado retorna `BLOCKED` al padre
- [x] Con un modelo Ollama que sí soporta tools (ej. llama3.1), el sistema funciona en modo normal
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

- CORE-236 (ToolRegistry)
- CORE-234 (Orchestrator) — este ticket modifica el comportamiento del Orchestrator en el caso degradado
