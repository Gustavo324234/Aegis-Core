# CORE-344 — Kernel Syscall & Tool: Control de Audio PipeWire (`SYS_AUDIO_CONTROL`)

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 57 — Linux Distro & Native Hardware Integration
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Para habilitar la interacción por voz en instalaciones locales o quiosco nativo, los agentes deben tener la capacidad de verificar el estado de los dispositivos de entrada/salida de audio (micrófono y altavoces) y silenciar o ajustar niveles según las peticiones del usuario.

## Solución propuesta

1. Implementar la syscall `SYS_AUDIO_CONTROL` y la herramienta de kernel `manage_audio_device` en `ank-core`.
2. Permitir a los agentes consultar nodos de audio activos (`pw-cli` / `wpctl` / invocación nativa) y silenciar o desmutear el micrófono ante comandos del usuario.
3. Garantizar manejo seguro con fallbacks cuando no hay servidor PipeWire/PulseAudio presente.

## Criterios de aceptación

- [x] Syscall `SYS_AUDIO_CONTROL` (`Syscall::AudioControl`) registrada e implementada en `ank-core`.
- [x] Soporte para consulta de estado (mute, volumen) y ejecución de comandos de cambio de estado.
- [x] Pruebas unitarias de parsing y manejo de errores.
