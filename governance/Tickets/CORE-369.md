# CORE-369: Harden Shell UI: Chat Auto-Reconnect, Logout State Purge, Agent/Train WS Protocol Alignment, Siren STT Recovery, and Audio Safety

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Shell Engineer

## Description
1. In `shell/ui/src/store/useAegisStore.ts`, transient Chat WebSocket errors set `status: 'error'` and prevented `onclose` / `App.tsx` from ever reconnecting until a full page reload (which forced re-login), while closing a previous socket during `connect()` left stale `onclose` handlers attached. Furthermore, `logout()` left plaintext `messages`, `agentTree`, `activeProjects`, and `sttApiKey` in memory and persisted `messages` in `localStorage` (`aegis-storage`), leaking prior tenant conversations on shared browsers despite encrypted server-side history (`loadChatHistory`).
2. `connectAgentStream()` expected PascalCase `AgentEvent` tags and `{ tree }` whereas `ank-core::agents::event::AgentEvent` emits `snake_case` tags (`tree_snapshot` with `{ nodes }`, externally-tagged `AgentRole`, etc.), `connectTrainingStream()` connected to `/api/train/progress` without Citadel `session-key` subprotocol credentials, Siren WebRTC DataChannel ignored `STT_EMPTY` (freezing UI on `'transcribing'` and breaking `conversation` mode mic reactivation), `ConnectionQR.tsx` omitted Citadel auth headers, `SttModelManager.tsx` swallowed `403`/`400` download errors silently, and `TTSPlayer.ts` crashed with `RangeError` on odd-byte PCM payloads while hardcoding a `22050 Hz` `AudioContext`.

## Acceptance Criteria
- [x] Add exponential-backoff Chat WebSocket auto-reconnect (suppressing reconnect on explicit `logout`/`disconnect` or `auth_failed`/`password_must_change`) and detach stale socket handlers before replacement; purge sensitive state on `logout()` and stop persisting plaintext `messages` in `localStorage`.
- [x] Normalize `snake_case` `AgentEvent` / `TreeSnapshot` payloads in `connectAgentStream()`, authenticate `/api/train/progress/{tenantId}` via `session-key.<key>` subprotocol, handle `STT_EMPTY` and `STT_ERROR` cleanly in Siren WebRTC DataChannel, attach Citadel headers in `ConnectionQR.tsx`, surface HTTP error messages in `SttModelManager.tsx`, and guard odd-byte `Int16Array` decoding + native `AudioContext` sample rates in `TTSPlayer.ts`.
