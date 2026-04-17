# CORE-106 — Latencia Real en CognitiveRouter (reemplazar speed_inv hardcodeado)

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/router/mod.rs` + `kernel/proto/models.yaml` (o equivalente)  
**Agente:** Kernel Engineer  
**Prioridad:** P2 — Routing inteligente  
**Estado:** TODO  
**Origen:** REC-013 / big-pickle DEBT-005 + claude-sonnet-4-6 sección 2.5

---

## Contexto

El `CognitiveRouter` usa scoring 40/30/20/10 (calidad/costo/velocidad/disponibilidad).
El factor de velocidad (`speed_inv`) está hardcodeado a `0.5` constante en
`compute_score()` (línea ~180). Esto hace que el 20% del peso dedicado a latencia
sea efectivamente neutro: todos los modelos tienen el mismo score de velocidad.
El router no puede diferenciar entre modelos rápidos y lentos.

---

## Cambios requeridos

1. Agregar campo `avg_latency_ms: Option<u32>` a la struct del modelo en
   `ModelCatalog`. Poblar con valores aproximados en el catálogo bundled
   (`models.yaml` o equivalente) basados en benchmarks públicos conocidos.
   Valores de referencia orientativos:
   - `gpt-4o-mini`: ~600ms
   - `claude-haiku-*`: ~800ms
   - `gpt-4o`: ~1500ms
   - `claude-sonnet-*`: ~1800ms
   - `claude-opus-*`: ~3000ms

2. En `compute_score()`, reemplazar `let speed_inv = 0.5` por:

   ```rust
   let speed_score = match model.avg_latency_ms {
       Some(ms) if ms > 0 => 1.0 / (ms as f64 / 1000.0),
       _ => 0.5, // fallback si no hay dato
   };
   ```

3. Si CORE-105 está DONE: en el `MetricsCollector`, actualizar `avg_latency_ms`
   del modelo en el catálogo en memoria usando exponential moving average tras
   cada inferencia completada. No persistir en disco — se recalcula por sesión.

4. Si CORE-105 no está DONE aún: implementar solo los pasos 1 y 2.
   El paso 3 se completa cuando CORE-105 esté DONE.

---

## Criterios de aceptación

- [ ] `speed_inv = 0.5` hardcodeado eliminado de `compute_score()`
- [ ] `ModelCatalog` tiene `avg_latency_ms: Option<u32>` con valores en el catálogo bundled
- [ ] El scoring de velocidad varía entre modelos distintos
- [ ] Si `avg_latency_ms` es `None`, el comportamiento es idéntico al actual (fallback 0.5)
- [ ] `cargo build -p ank-core` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

- CORE-105 (telemetría) para la parte de latencia dinámica — opcional para los pasos 1 y 2.
