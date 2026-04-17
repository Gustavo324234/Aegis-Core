# CORE-105 — Telemetría de Tokens por Segundo y Costo Estimado

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/` + `kernel/crates/ank-http/src/routes/` + `shell/ui/`  
**Agente:** Kernel Engineer (backend) + Shell Engineer (UI)  
**Prioridad:** P2 — Operabilidad  
**Estado:** TODO  
**Origen:** REC-012 / Gemini 3 Flash

---

## Contexto

Las métricas actuales del endpoint `/api/status` no incluyen throughput de inferencia
ni costo por sesión. Esta información es valiosa para operadores y es prerequisito
para resolver `speed_inv` hardcodeado en el `CognitiveRouter` (CORE-106).

---

## Cambios requeridos

### Kernel Engineer

1. En el `PCB`, registrar timestamps de inicio/fin de inferencia y tokens emitidos:

   ```rust
   pub struct PCBMetrics {
       pub started_at: Option<Instant>,
       pub completed_at: Option<Instant>,
       pub tokens_emitted: u32,
       pub model_id: String,
   }
   ```

2. En el HAL Runner, incrementar `tokens_emitted` en cada chunk del stream.
   Registrar `started_at` al comenzar y `completed_at` al terminar.

3. Agregar un rolling window de las últimas 10 inferencias completadas en `AppState`
   para calcular:
   - `tokens_per_second`: promedio de tokens/s de los últimos N PCBs completados
   - `total_tokens_session`: tokens totales desde inicio del servidor
   - `estimated_cost_usd`: calculado con el precio por token del modelo en
     `ModelCatalog`. Si el precio no está disponible, omitir (`null`), no hardcodear.

4. Exponer estos valores en `GET /api/status` como campos nuevos en `SystemMetrics`.

### Shell Engineer

5. En el `TelemetrySidebar`, consumir y mostrar `tokens_per_second` y
   `estimated_cost_usd` si están presentes en la respuesta de `/api/status`.

---

## Criterios de aceptación

- [ ] `GET /api/status` incluye `tokens_per_second` (f64) y `estimated_cost_usd` (Option<f64>)
- [ ] `tokens_per_second` es promedio rolling de las últimas inferencias completadas
- [ ] `estimated_cost_usd` es `null` si el modelo no tiene precio en catálogo (no falla)
- [ ] `TelemetrySidebar` muestra los nuevos campos si están presentes
- [ ] `cargo build --workspace` sin errores ni warnings de clippy
- [ ] `npm run build` en `shell/ui` sin errores TypeScript
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

- `ModelCatalog` debe exponer precio por token por modelo como `Option<f64>`.
  Verificar si ya existe; si no, agregar ese campo.
