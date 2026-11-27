# CORE-410: Catálogo Curado 2026 (`models.yaml`), `update_models.py` y Presets Recomendados en UI/Sandbox

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 62
- **Asignado a:** AI Architect + Shell Engineer

---

## Description

Actualizar el catálogo embebido de modelos y los puntos de onboarding para reflejar los mejores modelos pagos y gratuitos de 2026:
- Depurar entradas obsoletas o retiradas en `kernel/crates/ank-core/src/router/models.yaml` e incorporar los modelos validados con PinchBench y OpenRouter (DeepSeek V4/V4.1 Flash, GPT-5.6 Luna, Qwen 3.6/3.7 Flash & Max, Claude 4.6/5.5, Gemini 3.5/3.6/3.8 Flash & Lite, Nemotron 3 Ultra `:free`, Nemotron 3.5 `:free`, Gemma 4 `:free`, Groq GPT-OSS & Qwen 3.8 27B), asegurando `free_tier_eligible: false` explícito en todo modelo pago.
- Actualizar `tools/update_models.py` para sincronizar también `benchmark_pct`.
- Alinear presets y guías en `enginePresets.ts`, `PersonalQuickSetup.tsx`, `ModelCatalogViewer.tsx` y `tools/agent_sandbox/sandbox.py`.

## Acceptance Criteria

- [x] Todos los modelos pagos no-free-tier tienen `free_tier_eligible: false` explícito.
- [x] `PersonalQuickSetup.tsx` y `enginePresets.ts` ofrecen defaults funcionales (incluyendo DeepSeek, `openrouter/free` y `openai/gpt-oss-120b`).
- [x] `ModelCatalogViewer.tsx` aprovecha `benchmark_pct` cuando está presente.
