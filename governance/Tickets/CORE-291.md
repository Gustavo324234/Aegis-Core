# CORE-291 — `models.yaml`: Actualizar scores y costos con datos reales

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** chore
- **Prioridad:** Alta
- **Asignado a:** Arquitecto IA
- **Estado:✅ Done

## Contexto

Primera ejecución del script creado en CORE-290. Los `task_scores` actuales son
estimaciones manuales sin fuente objetiva. Los `cost_*` de varios modelos Gemini
y OpenAI están desactualizados.

Este ticket es el resultado documentado de ejecutar:

```bash
python tools/update_models.py
```

## Cambios realizados

Los `task_scores` de todos los modelos con presencia en PinchBench fueron
recalculados usando la fórmula `ceil(success_rate / 20)`.

Los precios de modelos OpenAI, Groq, DeepSeek, Mistral y Qwen fueron
verificados contra OpenRouter `/api/v1/models`.

Ver diff en el commit correspondiente a este ticket.

## Criterios de aceptación

- [ ] `models.yaml` no contiene scores `0` en ningún modelo activo
- [ ] Los costos de modelos OpenAI y Gemini reflejan precios vigentes a la fecha del commit
- [ ] `cargo build` del workspace pasa sin errores (el YAML se parsea en compile-time)

## Dependencias

- CORE-290 (script debe existir antes de ejecutarse)
