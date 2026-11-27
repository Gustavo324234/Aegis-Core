# CORE-409: CMR v4 — Complejidad Adaptativa de Chat (`Quick`/`Standard`/`DeepReasoning`), `benchmark_pct` y `max_tokens` para Razonadores

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 62
- **Asignado a:** Kernel Engineer + AI Architect

---

## Description

Permitir que el Chat Agent escale automáticamente a modelos de mayor razonamiento en preguntas complejas sin perder velocidad/ahorro en tareas rápidas:
- Agregar `ChatComplexity` (`Quick`, `Standard`, `DeepReasoning`) y `infer_chat_complexity` en `ank-core/src/pcb.rs`, propagándolo en `ws/chat.rs` y `StickyKey` del router.
- Agregar `benchmark_pct: Option<f64>` y `supports_reasoning: bool` en `ModelEntry` (`router/catalog.rs`) y usar `normalized_quality_for` en `router/scoring.rs` para desempatar modelos con precisión continua.
- Eliminar la penalización ciega `0.05` en `TaskType::Chat` cuando la complejidad es `DeepReasoning` y evitar overrides a modelos tiny/LocalOnly en consultas complejas.
- Ajustar `recommended_max_tokens` en `chal/drivers/cloud.rs` (hasta 4096 para modelos razonadores o turnos complejos, preservando auto-clamp HTTP 402 en OpenRouter).

## Acceptance Criteria

- [x] Consultas `DeepReasoning` priorizan calidad/análisis sobre modelos ultra-baratos y nunca quedan atrapadas en sticky cache de turnos `Quick`.
- [x] `benchmark_pct` diferencia modelos de 91%+ frente a modelos de ~80% cuando ambos tienen `task_score == 5`.
- [x] `max_tokens` no trunca respuestas de modelos con razonamiento en OpenRouter/Anthropic.
