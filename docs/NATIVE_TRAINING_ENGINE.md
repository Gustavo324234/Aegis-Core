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

## 5. Próximos Pasos Técnicos para la Implementación

1. **Creación de la API en Rust:** Programar los controladores de ruta en `ank-http` y los modelos de datos en `ank-core` para gestionar el ciclo de vida del entrenamiento.
2. **Desarrollo del Gestor de Subprocesos:** Implementar la lógica en Rust para hacer `spawn` de comandos en Python de forma segura, con límites de recursos y control de salida en tiempo real.
3. **Construcción de la Interfaz Web:** Diseñar el panel gráfico React en el Shell para configurar y controlar el motor de entrenamiento.
