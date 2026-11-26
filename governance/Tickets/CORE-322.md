# CORE-322 — Kernel: persistencia del ModelUsageTracker y endpoint /api/router/stats

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

1. **El tracker del router es 100% in-memory.** Un reinicio del kernel borra los contadores diarios del free tier (RPD) — el router puede reventar la cuota diaria de Gemini justo después de un boot — y resetea a cero los outcomes de confiabilidad que alimenta el scoring de CORE-320.
2. **La telemetría del router es invisible para la UI.** El `top3` del scoring, los cooldowns de circuitos y los outcomes solo van a logs del servidor; la UI no puede responder "¿por qué eligió este modelo?" ni mostrar countdowns de retry reales.

## Solución implementada

1. **Persistencia (rate_tracker.rs + mod.rs):** `TrackerSnapshot` (daily_counts + outcomes) serializado como PCB sintético `router_tracker:global` por el mismo canal que usa el KeyPool. `CognitiveRouter::spawn_tracker_persistence()` restaura al boot (los contadores con fecha vieja se descartan; los outcomes se fusionan aditivamente) y persiste cada 60s solo si hubo cambios (flag `dirty`). Wired en `ank-server/main.rs`.
2. **Endpoint `GET /api/router/stats`** (autenticado vía Citadel): outcomes por modelo con success_rate, latencia observada (EWMA), circuitos abiertos por provider/modelo con segundos de cooldown, y decisiones sticky activas. Libre de secretos por construcción.
3. **Hardening CORE-212:** `KeyPool::load()` normaliza provider ids legacy ("google" → "gemini") al cargar keys persistidas antes del fix de normalización en escritura.

## Criterios de aceptación

- [x] Tras un restore, los contadores RPD de hoy reducen `capacity_factor`; los de fechas pasadas no (test `snapshot_restore_roundtrip`).
- [x] Los outcomes sobreviven el roundtrip snapshot → restore.
- [x] `GET /api/router/stats` responde con outcomes, circuitos y sticky decisions sin exponer api_keys.
- [x] `cargo test -p ank-core --lib` en verde.
