# Especificación Técnica: Motor de Entrenamiento Nativo de Aegis OS (Self-Improving OS)

Esta especificación detalla el diseño arquitectónico, objetivos y componentes necesarios para implementar el **Motor de Entrenamiento Nativo** de Aegis OS. Esta característica permite al sistema aprender de las conversaciones locales del usuario y auto-mejorar sus modelos de lenguaje asociados de forma automatizada, tanto en hardware local como mediante servicios en la nube.

---

## 1. Objetivos del Sistema

* **Privacidad Absoluta (Citadel-Aligned):** Permitir a usuarios con hardware capaz entrenar su modelo localmente de forma privada, asegurando que sus logs de chat e información personal nunca salgan del encriptado local.
* **Abstracción de Complejidad:** Ocultar al usuario final los detalles de bajo nivel (instalaciones de Python, dependencias de CUDA, configuraciones de PyTorch, compiladores de `llama.cpp` para GGUF). El proceso se controla con un solo clic.
* **Adaptabilidad de Hardware:** Proveer dos modos de entrenamiento intercambiables: **Local** (para GPUs Nvidia y Apple Silicon) y **Nube Serverless** (para computadoras portátiles antiguas u otros dispositivos sin GPU de alto rendimiento).
* **Conversión y Hot-Reload Automático:** Al finalizar el entrenamiento, el sistema compila automáticamente el resultado a GGUF, lo registra en la instancia local de Ollama y realiza un reemplazo del modelo activo en caliente sin interrumpir el funcionamiento del servidor.

---

## 2. Arquitectura de Componentes

El motor de entrenamiento se compone de cuatro capas integradas dentro del monorepo:

```
┌──────────────────────────────────────────────────────────────────┐
│                           UI (Shell)                             │
│ - Panel de Configuración de Entrenamiento                        │
│ - Gráfico de Pérdida (Loss) y Métricas en Tiempo Real            │
└────────────────────────────────▲─────────────────────────────────┘
                                 │ WebSockets / REST
┌────────────────────────────────▼─────────────────────────────────┐
│                       Servidor Web (ank-http)                    │
│ - API Endpoints: /api/train/start, /api/train/status             │
│ - WebSocket Handler para streaming de logs y métricas            │
└────────────────────────────────▲─────────────────────────────────┘
                                 │ Control
┌────────────────────────────────▼─────────────────────────────────┐
│                    Núcleo Cognitivo (ank-core)                   │
│ - Módulo `trainer`: Gestor de ciclos y subprocesos               │
│ - Replicador de datasets en formato JSONL                        │
│ - Conector API de Nube (RunPod / Replicate)                      │
└────────────────────────────────┬─────────────────────────────────┘
                                 │ Ejecución
                 ┌───────────────┴───────────────┐
                 ▼ Local                         ▼ Nube (Serverless)
       ┌───────────────────┐           ┌───────────────────┐
       │   Subprocesos     │           │   API Remota      │
       │   Python/Ollama   │           │   RunPod / HF     │
       └───────────────────┘           └───────────────────┘
```

---

## 3. Especificación del Flujo de Datos

### Paso 1: Generación y Curación del Dataset
* El módulo `trainer` en `ank-core` ejecuta internamente el script [generate_dataset.py](file:///e:/Aegis/Aegis-Core/tools/fine-tuning/generate_dataset.py).
* Lee el log local `chat_history.log` del tenant activo en `data_dir/users/<tenant_id>/workspace/chat_history.log`.
* Segmenta los datos en sesiones de chat, inyecta el `system_prompt` base del **Aegis Assistant** y los escribe en un archivo temporal en formato JSON Lines.

### Paso 2: Ejecución del Entrenamiento

#### A. Modo Local (Procesamiento por Subproceso)
* El kernel de Aegis inicia un subproceso de Python llamando a [fine_tune.py](file:///e:/Aegis/Aegis-Core/tools/fine-tuning/fine_tune.py) con los parámetros configurados.
* Captura el flujo de salida estándar (`stdout`) del proceso.
* Un parser busca patrones de pérdida (`Loss = X.XXXX`) en el output del script y los envía en tiempo real al WebSocket de la UI para graficar el avance.

#### B. Modo Nube (Procesamiento Remoto Serverless)
* El kernel de Aegis empaqueta el dataset temporal (opcionalmente encriptado con una clave efímera).
* Realiza una solicitud HTTP a un proveedor serverless (ej. RunPod Serverless o Replicate) utilizando la API Key configurada por el usuario.
* Envía los datos y arranca un contenedor Docker preconfigurado con el entorno de entrenamiento de Aegis.
* Hace un sondeo de estado (*polling*) o recibe *webhooks* con los logs y el progreso para retransmitirlos a la UI.

### Paso 3: Conversión y Registro
* Una vez finalizado el entrenamiento, el sistema ejecuta las utilidades de conversión de `llama.cpp` (o descarga el GGUF directamente desde el contenedor de la nube).
* Genera el archivo final `aegis-assistant-custom.gguf`.
* Envía una solicitud a la API local de Ollama para registrar el modelo:
  ```bash
  ollama create aegis-assistant-custom -f Modelfile
  ```

### Paso 4: Hot-Reload
* El `CognitiveRouter` de Aegis actualiza la configuración del tenant en la base de datos cifrada (`TenantDB`) para establecer `ollama/aegis-assistant-custom` como el nuevo modelo activo.
* Las siguientes consultas en el chat son dirigidas automáticamente al nuevo modelo personalizado.

---

## 4. Diseño de la API REST y WebSockets

### REST Endpoints
* **`POST /api/train/start`:** Inicia el ciclo de entrenamiento.
  * **Payload:**
    ```json
    {
      "mode": "local" | "cloud",
      "model_base": "qwen-3.6-27b" | "phi-4-mini",
      "epochs": 3,
      "learning_rate": 0.0002,
      "cloud_provider": {
        "name": "runpod",
        "api_key": "..."
      }
    }
    ```
* **`GET /api/train/status`:** Consulta el estado actual (Idle, Preparing, Training, Exporting, Completed, Failed).
* **`POST /api/train/cancel`:** Cancela de forma segura el subproceso o la ejecución remota activa.

### WebSocket Events (`/ws/train/progress`)
Transmite tramas binarias o de texto con las métricas en tiempo real:
```json
{
  "status": "training",
  "epoch": 1.2,
  "step": 45,
  "loss": 0.3542,
  "eta_seconds": 1200
}
```

---

## 5. Estado de la Implementación (MVP Completado)

El motor de entrenamiento nativo ha sido completamente implementado y estabilizado en el monorepo bajo las siguientes características:
1. **API y Enrutamiento en Rust (`ank-http` / `ank-core`):** Endpoints completamente funcionales para iniciar, cancelar y consultar estado del entrenamiento. El core expone la lógica para invocar dinámicamente scripts de Python bajo demanda.
2. **Pre-flight Checks (Validación de Entorno):** El backend en Rust ejecuta validaciones síncronas rápidas (`python -c "import torch, peft..."`) antes de lanzar el entrenamiento principal. Si el entorno carece de librerías o falla la configuración, se interrumpe y se notifica con un mensaje legible a la interfaz.
3. **Interfaz Gráfica de Usuario (`shell/ui`):** 
   - Panel de control interactivo con campos de hiperparámetros (épocas, batch size, learning rate, tipo de modelo y entorno local vs nube).
   - Consola negra interactiva con scroll automático para logs en vivo del subproceso.
   - Gráfico dinámico SVG para seguir en tiempo real la curva de pérdida de pasos de entrenamiento.
   - Telemetría en tiempo real de uso de memoria de video (VRAM Widget) con alertas automáticas de color en caso de consumo crítico (>85%) para mitigar desbordamientos.
4. **Entorno Aislado para NixOS:** Inclusión de [shell.nix](file:///e:/Aegis/Aegis-Core/tools/fine-tuning/shell.nix) para enlazar los drivers OpenGL del anfitrión, CUDA, C++ Standard Libraries y construir el entorno virtual `.venv` de forma reproducible.

---

## 6. Visión de Futuro y Próximos Pasos

El motor cognitivo evolucionará en las siguientes áreas planificadas:
1. **Auto-conversión y Registro Automatizado (Hot-Reload):** Automatizar el script que ejecuta `llama.cpp` tras finalizar el entrenamiento para empaquetar el modelo en formato GGUF, registrarlo en la instancia local de Ollama e indicarle al enrutador cognitivo de Aegis (`CognitiveRouter`) que empiece a redirigir consultas al modelo recién personalizado sin reiniciar el daemon de Rust.
2. **Entrenamiento en Nube Serverless:** Completar la integración remota con proveedores como RunPod o Replicate mediante envío encriptado del dataset curado para dispositivos locales que no posean GPUs dedicadas.
3. **Evaluación de Dataset con Citadels:** Incorporar un agente auditor en el protocolo Citadel que filtre la información sensible del usuario (claves, tokens, datos personales) en `chat_history.log` antes de compilar el dataset JSON Lines.
