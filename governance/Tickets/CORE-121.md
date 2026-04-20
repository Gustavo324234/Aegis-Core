# CORE-121 — Fix: agregar `openrouter/free` al catálogo de modelos

## Contexto

Durante el smoke test del 2026-04-20, el CognitiveRouter fallaba con
`Hardware Failure: Reqwest error after 2 retries: error sending request for url (http://localhost:11434/v1/chat/completions)`
en servidores sin Ollama.

**Causa raíz:** el model_id `openrouter/free` no existía en `models.yaml`. El
KeyPool tenía una key de OpenRouter con `active_models: ["openrouter/free"]`,
pero el Router nunca podía matchearla con ningún entry del catálogo. Sin key
disponible para modelos cloud, el scoring caía a los modelos locales de Ollama
(costo 0, `is_local: true`) que ganaban automáticamente — y fallaban porque
Ollama no está instalado en servidores cloud-only.

## Cambio realizado

Agregado entry a `kernel/crates/ank-core/src/router/models.yaml`:

```yaml
- model_id: "openrouter/free"
  provider: "openrouter"
  display_name: "OpenRouter Free Tier"
  context_window: 131072
  cost_input_per_mtok: 0.0
  cost_output_per_mtok: 0.0
  supports_tools: false
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 2000
  task_scores:
    chat: 3
    coding: 3
    planning: 3
    analysis: 3
    summarization: 3
    extraction: 3
```

## Acceptance criteria

- [x] `models.yaml` contiene entry con `model_id: "openrouter/free"` y `provider: "openrouter"`
- [x] `cargo build --workspace` pasa sin errores
- [ ] Smoke test: mensaje de chat llega a OpenRouter sin error Ollama

## Notas

- Los modelos Ollama se mantienen en el catálogo — otros usuarios pueden tener Ollama instalado
- `openrouter/free` tiene `is_local: false` — necesita key del KeyPool para ser seleccionado
- El scoring (todos 3) lo coloca por debajo de modelos premium cuando hay otras keys configuradas
