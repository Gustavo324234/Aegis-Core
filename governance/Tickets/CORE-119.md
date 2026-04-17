# CORE-119 — Reemplazar `speed_inv = 0.5` hardcodeado en `CognitiveRouter`

**Epic:** Epic 36 — Post-Launch Improvements (PLANNED)
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA — Routing quality
**Estado:** TODO
**Origen:** REC-013 / Auditoría multi-modelo 2026-04-16

---

## Contexto

El `CognitiveRouter` usa un scoring multi-criterio con pesos 40/30/20/10 para
seleccionar el modelo óptimo. El 20% de peso dedicado a latencia usa un factor
`speed_inv = 0.5` hardcodeado, constante para todos los modelos y proveedores.

Esto neutraliza efectivamente la dimensión de velocidad en las decisiones de
routing: todos los modelos reciben el mismo score de latencia, haciendo que el
router seleccione siempre por calidad y costo (los otros 70% del peso).

Para tareas latency-sensitive (Siren, autocomplete, respuestas cortas), el router
debería preferir modelos más rápidos aunque sean marginalmente menos precisos.

**Archivo afectado:** `kernel/crates/ank-core/src/router/mod.rs` línea ~180

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/router/mod.rs`
**Archivo:** `kernel/crates/ank-core/src/router/catalog.rs` (o donde esté `ModelCatalog`)

### Fase 1 — Valores estáticos por proveedor en el catálogo (interim)

Mientras no haya medición dinámica (CORE-118), usar valores diferenciados por
proveedor en el catálogo bundled:

```yaml
# models.yaml — agregar campo avg_latency_ms por modelo
- id: "gpt-4o"
  provider: "openai"
  avg_latency_ms: 800    # P50 observado empíricamente
- id: "claude-3-haiku"
  provider: "anthropic"
  avg_latency_ms: 400
- id: "llama-3-70b"
  provider: "groq"
  avg_latency_ms: 150    # Groq es significativamente más rápido
```

### Fase 2 — Usar `avg_latency_ms` en `compute_score()`

```rust
fn compute_score(&self, model: &ModelInfo, task: &TaskProfile) -> f64 {
    let quality_score = model.quality_score; // 0.0 - 1.0
    let cost_score = 1.0 - model.cost_normalized; // inverso del costo
    let speed_score = if model.avg_latency_ms > 0 {
        // Normalizar: 100ms = 1.0, 2000ms = 0.0
        (1.0 - (model.avg_latency_ms as f64 / 2000.0)).max(0.0)
    } else {
        0.5 // fallback si no hay dato
    };
    let context_score = /* ... lógica existente ... */;

    (quality_score * 0.40)
        + (cost_score * 0.30)
        + (speed_score * 0.20)  // Reemplaza speed_inv = 0.5
        + (context_score * 0.10)
}
```

### Fase 3 — Actualización dinámica desde métricas reales (dependiente de CORE-118)

Una vez que CORE-118 registra TPS y latencia por inferencia, el `CatalogSyncer`
puede actualizar `avg_latency_ms` en background:

```rust
// En el background syncer
fn update_latency_from_metrics(&mut self, model_id: &str, observed_latency_ms: u64) {
    if let Some(model) = self.catalog.get_mut(model_id) {
        // Exponential moving average: alpha = 0.1
        model.avg_latency_ms = (model.avg_latency_ms as f64 * 0.9
            + observed_latency_ms as f64 * 0.1) as u64;
    }
}
```

## Criterios de aceptación

- [ ] `speed_inv = 0.5` está eliminado del código
- [ ] `models.yaml` tiene `avg_latency_ms` para al menos los modelos tier-1 del catálogo bundled
- [ ] `compute_score()` usa `avg_latency_ms` normalizado en el cálculo del 20% de velocidad
- [ ] Modelos en Groq/local reciben mayor speed_score que modelos en providers más lentos
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] Tests del router verifican que el score de velocidad varía entre modelos

## Dependencias

- Implementar Fase 1+2 sin depender de CORE-118
- Fase 3 depende de CORE-118 (métricas dinámicas)
- No bloquea el lanzamiento
