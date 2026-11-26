# CORE-415: App Android como Asistente Digital Predeterminado (`ROLE_ASSISTANT`) + `ForegroundService` de Escucha Continua

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 63
- **Asignado a:** Shell Engineer

---

## Description

Integrar la app móvil Expo (`app/`) con capacidades nativas Android compilables desde Android Studio: registro oficial como Asistente Digital Predeterminado de Android (`ACTION_ASSIST` / `VOICE_COMMAND`), servicio en primer plano (`FOREGROUND_SERVICE_MICROPHONE`) para escucha continua 24/7 con pantalla apagada, grabación real con VAD tolerante a respiración y gestión de voces autorizadas desde el teléfono.

## Acceptance Criteria

- [x] Plugin Expo `app/plugins/withAegisAssistant.js` genera permisos, intent-filters de asistente y clases Kotlin `AegisVoiceForegroundService`, `AegisAssistantModule` y `AegisAssistantPackage`.
- [x] `voiceService.ts`, `chat.tsx` y `settings.tsx` reemplazan el mock de 5s por grabación real con detección de pausas, activación automática al abrir desde botón/gesto de asistente y controles de escucha 24/7 + enrolamiento vocal.
- [x] Documentación de compilación de APK paso a paso para Android Studio agregada en `app/README.md`.
