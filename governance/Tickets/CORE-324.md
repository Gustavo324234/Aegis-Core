# CORE-324 — Kernel: CMR scoring v3.1 — EWMA de latencia, half-open breaker y costo ponderado por tarea

**Tipo:** feat
**Prioridad:** Media
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

1. **Latencia con promedio plano:** la media de las últimas 20 muestras reacciona lento a un provider degradándose y un outlier la distorsiona por 20 requests.
2. **Circuit breaker sin half-open:** un circuito abierto bloquea TODO el tráfico del provider hasta que la ventana desliza, aunque el provider ya se haya recuperado.
3. **Costo plano input+output:** las tareas de output largo (Code/Planning/Creative) generan varias veces más tokens de salida que de entrada; sumar plano subestima sistemáticamente a los modelos con output caro (Opus: $75/Mtok out vs $15 in).

## Solución implementada

1. **EWMA (α=0.3)** en `observed_latency_ms` y en las stats — el estimador reacciona en pocas muestras a la degradación y un outlier solo lo empuja un 30%.
2. **Half-open canary** (`provider_circuit_allows_probe`): con el circuito abierto pero ≥15s sin fallos nuevos, `decide()` deja pasar UN candidato como sonda cada 10s. Si la sonda falla, la ventana se refresca; si responde, el tráfico ya fluye mientras el circuito envejece.
3. **`task_weighted_cost()`**: output ×3 para Code/Planning/Creative en el término de costo del scoring (y en la normalización `max_cost` de `decide()`).

## Descope documentado

**Sticky por sesión/conversación:** el PCB no transporta un ID de conversación estable (el `pid` es por turno y `session_key` es por login), así que el sticky cache sigue siendo por (tenant, task, pref). Requiere agregar un `conversation_id` al PCB y al protocolo WS — ticket futuro.

## Criterios de aceptación

- [x] Una ráfaga de muestras lentas mueve el estimador de latencia por encima del promedio histórico (test `observed_latency_is_recency_weighted`).
- [x] La sonda half-open NO se permite con fallos frescos (<15s) ni sin historial (test `half_open_probe_blocked_while_failures_fresh`).
- [x] Las policies CostOptimized/QualityOptimized siguen ordenando correctamente con el costo ponderado (test `test_routing_policy_scoring`).
- [x] `cargo test -p ank-core --lib` en verde.
