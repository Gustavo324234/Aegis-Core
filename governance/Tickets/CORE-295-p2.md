# CORE-295-p2 — Shell: preservar stt_provider al guardar config de voz

**Tipo:** fix  
**Prioridad:** Alta  
**Épica:** EPIC 52 — Voice Quality  
**Estado:✅ Done
**Asignado a:** UI Engineer  

---

## Contexto

Al guardar la configuración de voz en la interfaz de usuario (React), el valor de `stt_provider` se sobrescribía incorrectamente o no se enviaba al backend, haciendo que el usuario tuviera que volver a configurarlo.

## Cambios Realizados

- Corregida la persistencia en el Zustand store para enviar `stt_provider` correctamente.
- Validada la serialización de payload en el endpoint `/api/tenants/voice-config` del backend.
