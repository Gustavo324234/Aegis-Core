# CORE-165 — feat: Model-per-Agent — Integración CMR con TaskType por AgentNode

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-158, CORE-155
**Estado:** ✅ Completo

---

## Contexto

El Cognitive Model Router (CMR, Epic 26) ya sabe seleccionar el mejor modelo disponible según el `TaskType` de un PCB. La integración con el sistema multiagente (ADR-AGENTS-005) consiste en que cada `AgentNode` tenga su propio `TaskType`, y que el `AgentOrchestrator` lo pase al CMR al ejecutar la inferencia de cada agente.

Esto garantiza que:
- Un `SpecialistAgent` de código (`TaskType::Code`) recibe el modelo con mejor `task_scores.code`
- Un `ProjectSupervisor` de planificación (`TaskType::Planning`) recibe el modelo con mejor razonamiento
- El KeyPool global se comparte — no hay duplicación de keys

Este ticket es principalmente de **glue code** entre CORE-158 y el CMR existente.

---

## Cambios Requeridos

### En `AgentOrchestrator::run_agent_loop` (CORE-158)

Cuando el loop va a ejecutar inferencia, debe llamar al CMR con el `task_type` del agente:

```rust
// En run_agent_loop, antes de llamar al HAL:

let node = tree.read().await;
let agent_node = node.get(&agent_id).ok_or_else(|| anyhow::anyhow!("Agent not found"))?;
let task_type = agent_node.task_type.clone();
drop(node);

// Delegar al CMR la selección de modelo (lógica ya existente en Epic 26)
let routing_decision = router.select_model_for_task(&task_type, &tenant_id).await?;

// Usar el modelo seleccionado para la inferencia via HAL
let result = chal.infer_with_model(&routing_decision.model_id, &prompt, context_budget).await?;
```

El método `router.select_model_for_task()` puede ser el método existente del CMR o un wrapper ligero. **Verificar la API exacta del CognitiveRouter antes de implementar.**

### System prompts por TaskType

Agregar en la configuración del `AgentOrchestrator` (o en un archivo de configuración YAML) los system prompts base por tipo de agente:

```yaml
# ank-core/config/agent_prompts.yaml
project_supervisor: |
  Eres el supervisor del proyecto {project_name}. Tu trabajo es:
  1. Entender el objetivo global dado por el usuario
  2. Descomponerlo en tareas para tus supervisores de dominio
  3. Agregar los reportes recibidos y presentar un resumen ejecutivo
  Tu respuesta DEBE estar en JSON estructurado con campos: 
  "plan", "dispatches" (array de tareas), "summary"

domain_supervisor: |
  Eres el supervisor del dominio {domain} en el proyecto {project_name}.
  Tu trabajo es coordinar a tus agentes especialistas.
  Recibirás una tarea de tu supervisor de proyecto y la dividirás en sub-tareas.
  Tu respuesta DEBE estar en JSON con campos: "task_breakdown", "summary"

specialist: |
  Eres un agente especialista en {domain} para el proyecto {project_name}.
  Tu trabajo es ejecutar una tarea específica y reportar el resultado.
  Sé preciso y técnico. Incluye el código o los cambios exactos.
  Tu respuesta DEBE estar en JSON con campos: "result", "artifacts", "status"
```

### Logging de routing decisions

El log de cada agente debe incluir el modelo seleccionado:

```
[SPECIALIST][scheduler][CORE-165] → modelo seleccionado: claude-3-5-sonnet (score: 0.87, task: CODE)
```

---

## Criterios de Aceptación

- [x] Cada agente usa el modelo seleccionado por el CMR según su `task_type`, no un modelo hardcodeado
- [x] Si el CMR no puede seleccionar modelo (KeyPool vacío, sin keys válidas), el agente falla con `AgentState::Failed` y el error se propaga hacia arriba
- [x] El log de cada inferencia incluye el modelo seleccionado y el score
- [x] Los system prompts base están externalizados (no hardcodeados en el código Rust)
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*