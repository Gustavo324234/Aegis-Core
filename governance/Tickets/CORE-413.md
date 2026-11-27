# CORE-413: VAD Natural Tolerante a Respiración (`smartVadRecorder`) y Estudio de Voces/Wake-Word en `shell/ui`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 63
- **Asignado a:** Shell Engineer

---

## Description

Eliminar los cortes abruptos al respirar durante dictado o modo conversación continua, agregar ventana de repregunta manos libres (`8s`) e incorporar en `SirenConfigTab` un estudio completo para configurar el nombre de activación, tolerancia de pausas y múltiples voces autorizadas con prueba en vivo.

## Acceptance Criteria

- [x] `smartVadRecorder.ts` implementa pre-roll de 500ms, calibración adaptativa de ruido y tolerancia configurable de pausas para respirar (`2.4s` por defecto).
- [x] `sirenSlice.ts`, `chatSlice.ts` y `InputModeSelector.tsx` soportan wake-word biométrico continuo con fallback `"Aegis"` y ventana de repregunta de `8s` tras responder.
- [x] `SirenConfigTab.tsx` permite grabar 3 muestras por voz, agregar o eliminar voces, activar/desactivar filtro biométrico y probar reconocimiento en vivo.
