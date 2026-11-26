# CORE-323 — Catálogo: curaduría de models.yaml (modelos retirados y IDs corregidos)

**Tipo:** chore
**Prioridad:** Media
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Arquitecto IA

---

## Problema

El catálogo bundleado arrastra entradas muertas o erróneas que desperdician slots del ranking y producen 404 garantizados:

- `anthropic/claude-opus-4-6` no existe (el ID real de la familia es `claude-opus-4-8`).
- `groq/mixtral-8x7b` fue decomisionado por Groq.
- `gemini-1.5-pro` / `gemini-1.5-flash` retirados por Google para proyectos nuevos.
- `openai/o3-mini` reemplazado por `o4-mini`.
- `meta-llama/llama-3.1-70b-instruct` (OpenRouter) con todos los scores en 1 — peso muerto.

## Solución implementada

- Opus actualizado a `anthropic/claude-opus-4-8`; agregado `anthropic/claude-haiku-4-5` (tier rápido/barato).
- `openai/o3-mini` → `openai/o4-mini`; agregado `openai/gpt-4.1` (1M ctx, coding 5).
- Eliminados: `groq/mixtral-8x7b` (reemplazado por `qwen/qwen3-32b` servido por Groq, con tools), `gemini-1.5-pro`, `gemini-1.5-flash`, `meta-llama/llama-3.1-70b-instruct`.

Nota: las keys con `active_models` descubiertos siguen gateando qué modelos son invocables realmente, así que las adiciones son seguras incluso si el provider del usuario no las sirve.

## Criterios de aceptación

- [x] El catálogo no contiene IDs decomisionados conocidos.
- [x] Tests de catálogo (≥ 15 modelos, perfiles cloud/local/hybrid) en verde.
