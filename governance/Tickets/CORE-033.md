# CORE-033 — shell/ui: componentes de autenticación y onboarding

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/src/components/`
**Asignado a:** Shell Engineer
**Prioridad:** 🔴 Alta
**Estado:** DONE
**Depende de:** CORE-031

---

## Contexto

Flujo completo de autenticación y primer uso. Se porta desde el legacy.

**Referencia:** `Aegis-Shell/ui/src/components/`

---

## Componentes a portar

### `LoginScreen.tsx`
Pantalla de login con campos `tenant_id` + `passphrase`.
Llama a `authenticate()` del store. Maneja los tres resultados:
`authenticated` → App.tsx continúa, `password_must_change` → ForcePasswordChangeScreen,
`failed` → error inline.

### `AdminSetupScreen.tsx`
Setup del primer Master Admin cuando el sistema está en `STATE_INITIALIZING`.
Campos: username + password + confirmación.
Llama a `POST /api/admin/setup`.

### `BootstrapSetup.tsx`
Setup via OTP — detecta `?setup_token=...` en la URL.
Llama a `POST /api/admin/setup-token`.
Tras el éxito, limpia el token de la URL y redirige al login.

### `ForcePasswordChangeScreen.tsx`
Pantalla de cambio de password obligatorio.
Llama a `resetPassword()` del store para el tenant actual.

### `UserPasswordChange.tsx`
Cambio voluntario de password (desde Settings).

### `EngineSetupWizard.tsx`
Wizard de configuración del engine cognitivo.
Muestra presets de providers (OpenAI, Anthropic, Groq, etc.).
Al completar llama a `configureEngine()` del store.
Se omite si el admin ya configuró un engine global.

---

## Criterios de aceptación

- [x] `LoginScreen` llama correctamente a `authenticate()` y maneja los tres resultados
- [x] `BootstrapSetup` extrae el token de la URL y lo envía a `POST /api/admin/setup-token`
- [x] `AdminSetupScreen` crea el Master Admin y redirige al login
- [x] `ForcePasswordChangeScreen` completa el reset y actualiza `sessionKey` en el store
- [x] `EngineSetupWizard` muestra presets y llama a `configureEngine()`
- [x] `npm run build` → 0 errores TypeScript

## Referencia

`Aegis-Shell/ui/src/components/LoginScreen.tsx`
`Aegis-Shell/ui/src/components/BootstrapSetup.tsx`
`Aegis-Shell/ui/src/components/AdminSetupScreen.tsx`
`Aegis-Shell/ui/src/components/ForcePasswordChangeScreen.tsx`
`Aegis-Shell/ui/src/components/EngineSetupWizard.tsx`
