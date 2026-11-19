# CORE-325 — Kernel: deduplicar el feed de señales del fallback walk en chal/mod.rs

**Tipo:** chore
**Prioridad:** Media
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

La caminata de fallbacks de `chal/mod.rs` (primario → rotación de keys → fallback chain, en sus variantes de error HTTP y stream vacío) tenía el bloque "alimentar el tracker del router" inlineado en **9 sitios**, con inconsistencias entre ellos: algunos registraban el circuito por-modelo en fallos permanentes (401/tier-0) y otros no; algunos registraban el failure junto al empty-response y otros solo el empty. Cada bugfix del feed había que replicarlo a mano en todos los sitios.

## Solución implementada

Dos helpers privados en `CognitiveHAL`:

- `note_model_failure(model, provider, err)` — scoring window + circuito de provider siempre; circuito por-modelo cuando `is_permanent_model_failure(err)`.
- `note_empty_response(model, provider)` — empty-response circuit + failure window.

Los 9 sitios delegan en ellos. Cambio de comportamiento deliberado (homogeneización): las ramas que antes omitían el chequeo de fallo permanente o el failure junto al empty ahora lo aplican — la dirección consistente con el diseño de CORE-FIX (D) y (F).

## Criterios de aceptación

- [x] Cero llamadas directas a `record_failure`/`record_empty_response`/`record_model_unavailable` inlineadas en el fallback walk — todo pasa por los helpers.
- [x] `cargo test -p ank-core --lib` en verde (incluye la suite de agents y chal).
