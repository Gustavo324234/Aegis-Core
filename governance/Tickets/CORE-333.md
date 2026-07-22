# CORE-333 — B2 — Benchmarks: Publicación de resultados reales y reproducibles de PinchBench

**Tipo:** chore
**Prioridad:** Alta
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** Arquitecto IA

---

## Problema

El concepto de "ruteo cognitivo" dinámico (seleccionar el mejor modelo local o en la nube para cada tarea) es un reclamo técnico fuerte que requiere evidencia empírica. Sin datos publicados de rendimiento y latencia, los desarrolladores considerarán que se trata de "vaporware" u optimizaciones teóricas sin impacto real.

## Solución propuesta

Ejecutar la herramienta PinchBench disponible en `tools/` utilizando modelos reales de desarrollo (como modelos de OpenAI, Gemini, Anthropic y Ollama locales en Qwen/Llama) para recopilar métricas detalladas:
1. Precisión del modelo en tareas de uso de herramientas (Tool-Use) y formateo estructurado JSON.
2. Latencia promedio por token generado (Time to First Token y tokens por segundo).
3. Costo financiero estimado por millón de tokens procesados.

Compilar estos resultados en una tabla de benchmarks clara y honesta, y publicarla como parte de la documentación oficial.

## Criterios de aceptación

- [x] Ejecución del harness PinchBench contra al menos 4 proveedores distintos.
- [x] Recopilación estructurada de los scores de precisión y tiempos de respuesta.
- [x] Creación de un archivo de reporte o sección dedicada en la documentación del repositorio (`docs/BENCHMARKS_PINCHBENCH.md`).
- [x] Publicación de instrucciones para que cualquier usuario pueda reproducir localmente el benchmark utilizando sus propias API keys.
