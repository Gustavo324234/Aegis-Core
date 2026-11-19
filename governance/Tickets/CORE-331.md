# CORE-331 — Shell: Native Training Engine UI & Progress Stream

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** UI/Shell Engineer + AI Architect

---

## Problema

Aegis OS requiere un módulo nativo que permita al usuario entrenar su propio modelo de chat (p. ej., Qwen 3.6, Llama 3.1) utilizando sus historiales locales de chat, ya sea localmente (si posee recursos de hardware como GPU) o en la nube. 
Aunque la infraestructura de backend en Rust (`TrainingManager` y endpoints REST/WebSocket) y los scripts de entrenamiento en Python (`generate_dataset.py` y `fine_tune.py`) ya están implementados y probados, **no existe una interfaz de usuario en la Shell** para que el usuario pueda configurar parámetros (modelo base, épocas, tasa de aprendizaje), iniciar o detener el entrenamiento, y monitorear en tiempo real la pérdida de entrenamiento (*training loss*) mediante gráficos y consolas de log en vivo.

## Solución propuesta

1. **Frontend UI Panel (`TrainingPanel.tsx`):**
   - Crear un panel con diseño premium, usando la estética visual oscura de Aegis OS con acentos cian, animaciones sutiles y tipografía monoespaciada.
   - Incluir controles para configurar parámetros de entrenamiento:
     - Modelo Base (ej. `Qwen 3.6`, `Phi-4-mini`, `Aegis Assistant base`).
     - Tasa de Aprendizaje (Learning Rate).
     - Número de Épocas (Epochs).
     - Tipo de Ejecución (Local vs. Nube).
   - Botones interactivos para "Iniciar Entrenamiento" y "Cancelar Entrenamiento".
   - Un componente gráfico de curva de pérdida (*loss curve chart*) interactivo en SVG/Canvas que dibuje la evolución de la pérdida en tiempo real a medida que llegan métricas.
   - Un visor de logs tipo consola para mostrar la salida en crudo del script de entrenamiento.

2. **Aegis Store Integration (`useAegisStore.ts`):**
   - Implementar el estado del entrenamiento en el store global: `isTrainingActive`, `trainingProgress` (pérdida actual, época actual, progreso porcentual, historial de pérdida), y `trainingLogs`.
   - Implementar acciones para invocar `POST /api/train/start` y `POST /api/train/cancel`.
   - Implementar lógica para conectarse a `/ws/train/progress` mediante WebSocket para escuchar actualizaciones en tiempo real y actualizar el estado global.

3. **Mounting en la UI:**
   - Registrar la pestaña "Entrenamiento" en `SettingsPanel.tsx` (o un botón dedicado en el Dashboard) para facilitar el acceso rápido del usuario.

## Criterios de aceptación

- [ ] El usuario puede abrir la pestaña de "Entrenamiento" en el Panel de Configuración y configurar los hiperparámetros.
- [ ] Al presionar "Iniciar", se realiza la petición correspondiente al backend y se inicia el WebSocket de streaming de métricas.
- [ ] La pérdida de entrenamiento se actualiza dinámicamente y se representa en una gráfica visual interactiva.
- [ ] El usuario puede cancelar el entrenamiento en cualquier momento y verificar que el backend detenga el subproceso correspondiente sin dejar zombies en memoria.
- [ ] La interfaz maneja de forma elegante los estados de carga, desconexión y finalización del entrenamiento.
