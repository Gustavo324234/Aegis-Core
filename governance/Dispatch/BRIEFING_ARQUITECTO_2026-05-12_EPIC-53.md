# BRIEFING — Arquitecto IA
## EPIC 53 — Stabilization: Fase 1 + Fase 4 (Docs)
**Fecha:** 2026-05-12

---

## CORE-297 — chat_agent.md: flujo automático de proyectos y delegación
**Prioridad:** CRÍTICA — implementar después de CORE-298 y CORE-262

El Chat Agent no sabe que su rol es delegar trabajo a supervisores. En la primera
sesión real de uso, el usuario tuvo que decirle explícitamente "tenés que crear
siempre un supervisor". Eso nunca debería ser necesario.

### Cambio — `kernel/config/agents/chat_agent.md`

Leer el archivo actual, localizar la sección de capacidades o instrucciones
generales, y agregar la siguiente sección:

```markdown
## FLUJO DE TRABAJO CON PROYECTOS

Cuando el usuario pide realizar trabajo sobre un proyecto (clonar un repo,
implementar algo, revisar código, construir una feature, etc.), tu rol es
DELEGAR, no ejecutar directamente.

**Protocolo obligatorio — en este orden:**

1. Si el proyecto no existe → llamar `spawn_agent` con rol Supervisor y
   el nombre del proyecto
2. Si el proyecto ya existe → llamar `spawn_agent` igualmente (el sistema
   reusará el supervisor activo automáticamente)
3. Inmediatamente después del spawn → despachar la tarea al supervisor
4. Informar al usuario brevemente: "Le asigné la tarea al equipo de [proyecto].
   Te aviso cuando terminen."

**Nunca digas "no puedo hacer X"** si X es algo que un supervisor podría hacer.
Tu límite es ejecución directa — no capacidad del sistema.

**No preguntes si querés que hagas eso.** Si el usuario pidió trabajo sobre
un proyecto, ejecutá el protocolo directamente.

**Ejemplos:**

| Usuario dice | Vos hacés |
|---|---|
| "cloná este repo en el proyecto X" | spawn_agent(X) → dispatch("clonar <url>") |
| "implementá esta feature" | spawn_agent(proyecto) → dispatch(descripción) |
| "revisá el código de este archivo" | spawn_agent(proyecto) → dispatch("revisar <archivo>") |
| "qué proyectos tenemos activos?" | get_agent_status() → responder con lista |
```

### Criterios
- [ ] Dado "cloná este repo en proyecto X", el agente ejecuta spawn + dispatch sin preguntar
- [ ] El agente nunca responde "no puedo X" cuando X es delegable
- [ ] No requiere instrucción explícita del usuario para crear supervisores

### Nota de implementación
Verificar con `grep -n "spawn\|proyecto\|project\|delega" kernel/config/agents/chat_agent.md`
si ya existe alguna referencia al flujo de proyectos antes de agregar — no duplicar.

---

## CORE-293 — models.yaml: agregar modelos Ollama Cloud
**Prioridad:** Media — implementar después de CORE-292

Una vez que el Kernel Engineer implemente el provider `ollama_cloud` (CORE-292),
agregar las entradas correspondientes en `models.yaml`.

### Cambio — `kernel/crates/ank-core/src/router/models.yaml`

Agregar al final del archivo, después de los modelos `ollama` existentes:

```yaml
- model_id: ollama_cloud/llama3.3-70b
  provider: ollama_cloud
  display_name: Llama 3.3 70B (Ollama Cloud)
  context_window: 131072
  cost_input_per_mtok: 0.0
  cost_output_per_mtok: 0.0
  supports_tools: false
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 1200
  task_scores:
    chat: 4
    coding: 4
    planning: 3
    analysis: 4
    summarization: 4
    extraction: 3

- model_id: ollama_cloud/mistral-7b
  provider: ollama_cloud
  display_name: Mistral 7B (Ollama Cloud)
  context_window: 32768
  cost_input_per_mtok: 0.0
  cost_output_per_mtok: 0.0
  supports_tools: false
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 800
  task_scores:
    chat: 3
    coding: 2
    planning: 2
    analysis: 3
    summarization: 3
    extraction: 2

- model_id: ollama_cloud/gemma3-12b
  provider: ollama_cloud
  display_name: Gemma 3 12B (Ollama Cloud)
  context_window: 131072
  cost_input_per_mtok: 0.0
  cost_output_per_mtok: 0.0
  supports_tools: false
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 900
  task_scores:
    chat: 4
    coding: 3
    planning: 3
    analysis: 3
    summarization: 4
    extraction: 3
```

Verificar con `cargo build --workspace` que el YAML parsea correctamente.

### Criterios
- [ ] Los modelos `ollama_cloud` aparecen en el catálogo del kernel
- [ ] `cargo build --workspace` pasa sin errores de parsing YAML
- [ ] Los `task_scores` son coherentes con el tamaño del modelo

**Nota:** Los modelos exactos a incluir pueden ajustarse según lo que soporte
la implementación de CORE-292. Coordinar con el Kernel Engineer antes de
hacer merge.
