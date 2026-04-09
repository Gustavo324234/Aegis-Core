# CORE-031 — shell/ui: stores Zustand

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/src/store/`
**Asignado a:** Shell Engineer
**Prioridad:** 🔴 Crítica — todos los componentes dependen del store
**Estado:** DONE
**Depende de:** CORE-030

---

## Contexto

El store Zustand es el cerebro de la UI. Gestiona auth, WebSocket, telemetría,
mensajes, Siren, y el estado de admin. Se porta desde el legacy con una
diferencia clave: las URLs apuntan a `ank-server` directamente — no hay BFF Python,
pero los endpoints son idénticos, por lo que el store **no necesita cambios de lógica**.

**Referencia:** `Aegis-Shell/ui/src/store/useAegisStore.ts` — portar sin cambios funcionales.

---

## Archivos a crear

### `shell/ui/src/store/useAegisStore.ts`

Portar desde `Aegis-Shell/ui/src/store/useAegisStore.ts` íntegramente.

El store incluye:
- Estado: `messages`, `status`, `system_metrics`, `tenantId`, `sessionKey`, `isAuthenticated`, `isAdmin`, `taskType`, `lastRoutingInfo`, `needsPasswordReset`, `adminActiveTab`, etc.
- Acciones: `authenticate()`, `connect()`, `sendMessage()`, `appendToken()`, `fetchTenants()`, `createTenant()`, `deleteTenant()`, `resetPassword()`, `startSirenStream()`, `stopSirenStream()`, `configureEngine()`, `logout()`
- Persistencia: `zustand/middleware persist` en `localStorage` bajo key `aegis-storage`
- Polling de telemetría cada 3 segundos vía `/api/status`

### `shell/ui/src/audio/TTSPlayer.ts`

Portar desde `Aegis-Shell/ui/src/audio/TTSPlayer.ts` — player de audio TTS via Web Audio API.

### `shell/ui/src/constants/enginePresets.ts`

Portar desde `Aegis-Shell/ui/src/constants/enginePresets.ts` — presets de providers.

### `shell/ui/src/i18n.ts`

Portar desde `Aegis-Shell/ui/src/i18n.ts`.

---

## Criterios de aceptación

- [x] `useAegisStore` exporta todos los tipos: `Message`, `MessageType`, `SystemStatus`, `TaskTypeValue`, `RoutingInfo`, `SystemMetrics`
- [x] `authenticate()` llama a `POST /api/auth/login` y retorna `'authenticated' | 'password_must_change' | 'failed'`
- [x] `connect()` abre WebSocket a `/ws/chat/{tenant_id}` con `Sec-WebSocket-Protocol: session-key.<key>`
- [x] `sendMessage()` envía JSON `{ prompt, task_type }` por el WebSocket
- [x] `startSirenStream()` abre WebSocket a `/ws/siren/{tenant_id}` y captura audio PCM 16kHz
- [x] `persist` guarda y restaura `isAuthenticated`, `tenantId`, `sessionKey`, `messages`
- [x] `npm run build` → 0 errores TypeScript

## Referencia

`Aegis-Shell/ui/src/store/useAegisStore.ts` — 483 líneas, portar completo
`Aegis-Shell/ui/src/audio/TTSPlayer.ts` — portar completo
