# CORE-320 — Kernel: CMR Scoring v3 — confiabilidad observada, tool-use degradado y pooling multi-key

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

El scoring del `CognitiveRouter` ignora tres señales que el sistema ya posee:

1. **Confiabilidad observada muerta.** `ModelUsageTracker` acumula `ModelOutcomes` (successes/failures por modelo) desde CORE-D2, pero `compute_score` nunca consume `success_rate()`. Un modelo que falla el 50% de las veces compite de igual a igual contra un hermano estable mientras su circuito esté cerrado.
2. **`ToolUseSupport::Degraded` no afecta el ruteo.** CORE-237 detecta en runtime que un modelo no puede llamar herramientas y lo marca `Degraded` en el catálogo, pero `compute_score` solo mira el flag estático `supports_tools`. Un subagente (que necesita tools) puede ser ruteado a un modelo que ya demostró no soportarlas.
3. **El free-tier no aprovecha múltiples keys.** `capacity_factor` aplica el RPM/RPD de *una* key, pero el `KeyPool` rota round-robin entre N keys gratuitas del mismo provider. Con 3 keys de Gemini free la capacidad real es 3× el límite; el router hoy se auto-limita a 1× y deprioriza modelos con headroom real.

## Solución propuesta

1. **Factor de confiabilidad** en `compute_score`: con ≥ 3 muestras, multiplicar por `0.5 + 0.5 × success_rate` (un modelo 100% estable conserva su score; uno que siempre falla retiene 50% — la exclusión dura sigue siendo trabajo de los circuit breakers). Outcomes se consultan fusionando ID de catálogo e ID bare del API (ver CORE-319.4).
2. **Tool-use efectivo**: `supports_tools && tool_use_support != Degraded` como señal para el factor chat/subagente — modelos degradados quedan al fondo del ranking para tareas de subagentes con la misma penalización que los sin tools.
3. **Pooling multi-key**: nuevo `KeyPool::count_available_free_keys(provider, model, tenant)`; `decide()` escala `free_tier_rpm/rpd × N` antes de pedir `capacity_factor`.

## Criterios de aceptación

- [x] Con ≥ 3 muestras, un modelo con failures recientes puntúa por debajo de un hermano equivalente estable; con < 3 muestras no hay penalización (sin datos ≠ malo).
- [x] Un modelo `Degraded` puntúa igual que uno sin `supports_tools` para tareas de subagente, y no cambia su score para Chat.
- [x] `count_available_free_keys` cuenta keys free activas y no rate-limitadas del tenant + globales, respetando `active_models`.
- [x] Con N keys free, el `capacity_factor` no penaliza hasta acercarse a N× el límite del modelo.
- [x] `cargo test -p ank-core --lib` en verde.
