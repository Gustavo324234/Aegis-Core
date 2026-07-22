# CORE-343 — Kernel Syscall & Tool: Telemetría de Hardware Nativa (`SYS_HW_TELEMETRY`)

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 57 — Linux Distro & Native Hardware Integration
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Los agentes del kernel necesitan conocer el estado físico del host (batería, temperatura de CPU, carga de memoria) para tomar decisiones informadas de ruteo cognitivo y prevenir fallos en hardware limitado.

## Solución propuesta

1. Implementar un módulo de telemetría de hardware en `ank-core` que consulte métricas de sistema operativo (batería en `/sys/class/power_supply/`, carga de CPU y memoria).
2. Exponer la syscall `SYS_HW_TELEMETRY` y registrar la herramienta `get_hardware_telemetry` en `ToolRegistry` para que los agentes puedan consultar la telemetría del sistema.
3. Asegurar degradación graciosa (Graceful Degradation) cuando se ejecuta en entornos donde `/sys` o los sensores no estén presentes (retornando estado simulado o valores estructurados por defecto).

## Criterios de aceptación

- [x] Implementación de `HardwareTelemetry` en `ank-core` (`Syscall::HardwareTelemetry`).
- [x] Syscall `SYS_HW_TELEMETRY` expuesta y manejada en `SyscallExecutor`.
- [x] Pruebas unitarias en `ank-core` verificando la lectura de métricas y el fallback ante ausencia de sensores.
