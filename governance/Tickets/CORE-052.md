# CORE-052 — app: pantallas y navegación

**Épica:** 32 — Unified Binary
**Fase:** 6 — Mobile App
**Repo:** Aegis-Core — `app/`
**Asignado a:** Mobile Engineer
**Prioridad:** 🟡 Media
**Estado:** TODO
**Depende de:** CORE-051

---

## Contexto

Portar las pantallas y la navegación de la app. Expo Router v4 con
file-based routing. La estructura de rutas es idéntica al legacy.

**Referencia:** `Aegis-App/app/`

---

## Archivos a portar

### Routing — `app/app/`
- `_layout.tsx` — root layout con auth guard
- `index.tsx` — redirect a auth o chat
- `(auth)/` — pantallas de login y setup
  - `login.tsx` — LoginScreen (servidor + email + password)
  - `cloud-setup.tsx` — CloudSetupScreen (API keys por provider)
- `(main)/` — pantallas principales
  - `chat.tsx` — ChatScreen con ModeSelector
  - `settings.tsx` — SettingsScreen
  - `voice.tsx` — VoiceScreen

### Componentes — `app/src/components/`
- `ChatBubble.tsx` — burbuja de mensaje con StreamingCursor
- `ModeSelector.tsx` — Satellite / Cloud / Hybrid
- `VoiceButton.tsx` — botón de grabación

---

## Criterios de aceptación

- [ ] La app navega correctamente entre auth y chat
- [ ] El auth guard redirige a login si no hay sesión
- [ ] `ChatScreen` muestra mensajes y envía prompts
- [ ] `ModeSelector` cambia entre Satellite y Cloud mode
- [ ] El fallback automático (server offline → Cloud mode) funciona
- [ ] `npx expo export` sin errores

## Referencia

`Aegis-App/app/` — routing completo a portar
`Aegis-App/src/components/` — componentes a portar
