# CORE-417: Builder Automático Android APK en CI, Auto-Updater In-App (SHA-256) y QR de Descarga en Web

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 63
- **Asignado a:** DevOps Engineer + Shell Engineer

---

## Description

Automatizar la compilación y distribución de la APK Android directamente desde GitHub Actions y GitHub Releases, sumar actualización in-app con verificación SHA-256 (preparada con flag `PLAY_STORE_BUILD=1` para futuro Play Store) y exponer la descarga directa desde la UI web de Aegis (`ConnectionQR.tsx`).

## Acceptance Criteria

- [x] Job `build-android-apk` y empaquetado de `desktop-voice.tar.gz` integrados en `.github/workflows/publish-native.yml`.
- [x] `withAegisAssistant.js`, `aegisAssistantNative.ts`, `appUpdater.ts` y `settings.tsx` permiten buscar, verificar SHA-256 e instalar actualizaciones desde GitHub Releases (desactivable vía `PLAY_STORE_BUILD=1`).
- [x] `ConnectionQR.tsx` incluye pestaña *"2. Bajar APK"* con QR y botón de descarga directa de `aegis-android.apk`.
