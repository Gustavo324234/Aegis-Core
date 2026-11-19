# CORE-328 — Kernel: confiabilidad task-aware — outcomes por (modelo, tarea)

**Tipo:** feat
**Prioridad:** Media
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

El factor de confiabilidad del scoring (CORE-320) usa outcomes globales por modelo. Pero un modelo puede ser sólido en Chat y fallar consistentemente los turnos de Code con tools (el patrón real de varios modelos free-tier): la señal global promedia ambos y esconde el problema — el router sigue mandándole tareas de Code a un modelo que demostró no poder con ellas.

## Solución implementada

1. **Tracker**: `record_success_for_task` / `record_failure_for_task` tallan el outcome global Y una entrada compuesta `"{model}::task::{Task}"` en el mismo mapa de outcomes — persiste vía el snapshot de CORE-322 y aparece en `/api/router/stats` con clave autodescriptiva, sin migración de esquema.
2. **CHAL**: los helpers de CORE-325 (`note_model_failure` / `note_empty_response`) y el sitio de éxito reciben `pcb.task_type` — un solo punto de cambio gracias al dedupe previo.
3. **Router**: `outcomes_merged_for_task` prefiere la señal específica de la tarea cuando tiene ≥ 3 muestras (mismo umbral del scorer) y cae a la global cuando no — sin datos por tarea, el comportamiento es idéntico al de CORE-320.

## Criterios de aceptación

- [x] Un fallo de Code talla el global y el compuesto Code, sin tocar Chat (test `task_outcomes_tally_global_and_composite`).
- [x] Con ≥ 3 muestras de la tarea, el scorer usa la señal específica; tareas sin muestras caen a la global (test `test_task_aware_outcomes_override_global`).
- [x] Las entradas compuestas sobreviven snapshot/restore (mismo mapa persistido de CORE-322).
- [x] `cargo test -p ank-core --lib` en verde.
