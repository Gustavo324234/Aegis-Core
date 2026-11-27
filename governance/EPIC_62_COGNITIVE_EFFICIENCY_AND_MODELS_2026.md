# EPIC 62 — Cognitive Efficiency, Adaptive Intelligence & 2026 Model Matrix

**Estado:** ✅ Completa

## 🎯 Objetivo

Reducir drásticamente el consumo de tokens por turno conversacional (-60% a -75%) y elevar simultáneamente la inteligencia percibida de Aegis OS mediante:
1. Higiene del repositorio, modularización focalizada de God-Files (`router/scoring.rs`, `chal/context_builder.rs`) y aislamiento limpio de tests en `enclave/mod.rs`.
2. **Dynamic Tool Gating** (inyección Just-in-Time por dominio), arquitectura de prompt **Cache-First** (prefijo de sistema invariante entre minutos) y memoria personal selectiva + poda de outputs antiguos en bucles ReAct.
3. **CMR v4**: clasificación adaptativa de complejidad conversacional (`Quick` / `Standard` / `DeepReasoning`), puntajes continuos (`benchmark_pct` de PinchBench) y presupuesto dinámico de `max_tokens` para modelos razonadores.
4. Curaduría 2026 de `models.yaml` (mejores modelos pagos costo-eficientes y gratuitos con `tools`), sincronizador actualizado y presets alineados en `shell/ui` y `agent_sandbox`.
5. Suite de verificación de presupuesto de tokens, caché, ruteo inteligente y documentación actualizada.

---

## 🏗️ Tickets de la Épica

1. **`CORE-407`** (`chore`, ✅ Done): Higiene del Repo, Modularización Focalizada (`router/scoring.rs`, `chal/context_builder.rs`) y Mapa de Arquitectura.
2. **`CORE-408`** (`perf`, ✅ Done): Dynamic Tool Gating, Prompt Cache-First, Deduplicación de `chat_agent.md` y Memoria Selectiva.
3. **`CORE-409`** (`feat`, ✅ Done): CMR v4 — Complejidad Adaptativa de Chat (`Quick`/`Standard`/`DeepReasoning`), `benchmark_pct` y `max_tokens` para Razonadores.
4. **`CORE-410`** (`feat`, ✅ Done): Catálogo Curado 2026 (`models.yaml`), `update_models.py` y Presets Recomendados en UI/Sandbox.
5. **`CORE-411`** (`test`, ✅ Done): Suite de Regresión de Tokens/Inteligencia, Documentación PinchBench/README y Verificación SRE Completa.
