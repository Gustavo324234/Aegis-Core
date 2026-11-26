# CORE-305 — Kernel: Arquitectura de Ruteo Cognitivo Asimétrico (Local-First)

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 51 — Model Intelligence: PinchBench + Ollama Cloud + CMR v2  
**Estado:** ✅ Done  
**Asignado a:** Arquitecto IA + Kernel Engineer  

---

## Problema
Aegis aspira a ser un sistema operativo cognitivo self-hosted que corre en hardware local (p. ej., portátiles, Jetson, Raspberry Pi 5). Sin embargo, las tareas avanzadas de planificación, ejecución de sub-agentes jerárquicos y compilación de gráficos DAG requieren de modelos frontera con alta capacidad de razonamiento. Correr estos modelos masivos en local genera una latencia inaceptable para el usuario, mientras que derivar todo a la nube compromete la privacidad (promesa central de Citadel).

## Solución propuesta
Implementar una arquitectura de ruteo cognitivo asimétrico dividiendo las responsabilidades lógicas del sistema:

1. **Ruteador Cognitivo Local (Edge Layer):**
   * Dispositivos locales modestos ejecutarán modelos locales ultra-pequeños y optimizados (de 1B a 3B de parámetros, ej. Llama 3.2 1B, Qwen 2.5 1.5B/3B).
   * Responsabilidades locales: detección rápida de voz, procesamiento y parsing sintáctico, gate de seguridad básico de Citadel, enrutamiento primario de intenciones (L1 Memory).
2. **Procesador Cognitivo Avanzado (Asymmetric Layer):**
   * Las tareas de razonamiento pesado (compilación DAG, ejecución recursiva de sub-agentes, síntesis L3/memoria a largo plazo) se enrutan de forma asincrónica.
   * Se proveerán dos modos configurables:
     * **Modo Red Local:** Offloading seguro a una máquina dedicada en la misma red local (ej. PC doméstica con GPU).
     * **Modo Enclave en la Nube:** Offloading a APIs de nube (OpenRouter, Anthropic, Ollama Cloud) garantizando que los datos de contexto sensibles se anonimizen o cifren antes del envío.
3. **Control Dinámico de Enrutamiento (CMR v2 Enhancements):**
   * Expandir la lógica del motor CMR para que decida dinámicamente, según la complejidad del ticket o tarea, si se resuelve localmente o se delega al enclave pesado.

## Criterios de aceptación
- [ ] Tareas ligeras (saludos, consultas directas del sistema) se resuelven 100% en local en <200ms usando modelos Edge.
- [ ] El planificador cognitivo puede segmentar una tarea en sub-tareas asimétricas, delegando el razonamiento denso al Procesador Avanzado y el control local al Edge.
- [ ] Se implementa un proxy de sanitización de contexto que remueve secretos, tokens de acceso y datos extremadamente sensibles antes de enviar datos al Enclave en Nube (si este modo está activo).
- [ ] Los scores de PinchBench y latencias reales se usan para guiar la decisión de offloading.
