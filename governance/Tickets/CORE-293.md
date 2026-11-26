# CORE-293 — `models.yaml`: Agregar modelos Ollama Cloud

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Media
- **Asignado a:** Arquitecto IA
- **Estado:✅ Done

## Contexto

Con CORE-292 el kernel soporta el provider `ollama_cloud`. Este ticket agrega
las entradas correspondientes en `models.yaml` para que el CMR pueda rutear
hacia modelos Ollama Cloud.

Los modelos Ollama Cloud son modelos grandes que corren en los servidores de
Ollama.com. No requieren GPU local. El usuario paga por uso con su API key de
Ollama.

## Modelos a agregar

Referencia: https://ollama.com/search?c=cloud

```yaml
- model_id: "ollama_cloud/gpt-oss:120b-cloud"
  provider: "ollama_cloud"
  display_name: "GPT-OSS 120B (Ollama Cloud)"
  context_window: 128000
  cost_input_per_mtok: 0.8
  cost_output_per_mtok: 2.0
  supports_tools: true
  supports_json_mode: true
  is_local: false
  avg_latency_ms: 1800
  task_scores:
    chat: 4
    coding: 4
    planning: 4
    analysis: 4
    summarization: 4
    extraction: 4

- model_id: "ollama_cloud/llama3.3:70b-cloud"
  provider: "ollama_cloud"
  display_name: "Llama 3.3 70B (Ollama Cloud)"
  context_window: 131072
  cost_input_per_mtok: 0.4
  cost_output_per_mtok: 0.8
  supports_tools: true
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 1200
  task_scores:
    chat: 4
    coding: 4
    planning: 3
    analysis: 4
    summarization: 4
    extraction: 3

- model_id: "ollama_cloud/gemma3:27b-cloud"
  provider: "ollama_cloud"
  display_name: "Gemma 3 27B (Ollama Cloud)"
  context_window: 131072
  cost_input_per_mtok: 0.3
  cost_output_per_mtok: 0.6
  supports_tools: false
  supports_json_mode: false
  is_local: false
  avg_latency_ms: 900
  task_scores:
    chat: 4
    coding: 3
    planning: 3
    analysis: 4
    summarization: 4
    extraction: 3
```

## Notas

- Los costos son estimaciones iniciales — actualizar con `update_models.py` una vez
  que Ollama publique precios oficiales de la API Cloud.
- Los `task_scores` serán actualizados en la próxima ejecución del script cuando
  PinchBench incorpore benchmarks de modelos Ollama Cloud.
- El `display_name` incluye "(Ollama Cloud)" para distinguirlos de los modelos
  locales en la UI del tenant.

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa (el YAML se parsea en compile-time)
- [ ] Los modelos con `provider: "ollama_cloud"` tienen `is_local: false`
- [ ] El CMR en `ModelProfile::Cloud` incluye los modelos Ollama Cloud
- [ ] El CMR en `ModelProfile::Local` excluye los modelos Ollama Cloud
- [ ] El CMR en `ModelProfile::Hybrid` incluye todos

## Dependencias

- CORE-292 (el provider debe existir en el kernel antes de agregar los modelos)
