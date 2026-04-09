# CORE-051 — app: stores, servicios y tipos portados

**Épica:** 32 — Unified Binary
**Fase:** 6 — Mobile App
**Repo:** Aegis-Core — `app/src/`
**Asignado a:** Mobile Engineer
**Prioridad:** 🟡 Media
**Estado:** TODO
**Depende de:** CORE-050

---

## Contexto

Portar toda la lógica de negocio de la app — stores Zustand, servicios
de auth, cliente BFF, router cloud y servicios de voz.

**Referencia:** `Aegis-App/src/`

---

## Archivos a portar (sin cambios funcionales)

### `app/src/stores/`
- `authStore.ts` — sesión, tenantId, sessionKey, serverUrl, mode
- `chatStore.ts` — mensajes, streaming, estado del chat

### `app/src/services/`
- `bffClient.ts` — cliente HTTP para `ank-server` (login, upload, etc.)
- `cloudRouter.ts` — llamadas directas a OpenAI/Anthropic/Groq/Gemini/etc.
- `voiceService.ts` — STT + TTS via OS APIs + Siren bridge WS
- `secureStorage.ts` — wrapper de `expo-secure-store`
- `permissionService.ts` — solicitud contextual de permisos

### `app/src/types/`
- `ChatMessage.ts`, `UiState.ts`, `AuthCredentials.ts`, `Provider.ts`

### `app/src/constants/`
- `providers.ts` — lista de providers cloud soportados

---

## Criterios de aceptación

- [ ] `authStore` persiste sesión en `expo-secure-store`
- [ ] `bffClient.login()` llama a `POST /api/auth/login` con SHA-256 del passphrase
- [ ] `cloudRouter` soporta OpenAI, Anthropic, Groq, Grok, Gemini, OpenRouter
- [ ] `voiceService` conecta al WS Siren en modo Satellite
- [ ] `npx expo export` sin errores TypeScript

## Referencia

`Aegis-App/src/` — portar completo
