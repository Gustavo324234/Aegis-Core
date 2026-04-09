# CORE-035 — shell/ui: Siren UI — captura de audio y TTS playback

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/src/`
**Asignado a:** Shell Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-031

---

## Contexto

Interfaz de voz: botón de grabación, captura de audio PCM via Web Audio API,
y reproducción de TTS desde el kernel. La lógica de WebSocket Siren ya está
en `useAegisStore` — este ticket es solo la capa de UI.

**Referencia:** `Aegis-Shell/ui/src/components/ChatTerminal.tsx` (VoiceButton inline)
y `Aegis-Shell/ui/src/audio/TTSPlayer.ts`

---

## Trabajo requerido

### VoiceButton (dentro de ChatTerminal)
Botón con tres estados visuales:
- `idle` → ícono de micrófono, color neutro
- `listening` → ícono animado (pulsando), color verde
- `transcribing` → spinner, color amarillo

Al presionar: llama a `startSirenStream()`.
Al soltar o tras silencio detectado por VAD: `stopSirenStream()` se llama automáticamente.

### TTSPlayer — `src/audio/TTSPlayer.ts`
Player de audio que recibe chunks base64 del kernel y los reproduce
en secuencia sin gaps usando Web Audio API.
Portar desde `Aegis-Shell/ui/src/audio/TTSPlayer.ts` sin cambios.

---

## Criterios de aceptación

- [ ] El VoiceButton cambia de estado visualmente según `status` del store
- [ ] Presionar el botón llama a `startSirenStream()` del store
- [ ] `TTSPlayer.playChunk()` reproduce audio sin gaps
- [ ] `TTSPlayer.initialize()` llama a `AudioContext.resume()` tras gesto de usuario
- [ ] `npm run build` → 0 errores TypeScript

## Referencia

`Aegis-Shell/ui/src/audio/TTSPlayer.ts` — portar completo
