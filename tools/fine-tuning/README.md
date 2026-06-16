# Aegis OS - Guía de Ajuste Fino (Fine-Tuning) para el Asistente Local

Este directorio contiene las herramientas y scripts necesarios para entrenar y personalizar tu propio modelo de lenguaje pequeño (SLM) de pesos abiertos, optimizándolo para actuar como el **Asistente Principal** de tu instancia local de Aegis OS.

---

## 1. Requisitos Previos

El entrenamiento (fine-tuning) se realiza mediante **QLoRA** (cuantización a 4 bits). Recomendamos realizar el entrenamiento en una GPU Nvidia con al menos 12 GB de VRAM (en local) o en una máquina virtual en la nube (ej. RunPod, Lambda Labs, Google Colab).

### Instalación de dependencias básicas:
```bash
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu121
pip install transformers peft bitsandbytes trl accelerate packaging
```

*Nota: Si estás entrenando en la nube, es altamente recomendable utilizar **Unsloth** para acelerar el entrenamiento entre 2x y 5x y reducir el consumo de memoria. Puedes instalarlo con:*
```bash
pip install "unsloth[colab-new] @ git+https://github.com/unslothai/unsloth.git"
```

---

## 2. Preparación del Dataset

El script de entrenamiento espera un archivo de datos en formato JSON Lines (`.jsonl`) llamado `dataset.jsonl`.
Tienes dos formas de obtener este archivo:

### Opción A: Generación Automática desde tus Logs de Chat (Recomendado)
Hemos creado un script que lee automáticamente tus logs reales de conversación de tu instalación local de Aegis OS (`chat_history.log`), los agrupa en sesiones coherentes basadas en intervalos de tiempo, y genera el dataset de forma automatizada:

```bash
python generate_dataset.py --threshold_minutes 30 --output_file "dataset.jsonl"
```

* **`--data_dir`:** (Opcional) Permite forzar la ruta del directorio de datos de Aegis. Por defecto, el script detecta automáticamente las rutas estándar de Aegis en Windows (`%APPDATA%/aegis`), Linux (`~/.local/share/aegis`) y macOS.
* **`--threshold_minutes`:** Define cuántos minutos de inactividad separan una conversación de otra (por defecto: 30 minutos).

### Opción B: Creación Manual usando la Plantilla
Si prefieres construir el dataset de forma manual o sintética, cada línea debe estructurarse con la lista de mensajes en formato ChatML compatible con Hugging Face:

```json
{
  "messages": [
    {"role": "system", "content": "Eres el Asistente de Aegis OS. Hablas de forma concisa y natural para voz..."},
    {"role": "user", "content": "¿Hay alguna tarea pendiente hoy?"},
    {"role": "assistant", "content": "<aegis_sys_call>{\"tool\": \"read_tasks\"}</aegis_sys_call> Tienes 2 tareas pendientes en tu panel Kanban."}
  ]
}
```

* Revisa el archivo [dataset_template.jsonl](dataset_template.jsonl) para ver ejemplos reales de formato conversacional y llamadas a herramientas.

---

## 3. Ejecutar el Entrenamiento

Para iniciar el ajuste fino utilizando el script `fine_tune.py`:

```bash
python fine_tune.py --model_id "Qwen/Qwen2.5-7B-Instruct" --dataset_path "dataset.jsonl" --output_dir "./aegis-assistant-lora"
```

### Argumentos del Script:
* `--model_id`: El identificador de Hugging Face del modelo base (por defecto: `Qwen/Qwen2.5-7B-Instruct`, también puedes usar `microsoft/Phi-4-mini-instruct` o `meta-llama/Llama-3.1-8B-Instruct`).
* `--dataset_path`: Ruta a tu archivo de datos JSON Lines.
* `--output_dir`: Carpeta donde se guardará el adaptador entrenado (LoRA).
* `--epochs`: Número de épocas de entrenamiento (por defecto: 3).
* `--learning_rate`: Tasa de aprendizaje (por defecto: 2e-4).

---

## 4. Exportar a GGUF (Ollama / Inferencia Local)

Una vez que finaliza el entrenamiento, el script exportará los pesos del adaptador. Para poder correr este modelo localmente en tu laptop de pruebas a alta velocidad en la CPU, debes convertirlo al formato **GGUF**:

### Paso A: Clonar y compilar `llama.cpp`
```bash
git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp
make  # En Windows usa cmake
```

### Paso B: Fusionar y convertir a GGUF
Ejecuta el script de conversión de `llama.cpp` apuntando al modelo fusionado:
```bash
python convert_hf_to_gguf.py ../aegis-assistant-merged/ --outfile ../aegis-assistant.gguf --outtype q4_K_M
```
*(El tipo de cuantización `q4_K_M` es el recomendado para mantener una excelente relación entre precisión y bajo consumo de RAM).*

---

## 5. Importar el Modelo en Ollama

Para que Aegis OS pueda seleccionar tu nuevo modelo asistente, regístralo en la instancia local de Ollama:

1. Crea un archivo llamado `Modelfile` en el mismo directorio donde guardaste tu `aegis-assistant.gguf`:
   ```dockerfile
   FROM ./aegis-assistant.gguf

   # Definir parámetros de temperatura y contexto
   PARAMETER temperature 0.3
   PARAMETER num_ctx 8192

   # Prompt del Sistema base
   SYSTEM """Eres el Asistente Principal de Aegis OS. Tu objetivo es ayudar al usuario de forma cercana, respondiendo con oraciones concisas y limpias (óptimas para lectura por voz). Cuando sea necesario, ejecuta comandos del sistema envolviéndolos estrictamente en los tokens <aegis_sys_call> y </aegis_sys_call>."""
   ```

2. Compila el modelo en Ollama:
   ```bash
   ollama create aegis-assistant -f Modelfile
   ```

3. Edita la configuración de tu kernel Aegis (`kernel/crates/ank-core` o mediante el archivo `.env`) para seleccionar `aegis-assistant` como tu modelo primario de chat y voz.
