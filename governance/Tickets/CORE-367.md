# CORE-367: Harden Siren WebRTC/WS & Voice API: Citadel Auth & IDOR Protection, Secret Masking, PeerConnection Cleanup, and Opus Sample-Rate Fidelity

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `ank-http/src/routes/siren_api.rs` exposed `/api/siren/voices` without `CitadelAuthenticated`, ignored `auth` on `/api/siren/webrtc/offer/:tenant_id` (allowing cross-tenant IDOR signaling via `:tenant_id`), returned `api_key` and `stt_api_key` in plaintext on `GET /api/siren/config`, and accepted unsanitized `active_model.txt` model IDs and unbounded base64 PCM enrollment payloads; meanwhile `stt_download.rs` allowed any non-admin tenant to trigger multi-GB global Whisper model downloads and leaked `.downloading` temporary files on failure.
2. `webrtc_offer_handler` leaked `RTCPeerConnection` instances without closing on ICE disconnect/failure, accumulated unbounded `shared_audio` PCM buffers in memory, hardcoded `22050 Hz` resampling even for 16 kHz engines (`VoxtralDriver`), sent Opus frames without 20ms RTP pacing, and scheduled empty LLM tasks when STT returned empty transcripts (also affecting `ws/siren.rs`). Additionally, `WhisperLocalEngine` and `EspeakEngine` ran CLI subprocesses without timeouts or `kill_on_drop`.

## Acceptance Criteria
- [x] Enforce `CitadelAuthenticated` across all `siren_api.rs` routes, enforce tenant match on `/webrtc/offer/:tenant_id`, restrict `/api/stt/download` to master/root with temp-file cleanup and 4 GiB cap, mask `api_key`/`stt_api_key` on `GET /config` while preserving existing secrets when masked placeholders are resubmitted, and sanitize `active_model.txt` IDs.
- [x] Close `RTCPeerConnection` on `Failed | Disconnected | Closed` state transitions (without `Arc` cycles), cap WebRTC PCM buffers and per-tenant active sessions, pace 20ms Opus frames, skip LLM scheduling on empty STT transcripts in WebRTC and WS Siren handlers, expose `SirenEngine::sample_rate()`, and bound `whisper-cli`/`espeak-ng` execution with `kill_on_drop(true)` + timeouts.
