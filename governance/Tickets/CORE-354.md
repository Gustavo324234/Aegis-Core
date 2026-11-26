# CORE-354 — Real Ollama Modelfile Export and Model Registration in TrainingManager

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

En `TrainingManager::run_training_pipeline` (`ank-core/src/trainer/mod.rs`), el Paso 3 (`TrainingStatus::Exporting`) simulaba la exportación con un `tokio::time::sleep(Duration::from_secs(3)).await` en lugar de generar el `Modelfile` con el adaptador LoRA entrenado y registrarlo en Ollama.

## Solución propuesta

1. Implementar `export_and_register_ollama` en `TrainingManager` para generar un `Modelfile` real dentro del directorio de salida del adaptador LoRA (`FROM <base_model>`, `ADAPTER <lora_dir>`, `PARAMETER temperature 0.3` y `SYSTEM` con protocolo Tool Use v2).
2. Invocar `ollama create <custom_model_name> -f <Modelfile>` de forma asíncrona, registrando el modelo personalizado por tenant (`aegis-assistant-<tenant_id>`) y degradando con gracia si el binario `ollama` no está instalado o el daemon local no está activo.
3. Validar credenciales requeridas cuando se solicita `mode == "cloud"`.

## Criterios de aceptación

- [x] `TrainingManager` genera el `Modelfile` real en disco e invoca `ollama create` en lugar de `sleep(3)`.
- [x] Degradación elegante (Zero-Panic) cuando `ollama` no está instalado localmente, conservando el `Modelfile` generado.
- [x] Tests unitarios verificando la generación del `Modelfile` y validación de configuración.
