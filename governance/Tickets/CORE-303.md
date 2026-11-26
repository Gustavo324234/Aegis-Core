# CORE-303 — Kernel: Defensive Cognitive Loops & Boundary Autocorrection

## Metadata
- **Epic:** 53 — Stabilization: Agent Loop, Observability & Infrastructure
- **Tipo:** feat
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer
- **Estado:✅ Done

## Problema
Aegis trata el uso de herramientas (Tool Use) de manera determinista en el planificador ("LLM como ALU"). Sin embargo, los LLMs son probabilísticos y propensos a generar fugas de abstracción (p. ej., JSON mal formateado, parámetros faltantes, violación sutil de tipos, o llamadas a herramientas inexistentes).
Actualmente, estas fallas provocan que la máquina de estados determinística del kernel falle, aborte la tarea del agente o genere errores silenciosos de ejecución.

## Solución propuesta
Implementar un sistema defensivo con validadores automáticos y bucles de autocorrección antes de que el kernel ejecute una acción:

1. **Capa de Validación Sintáctica y Semántica:**
   Interceptar las invocaciones a herramientas producidas por la API del LLM antes de procesarlas en `SyscallExecutor`.
2. **Auto-Sanitización Heurística:**
   Si la llamada a la herramienta contiene pequeños errores de formato de tipo o JSON, aplicar heurísticas de sanitización automáticas (p. ej., forzar tipos, limpiar caracteres adicionales).
3. **Micro-loop de Autocorrección:**
   Si la sanitización falla, el kernel no debe abortar la tarea. Debe reinyectar automáticamente un mensaje de error estructurado directamente al LLM (sin intervención del usuario ni alterar la interfaz) indicando exactamente qué falló en su llamada (p. ej., `"Error: El parámetro X es requerido para la herramienta Y. Corrige tu llamada"`).
4. **Límite de reintentos:**
   Establecer un límite estricto de reintentos internos (p. ej., un máximo de 2 reintentos) para evitar bucles infinitos de auto-corrección.

## Criterios de aceptación
- [ ] Las llamadas a herramientas del LLM son interceptadas y validadas antes de la ejecución física de la syscall.
- [ ] Errores sintácticos corregibles heurísticamente se corrigen de forma transparente.
- [ ] Ante fallos semánticos, se gatilla un micro-loop interno que reinyecta el error al LLM para que se autocorriga.
- [ ] Si se excede el límite de 2 reintentos, el kernel aborta de forma segura devolviendo un error controlado y legible.
- [ ] No se filtra la conversación del micro-loop interno a la interfaz de usuario para mantener el chat limpio.
