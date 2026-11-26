# CORE-412: Biometría Vocal Multi-Perfil v2, Groq Whisper Zero-Config y Endpoint `/api/siren/utterance`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 63
- **Asignado a:** Kernel Engineer + AI Architect

---

## Description

Evolucionar el motor de biometría vocal y la API HTTP de Siren para soportar múltiples voces autorizadas (por persona o dispositivo), detección dinámica de frase de activación (`"Aegis escuchame"` o `"<Nombre> escuchame"`) y transcripción sin doble configuración aprovechando la API key de Groq ya cargada en el `KeyPool`.

## Acceptance Criteria

- [x] `speaker_id.rs` implementa huella vocal v2 (56-dim: MFCCs, $\Delta$-MFCC, $F_0$ y espectro con filtrado de silencios), compatibilidad con huellas v1 (9-dim), `merge_enrollments`, `verify_multi` y `match_and_strip_wake_phrase`.
- [x] `TenantDB` almacena y administra múltiples perfiles en `enrolled_voices` y estado `voice_biometrics_gate`.
- [x] `persona_api.rs` soporta `POST /api/persona/name` y `siren_api.rs` expone `/voices/enrolled`, `/voices/enroll`, `/voices/enrolled/:id`, `/voices/gate`, `/voices/verify-test` y `/utterance` manteniendo compatibilidad retroactiva en `/enroll`.
