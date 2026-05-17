# BRIEFING — Kernel Engineer
## CORE-301: CMR v2 — scoring contextual + latencia real
**Fecha:** 2026-05-13
**Branch:** `feat/core-301-cmr-v2-scoring`

---

## Contexto rápido

El `compute_score` actual tiene un peso fantasma (`1.0_f64 * 0.30`) que hace que
el 30% del score sea idéntico para todos los modelos. El CMR no ve el prompt,
no usa latencias reales, y no penaliza modelos con errores recientes.

Este ticket lo corrige con 4 cambios quirúrgicos en 2 archivos.

---

## Archivo 1: `kernel/crates/ank-core/src/router/rate_tracker.rs`

**Agregar dos campos al struct:**
```rust
latency_samples: Arc<RwLock<HashMap<String, VecDeque<u32>>>>,
error_window: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
```

**Inicializar en `new()`:**
```rust
latency_samples: Arc::new(RwLock::new(HashMap::new())),
error_window: Arc::new(RwLock::new(HashMap::new())),
```

**Agregar 4 métodos** (ver CORE-301.md para el código completo):
- `record_latency(model_id, latency_ms)` — guarda las últimas 20 latencias
- `record_error(model_id)` — registra error en ventana de 5 min
- `observed_latency_ms(model_id) -> Option<u32>` — promedio de latencias
- `recent_errors(model_id) -> u32` — errores en últimos 5 min

---

## Archivo 2: `kernel/crates/ank-core/src/router/mod.rs`

**Cambio A — nueva función `detect_content_type`** (agregar antes de `compute_score`):
Analiza el prompt con señales lexicales simples y devuelve un boost (0.0–0.30).
Ver código completo en CORE-301.md.

**Cambio B — reemplazar `compute_score`** con la versión que recibe:
```rust
fn compute_score(
    entry, task_type, prompt: &str,
    max_cost, max_latency,
    observed_latency: Option<u32>,
    recent_errors: u32,
) -> f64
```

Nueva fórmula (sin peso fantasma):
```
quality * 0.40 + cost_inv * 0.25 + speed_inv * 0.20 + context_fit * 0.15
multiplicado por (1.0 - error_penalty)
```

**Cambio C — actualizar el cálculo de `max_latency`** para usar latencia observada:
```rust
let obs = self.tracker.observed_latency_ms(&e.model_id).await;
let lat = obs.unwrap_or(e.avg_latency_ms.unwrap_or(2000)) as f64;
```

**Cambio D — actualizar la llamada a `compute_score`** en el loop de scoring:
```rust
let observed_lat = self.tracker.observed_latency_ms(&entry.model_id).await;
let errors = self.tracker.recent_errors(&entry.model_id).await;
let base = self.compute_score(
    &entry, task_type, &pcb.l1_instruction,
    max_cost, max_latency, observed_lat, errors,
);
```

**Cambio E — exponer el tracker:**
```rust
pub fn tracker_ref(&self) -> &Arc<ModelUsageTracker> {
    &self.tracker
}
```

---

## Archivo 3 (opcional si el tiempo lo permite): driver HTTP

En el driver que hace requests al provider (buscar `reqwest::Client::post` o similar),
capturar la latencia y llamar a `tracker.record_latency()` y `tracker.record_error()`.
Si no es evidente dónde hacerlo, dejarlo para un follow-up — los cambios A-E ya
son valiosos por sí solos.

---

## Criterios mínimos

- [ ] `cargo build --workspace` pasa
- [ ] `test_decide_returns_decision_for_chat` sigue pasando
- [ ] `compute_score` no tiene el `1.0_f64 * 0.30` hardcodeado

## Commit
```
feat(ank-core): CORE-301 CMR v2 — contextual scoring, real latency, error penalty
```

**No pushear a main. Abrir PR.**
