# CORE-127 — Fix: Catálogo de modelos Gemini desactualizado

**Status:** DONE — 2026-04-20

## Síntoma

El catálogo bundled solo tenía `google/gemini-2.0-flash`. Google lanzó
Gemini 2.5 Pro y Gemini 2.5 Flash que no aparecían en la UI.

## Fix en `models.yaml`

Agregados:

```yaml
- model_id: "google/gemini-2.5-pro"
  provider: "google"
  display_name: "Gemini 2.5 Pro"
  context_window: 1048576
  cost_input_per_mtok: 1.25
  cost_output_per_mtok: 10.0
  supports_tools: true
  supports_json_mode: true
  is_local: false
  avg_latency_ms: 2000
  task_scores: chat:5 coding:5 planning:5 analysis:5 summarization:5 extraction:5

- model_id: "google/gemini-2.5-flash"
  provider: "google"
  display_name: "Gemini 2.5 Flash"
  context_window: 1048576
  cost_input_per_mtok: 0.15
  cost_output_per_mtok: 0.6
  supports_tools: true
  supports_json_mode: true
  is_local: false
  avg_latency_ms: 600
  task_scores: chat:5 coding:4 planning:4 analysis:4 summarization:5 extraction:4
```

`gemini-2.0-flash` se mantiene para compatibilidad con keys existentes.

## Nota

El test `test_load_bundled_not_empty` requiere `>= 15 modelos` — el catálogo
ahora tiene 17 entries, el test sigue pasando.
