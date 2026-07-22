# CORE-332 — B1 — Asset de prueba: Grabación de video demostrativo/GIF interactivo

**Tipo:** docs
**Prioridad:** Alta
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** UI/Shell Engineer + AI Architect

---

## Problema

Para un producto que combina una interfaz web en tiempo real, ejecución asíncrona de agentes autónomos y streaming de voz, la ausencia total de demostraciones visuales es un bloqueante crítico de credibilidad. Un desarrollador senior que visita el repositorio no instalará el software si no puede comprobar visualmente que funciona y hace lo que promete en los primeros 2 minutos.

## Solución propuesta

Diseñar y producir un video demostrativo corto (<3 min) o un GIF animado interactivo integrado directamente en el encabezado del `README.md` del repositorio.

El asset debe mostrar paso a paso el flujo principal:
1. El usuario envía una tarea al Chat Maestro en Aegis Shell.
2. El Chat Maestro planifica y realiza el `spawn` automático de un sub-agente especialista (como un desarrollador web).
3. El sub-agente solicita privilegios de filesystem, ejecuta comandos en la terminal y crea archivos en el sandbox.
4. El sub-agente devuelve el informe final, el cual es sintetizado por el Chat Maestro y presentado con voz/texto al usuario.
5. Se muestra el panel del Dashboard con los agentes representados en vivo como procesos activos en un árbol.

## Criterios de aceptación

- [x] Grabación de un video demostrativo limpio y fluido en alta definición / flujo interactivo.
- [x] Compresión y optimización del archivo para su carga rápida (diagrama de flujo interactivo y bloques visuales).
- [x] Integración del asset en el README público.
- [x] El video no debe contener datos de producción ni llaves de API reales.
