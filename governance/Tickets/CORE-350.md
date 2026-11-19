# CORE-350 — Enforce Tenant Isolation, Secret Redaction, and Tool Use v2 in Fine-Tuning Dataset

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

1. **Violación de aislamiento multi-tenant:** `tools/fine-tuning/generate_dataset.py` iteraba sobre todos los directorios bajo `users/` sin filtrar por tenant, mezclando los historiales de chat de distintos usuarios en un único archivo `dataset.jsonl`.
2. **Exposición de secretos en dataset:** Los logs de conversación podían contener claves de API (`sk-...`, `gsk_...`, `AIza...`), tokens Bearer o `setup_token` que pasaban sin redactar al archivo de entrenamiento.
3. **Formato de herramientas obsoleto:** `generate_dataset.py`, `dataset_template.jsonl` y `fine_tune.py` seguían inyectando los tags legacy `<aegis_sys_call>` en lugar del protocolo actual de Tool Use estructurado (`__TOOL_CALL__`).

## Solución propuesta

1. Añadir `tenant_id: Option<String>` a `TrainingConfig` en `ank-core/src/trainer/mod.rs` e inyectarlo automáticamente desde `CitadelAuthenticated` en `POST /api/train/start` (`ank-http/src/routes/train.rs`).
2. Pasar `--data_dir` y `--tenant_id` desde `TrainingManager::run_training_pipeline` a `generate_dataset.py`, generando rutas de dataset y adaptador LoRA aisladas por tenant.
3. Actualizar `generate_dataset.py` para procesar exclusivamente el tenant solicitado (cuando se especifica `--tenant_id`), sanitizar claves/tokens sensibles mediante expresiones regulares y usar el formato Tool Use v2 en `generate_dataset.py`, `dataset_template.jsonl` y `fine_tune.py`.

## Criterios de aceptación

- [x] `POST /api/train/start` vincula el `tenant_id` autenticado por Citadel a `TrainingConfig`.
- [x] `generate_dataset.py` aísla estrictamente los logs del `tenant_id` indicado y redacta API keys/tokens.
- [x] Plantilla y scripts de fine-tuning actualizados al protocolo Tool Use v2.
