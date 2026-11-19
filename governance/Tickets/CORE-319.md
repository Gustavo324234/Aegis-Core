# CORE-319 — Kernel: CMR Router hardening — sticky cache, señales del tracker y catálogo

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Auditoría del `CognitiveRouter` (`kernel/crates/ank-core/src/router/`) detectó seis defectos de estado que degradan el ruteo en producción:

1. **Sticky cache nunca pega tras un override de preferencia.** `decide()` busca en el caché con la `model_pref` original del PCB, pero inserta la decisión bajo la `effective_pref` (que cambia cuando el light-task override fuerza `LocalOnly`, o cuando `LocalOnly` sin candidatos degrada a `HybridSmart`). Resultado: cada turno de un chat trivial ("hola") recalcula el ruteo completo y puede saltar de modelo, exactamente lo que el sticky cache debía evitar.
2. **El sticky reuse no valida el circuito por-modelo.** Solo consulta `is_provider_circuit_open`; un modelo con circuito abierto (401 / respuestas vacías) se sigue reutilizando hasta 30s.
3. **El mapa sticky crece sin límite.** Las entradas expiradas solo se eliminan si se vuelve a consultar exactamente la misma clave; tenants efímeros acumulan entradas para siempre.
4. **Mismatch del espacio de IDs entre tracker y router.** La capa CHAL registra señales (latencia observada, errores recientes, outcomes, circuito por-modelo) bajo el `model_id` *bare* del API (`decision.model_id`, p. ej. `llama-3.3-70b-versatile`), pero `decide()` consulta el tracker con el ID del catálogo (`groq/llama-3.3-70b-versatile`). Para todo modelo con prefijo de provider, el circuito por-modelo, la penalización por errores y la latencia observada **nunca aplican**: el router queda ciego a sus propias señales.
5. **`update_tool_use_support` hace read-modify-write del catálogo completo.** Clona todas las entradas, muta una y llama `replace_all`, lo cual (a) pierde modelos si el `CatalogSyncer` agrega entradas concurrentemente y (b) falsifica `last_synced` como si hubiera habido un sync.
6. **`get_candidates` excluye score < 3 sin fallback.** Un deployment con perfil `Local` y solo modelos pequeños (planning/coding score 1–2) recibe `ModelNotFound` para Planning/Code aunque haya modelos funcionando.

## Solución propuesta

1. Insertar la decisión sticky bajo la `model_pref` original del PCB (la misma que usa el lookup).
2. Validar también `is_model_circuit_open(decision.model_id)` antes de reutilizar una decisión sticky.
3. Podar entradas expiradas del mapa sticky en cada inserción.
4. Consultar el tracker con ambos IDs (catálogo y bare del API) y fusionar señales: circuito abierto si cualquiera está abierto, errores = suma, latencia = la observada disponible.
5. Nuevo `ModelCatalog::update_tool_support()` in-place con write-lock que no toca `last_synced`; `CognitiveRouter::update_tool_use_support` delega.
6. Si el filtro score ≥ 3 queda vacío, `get_candidates` degrada con gracia devolviendo todas las entradas ordenadas por score (graceful degradation, principio Zero-Panic).

## Criterios de aceptación

- [x] El sticky cache pega en turnos consecutivos aun cuando el light-task override cambia la preferencia efectiva (test de regresión).
- [x] Una decisión sticky cuyo modelo tiene el circuito por-modelo abierto se descarta y se re-rutea.
- [x] El mapa sticky no retiene entradas expiradas tras nuevas inserciones.
- [x] Las señales registradas bajo el ID bare del API afectan el scoring/circuitos de entradas de catálogo con prefijo (test de regresión con `groq/...`).
- [x] `update_tool_use_support` concurrente con `add_entry` no pierde modelos ni altera `last_synced`.
- [x] Perfil `Local` con modelos de score < 3 sigue ruteando (degradación, no `ModelNotFound`).
- [x] `cargo test -p ank-core --lib` en verde.
