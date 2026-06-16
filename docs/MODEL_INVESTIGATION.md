# Investigación: Modelos Viables para el Asistente Local de Aegis OS

Este documento evalúa los modelos de lenguaje pequeños (SLMs) de código abierto más viables en la actualidad para actuar como el **Asistente Principal** en local de Aegis OS, permitiendo conversación fluida en español, llamadas a herramientas estructuradas (Tool-Calling) y micro-ajuste fino (Fine-Tuning) periódico.

---

## 1. Criterios de Selección

Para que un modelo pueda ejecutarse de forma fluida en una computadora portátil antigua y cumplir con la tesis de Aegis OS, debe satisfacer los siguientes requisitos:

1. **Bajo Consumo de RAM (Eficiencia):** Debe poder ejecutarse cuantizado en un entorno local consumiendo menos de 5 GB de RAM.
2. **Precisión en Tool-Calling (JSON Estricto):** Debe comprender cuándo invocar herramientas de sistema (ej: `[SYS_MCP_EXEC]`) y formatear los argumentos en JSON sin errores de sintaxis.
3. **Conversación Fluida en Español:** Calidad de redacción natural y respuestas limpias (sin markdown excesivo) para ser procesadas por el motor de voz Siren.
4. **Compatibilidad con Ajuste Fino (LoRA/QLoRA):** Estructura estándar compatible con herramientas de entrenamiento accesibles (como Unsloth o Axolotl).

---

## 2. Modelos Candidatos Evaluados

| Familia de Modelo | Parámetros | RAM Necesaria (GGUF Q4_K_M) | Velocidad en CPU ARM/x86 | Precisión en Tool-Calling | Desempeño en Español |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Qwen-2.5-7B-Instruct** | 7.5B | ~4.8 GB | Media-Alta | **Excelente (Líder en 7B)** | Excelente |
| **Llama-3.1-8B-Instruct** | 8.0B | ~5.3 GB | Media | Alta (Estándar de la industria) | Excelente |
| **Phi-4-mini** | 3.8B | ~2.7 GB | Muy Alta | Media-Alta (Requiere validación) | Buena |
| **Gemma-2-2B-it** | 2.6B | ~1.8 GB | Máxima | Media (Sufre con múltiples herramientas) | Buena |

---

## 3. Análisis de los Modelos Ganadores

### Opción A: Qwen-2.5-7B-Instruct (El Ganador Técnico)
Es actualmente la opción más sólida del mercado para este tipo de arquitectura.
* **Por qué destaca:** Los modelos Qwen-2.5 han sido entrenados específicamente para codificación y generación de datos estructurados. Su capacidad para seguir esquemas JSON y realizar llamadas a funciones (*function calling*) supera incluso a modelos de mayor tamaño de otras familias.
* **Ventaja en Español:** Su base de datos de entrenamiento multilingüe es inmensa, logrando un lenguaje muy natural en español.
* **Viabilidad de Entrenamiento:** Es 100% compatible con **Unsloth** (un framework de entrenamiento acelerado), lo que permite hacerle un QLoRA en menos de 2 horas en una sola GPU comercial.

### Opción B: Phi-4-mini (3.8B) (El Ganador para Hardware Limitado)
Si la laptop vieja tiene muy poca RAM (ej: 8 GB en total, donde el sistema consume la mitad), un modelo de 7B/8B puede provocar lentitud. Phi-4-mini es la mejor alternativa compacta.
* **Por qué destaca:** Creado por Microsoft con un enfoque en razonamiento lógico intenso. Aunque tiene solo 3.8B de parámetros, compite directamente con modelos de 7B tradicionales en razonamiento matemático y de código.
* **Rendimiento Local:** Su footprint de memoria es minúsculo (~2.7 GB cuantizado), lo que garantiza una velocidad de tokens por segundo muy elevada en procesadores x86_64 antiguos.
* **Desafío:** Su capacidad de Tool-Calling nativa es menos robusta que la de Qwen. Para usarlo como orquestador de Aegis, **el fine-tuning básico es obligatorio** para fijar el formato de llamada de herramientas.

---

## 4. Pipeline de Implementación: Entrenamiento y Despliegue Local

Para materializar esta estrategia, el flujo de desarrollo recomendado consta de las siguientes fases:

```
  ┌────────────────────────┐
  │ 1. Recolección de Datos│ ──> Conversaciones cortas (Siren) + Esquemas de Tools
  └───────────┬────────────┘
              ▼
  ┌────────────────────────┐
  │ 2. Entrenamiento QLoRA │ ──> Uso de Unsloth (GPU en la nube)
  └───────────┬────────────┘
              ▼
  ┌────────────────────────┐
  │ 3. Cuantización GGUF   │ ──> Compresión a 4-bits o 5-bits via llama.cpp
  └───────────┬────────────┘
              ▼
  ┌────────────────────────┐
  │ 4. Distribución Local  │ ──> Carga en Ollama/ank-server en la laptop de pruebas
  └────────────────┘
```

### A. Preparación del Dataset (Entrenamiento Básico)
Generar un archivo en formato ShareGPT con 2,000 a 5,000 ejemplos de entrenamiento:
* **Entradas (Prompts):** Peticiones del usuario de la vida diaria (mensajes de voz, texto corto) y estados del sistema.
* **Salidas (Respuestas del Asistente):** Formato conversacional limpio (sin markdown complejo, sin viñetas) y llamadas de herramientas JSON del tipo:
  ```json
  {"tool": "SYS_MCP_EXEC", "arguments": {"server": "aegis-nexus", "tool": "read_file", "path": "..."}}
  ```

### B. Entrenamiento Acelerado con Unsloth
Usaremos **Unsloth** en un Notebook de Jupyter. Unsloth optimiza el uso de VRAM y acelera el entrenamiento en un **2x - 5x** en comparación con PyTorch puro.
* **Requisitos:** Una sola GPU en la nube (ej: RTX 3090/4090 por un costo aproximado de $0.80 USD/hora).
* El script generará un archivo adaptador LoRA.

### C. Conversión a GGUF y Ollama
1. Fusionar el adaptador LoRA con el modelo base original.
2. Compilar el modelo resultante a formato **GGUF** usando la herramienta `quantize` de `llama.cpp` a precisión `Q4_K_M` (óptima relación calidad/tamaño) o `Q5_K_M`.
3. Crear un archivo de configuración de Ollama (`Modelfile`) para registrar el modelo:
   ```dockerfile
   FROM ./aegis-assistant-q4.gguf
   SYSTEM "Eres el Asistente Principal de Aegis OS. Responde de forma concisa y natural..."
   ```

---

## 5. El Bucle de Aprendizaje Semanal (Offline Learning)

Para cumplir con tu idea de que "aprenda sobre la marcha", el flujo offline funcionará de la siguiente manera:

1. **Captura:** Cada vez que corriges al asistente en el chat principal, el kernel de Aegis guarda esa corrección como un par de entrenamiento `{"input": "...", "target_output": "..."}` en `/data/var/lib/aegis/logs/tuning_data.jsonl`.
2. **Filtro:** Un sub-agente local de curación (ej: un script que se ejecuta los domingos por la noche) filtra los datos para asegurar que no contengan contraseñas ni información sensible.
3. **Micro-Entrenamiento:** Se ejecuta un script automatizado local en la laptop que realiza un ajuste fino de muy bajo rango (LoRA con rank=4 o rank=8) utilizando la CPU/GPU local sobre los datos de esa semana.
4. **Hot-Reload:** Aegis OS recarga el nuevo adaptador LoRA en caliente en la instancia local de inferencia.
