# CORE-118 — Telemetría de tokens/seg y costo estimado en `SystemMetrics`

**Epic:** Epic 36 — Post-Launch Improvements (PLANNED)
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA — Operabilidad
**Estado:** TODO
**Origen:** REC-012 / Auditoría multi-modelo 2026-04-16

---

## Contexto

Las métricas actuales expuestas en `GET /api/status` incluyen estado del sistema,
uptime y métricas básicas. No incluyen métricas de throughput de inferencia ni
estimación de costo por sesión.

Estas métricas son útiles para:
1. **Operadores** que quieren entender el uso real del sistema
2. **CognitiveRouter** que podría usar latencia P95 real en lugar del valor
   hardcodeado `speed_inv = 0.5` (ver CORE-119)
3. **Usuarios** que quieren visibilidad sobre el costo de su uso

## Cambios requeridos

**Archivos:** `kernel/crates/ank-core/src/scheduler/mod.rs` o PCB
**Archivos:** `kernel/crates/ank-http/src/routes/` (handler de `/api/status`)
**Archivos:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

### Cambio 1 — Agregar campos al PCB para tracking de inferencia

```rust
pub struct PCB {
    // ... campos existentes ...
    pub inference_started_at: Option<Instant>,
    pub tokens_emitted: u32,
    pub model_used: Option<String>,
}
```

### Cambio 2 — Contar tokens en el HAL Runner

En el loop de streaming del HAL Runner:
```rust
while let Some(token) = stream.next().await {
    {
        let mut pcb = shared_pcb.write().await;
        pcb.tokens_emitted += 1;
    }
    // ... enviar token al event_broker ...
}
```

### Cambio 3 — Calcular TPS al completar la tarea

Al recibir `ProcessCompleted` en el scheduler o al cerrar el stream:
```rust
if let (Some(started), tokens) = (pcb.inference_started_at, pcb.tokens_emitted) {
    let elapsed = started.elapsed().as_secs_f64();
    let tps = tokens as f64 / elapsed;
    info!("PID {}: {} tokens in {:.2}s = {:.1} tok/s ({})",
          pcb.pid, tokens, elapsed, tps, pcb.model_used.as_deref().unwrap_or("unknown"));
}
```

### Cambio 4 — Agregar métricas acumuladas al `AppState`

```rust
pub struct InferenceMetrics {
    pub total_tokens: AtomicU64,
    pub total_requests: AtomicU64,
    pub avg_tps: Mutex<f64>, // rolling average
}
```

### Cambio 5 — Costo estimado

El `ModelCatalog` ya tiene información de modelos. Agregar campo `cost_per_1k_tokens_usd`
al modelo y calcularlo al completar cada PCB:

```rust
let cost = (pcb.tokens_emitted as f64 / 1000.0) * model.cost_per_1k_tokens_usd;
```

### Cambio 6 — Exponer en `/api/status`

```json
{
  "status": "operational",
  "uptime_seconds": 3600,
  "inference": {
    "total_requests": 42,
    "total_tokens": 18500,
    "avg_tokens_per_second": 45.2,
    "estimated_total_cost_usd": 0.037
  }
}
```

## Criterios de aceptación

- [ ] `GET /api/status` incluye `tokens_per_second` (promedio rolling de las últimas N inferencias)
- [ ] `GET /api/status` incluye `estimated_cost_usd` acumulado por sesión (reset en restart)
- [ ] Las métricas son atómicas y thread-safe (no requieren lock en el hot path)
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] La UI TelemetrySidebar muestra los nuevos campos (si el Shell Engineer lo considera)

## Dependencias

- Implementar antes de CORE-119 (`speed_inv` usa estas métricas)
- No bloquea el lanzamiento — es una mejora post-launch
