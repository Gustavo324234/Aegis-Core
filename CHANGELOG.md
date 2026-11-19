# Changelog

All notable changes to Aegis OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- **Connected Accounts — Google Calendar & Shared OAuth (`oauth_api.rs`, `google_calendar.rs`)**:
  - Per-tenant Google OAuth web endpoints (`/api/oauth/google/config`, `/api/oauth/google/start`, `/api/oauth/google/callback`, `/api/oauth/status`, `/api/oauth/tokens`, `/api/oauth/:provider`), supporting shared or custom OAuth Client IDs (e.g., Orion Creative Client ID) via encrypted `TenantDB` or `GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_CLIENT_ID`.
  - Agent tool `manage_google_calendar` supporting `status`, `list`, `create`, `update`, and `delete` with normalized local/RFC3339 timestamps and all-day events.
  - Unified `ConnectedAccountsTab` UI in Settings for Google OAuth configuration and WhatsApp QR linking.
- **Connected Accounts — WhatsApp Bridge, Anti-Spam & Persistent Contact Aliases**:
  - Node.js `@whiskeysockets/baileys` (`6.7.24`) sidecar (`tools/whatsapp_bridge/index.mjs`) managed by `ank_core::whatsapp::WhatsAppManager` and documented in `tools/whatsapp_bridge/README.md`.
  - Backend `/api/whatsapp/*` endpoints (`/status`, `/connect`, `/disconnect`, `/settings`, `/chats`, `/pending/:id/confirm`, `DELETE /api/whatsapp/pending/:id`).
  - Default confirmation mode (`require_confirmation = true`, 30-minute pending queue toggleable from chat or Settings) and per-tenant anti-spam rate limiter (`WhatsAppRateConfig`).
  - Encrypted tenant alias store (`KV_CONTACT_ALIASES`) exposed via `manage_whatsapp` actions `save_contact` and `list_contacts` plus `read_messages_with_aliases`.
- **Personal Assistant Experience & Antigravity SDK Integration (Epics 58–59)**:
  - Personal backend endpoints (`/api/personal/summary`, `/api/personal/facts/:id`, `/api/personal/reminders/:id`, `/api/personal/reminders/:id/complete`, `/api/personal/chronos/consolidate`, `/api/chronos/events`) backed by encrypted SQLite/SQLCipher `TenantDB`.
  - One-step onboarding (`PersonalQuickSetup`), Personal Assistant drawer (`PersonalAssistantDrawer`), avatar presets (`AssistantAvatar`), and browser neural voice selection (`naturalVoice.ts`).
  - Google Antigravity SDK delegation (`ank_core::agents::antigravity_runner`, `tools/antigravity_bridge/agy_worker.py`, `ide_hook.py`, `/api/antigravity/*`).
- **Infinite Chat Pagination & Visual-Only Clear (`chat_history.rs`, `ChatTerminal.tsx`)**:
  - Added `/api/chat/history` pagination (`before`, `limit`, `respect_clean`), `/api/chat/ui-state` (`clean_on_open`), and `/api/chat/visual-clear` to hide prior turns visually while keeping full persistent scrollback on demand.
- **Sandbox Web Server & Windows Update Tooling**:
  - Added `tools/agent_sandbox/launch_web_server.py` and updated `run_sandbox.bat` (`--server-only` flag supported) to run an isolated test `ank-server` on port `8090` (`AEGIS_SANDBOX_ENV`) without touching production `ProgramData` configs.
  - Added `update.bat` and `update.ps1` helpers to deploy compiled `ank-server.exe` release binaries and `shell/ui/dist` bundles on Windows.
- **Aegis Video Studio (`aegis.domain.video`) & Autopilot Production Pipeline (`video_studio/`, `video_api.rs`, `VideoStudioPanel.tsx`, [docs/VIDEO_STUDIO.md](docs/VIDEO_STUDIO.md))**:
  - Added 8-tool native microkernel module `aegis.domain.video` (`video_produce_auto`, `video_create_project`, `video_manage_scene`, `video_configure_engines`, `video_generate_assets`, `video_render_mp4`, `video_install_runtime`, `video_get_overview`) and `/api/video/*` HTTP routes with automatic portable runtime installation (FFmpeg + isolated Python venv with `edge-tts` and `Pillow`).
  - Added keyless free cloud AI backgrounds via **Pollinations AI (`pollinations_ai`, Flux)** with glass HUD title overlays (`maybe_write_hud_overlay`), **Pixabay Video & Photo API** support (`pixabay_stock` / auto-detected Pixabay keys alongside legacy `pexels_stock`), **Edge TTS HD**, **Motion Compositor 2.0 (`kinetic_card`)**, plus **Gemini Imagen/Veo/TTS**, **OpenAI**, **ElevenLabs**, and local **ComfyUI / Automatic1111 / Piper / Kokoro** endpoints.
  - Added resilient multi-stage fallback in `generate_assets_for_project` (voices degrade to `edge_tts`; visuals degrade via `pollinations_ai` before falling back to `kinetic_card`) tracked in `VideoProject.fallbacks_applied`.
  - Added host Hardware & API Capability Advisor (`probe_hardware_specs` cached 60s + `build_capability_advisor`) surfaced in both `VideoStudioPanel` and chat result cards (`ChatVideoResultCard`).
- **Live Tool Progress Streaming & Auto-Expanding Chat Input (`ws/chat.rs`, `ChatTerminal.tsx`, `useAegisStore.ts`, `agentEvents.ts`)**:
  - Streamed real-time `__PROGRESS__` events over `/ws/chat` as `tool_progress` frames rendered in `liveToolProgress` with a 180s active-tool watchdog extension.
  - Auto-expanded `ChatTerminal` input (`44px`–`280px` vertical resize, `max-w-5xl` stacked layout for prompts >85 chars or multiline) and widened long/video-card message bubbles (`max-w-[95%] sm:max-w-[90%]`).

### Changed
- **Reminder CRUD (`manage_reminder`)**: Extended beyond add/list/complete to support `update` and `delete`, plus normalized recurrence schedules (`none`, `daily`, `weekdays`, `weekly:...`, `monthly`).
- **UI Header Observability & Markdown Tables**: Added active provider + model display in `TelemetryDashboard` (`lastRoutingInfo`), grouped provider/model selector, and GFM Markdown table rendering (`remark-gfm`) in `ChatTerminal`.
- **Cognitive Router & Model Catalog**: Expanded catalog and model probing metadata (`models.yaml`, UI model catalog refresh/probing), prioritized stable free-tier routing options, and extended `model_unavailable` cooldown window in `rate_tracker.rs` from `300s` to `3600s`.
- **1-Click Automatic Video Studio UI (`VideoStudioPanel.tsx`)**: Made `"auto"` preset (`Automático ★`) the primary creation flow with advanced preset/engine selectors collapsed cleanly behind optional toggles.

### Fixed
- **ReAct Multi-Provider Failover & Empty Stream Recovery (`chal/mod.rs`, `router/mod.rs`)**:
  - Added `CognitiveRouter::decide_excluding` bypassing sticky model cache to cycle up to 3 provider/model hops across primary request errors, mid-stream failures, and post-tool empty completions (`stream_was_empty`), plus provider-aware tool re-serialization in `execute_agent_loop`.
  - Added `StreamTextSanitizer`, cross-iteration paragraph separators, and tightened `is_status_query` matching so conversational confirmations (such as `"ya está conectado"`) no longer misfire as agent-status shortcuts.
- **Video Request Routing, History Recovery & Supervisor Failure Reporting (`chal/mod.rs`, `router/modules.rs`, `orchestrator.rs`)**:
  - Prevented `is_video_production_request` and `is_video_advisory_or_followup_query` prompts from being misclassified into `spawn_agent(role="project_supervisor")` by `is_technical_request` / `CHAL Guard`.
  - Fixed `if recovered` empty-stream failover exit in `execute_with_decision` so text-only video promises from fallback models still flow through `CHAL Guard` (`resolve_video_request_prompt` + `video_produce_auto`), and handled mid-stream interruptions and short follow-up nudges cleanly.
  - Fixed `VideoScene` flexible parsing (`parse_flexible_scenes`, `#[serde(default)]` on `id`/`order`/`title`), LLM storyboard `fallback_chain` retries, and `split_text_to_scenes` (`extract_structured_scenes_from_prompt` + `strip_video_command_boilerplate`) so explicit numbered or parenthesized scenes produce clean narration without reading meta-instructions aloud.
  - Emitted `AgentEvent::Spawned`, `AgentEvent::StateChanged("Failed: ...")`, and `AgentEvent::SupervisorCompleted` error notifications from `orchestrator.rs` so background task failures surface visibly in chat.
- **SSE Multi-Byte UTF-8 Streaming (`chal/drivers/cloud.rs`)**: Switched cloud driver SSE line reading from lossy chunk-level UTF-8 decoding to byte-buffered `decode_next_sse_line_from_buf` over `Vec<u8>` so split multi-byte characters are preserved intact.
- **WhatsApp Contact Directory & History Ordering (`tools/whatsapp_bridge/index.mjs`)**:
  - Merged and deduplicated `@lid` vs `@s.whatsapp.net` entries by extracted digits (`buildMergedDirectory`), added accent/punctuation-insensitive matching across `name`/`verifiedName`/`notify`, exposed `GET /contacts`, raised caps (`MAX_CHATS = 300`, `MAX_CONTACTS = 2000`), and preserved timestamps in `messaging-history.set`.

### Planned
- Satellite & Cloud Mobile App completion (`app/` client)
- Minimal immutable self-hosted Linux distribution (`distro/`)
- Performance optimization and local vector database scaling (LanceDB L3)

---

## [1.1.0] — 2026-05-26

### Added
- **Epic 51: Model Intelligence** — Integrated PinchBench, Ollama Cloud support, real-time CMR v2 context scoring, and symmetric local-first routing.
- **Epic 52: Voice Quality** — Stabilized Siren audio streaming protocol, transitioned WebSocket to WebRTC/WebTransport channels, and added hardware-level mic-mute feedback loops during TTS.
- **Epic 53: Stabilization** — Realized ReAct LLM execution in `run_agent_loop`, integrated administrative service management, and created real-time Dashboard observability widgets (Kanban, API costs) with true multi-tenant database isolation.
- **Epic 54: Aegis Connect** — Implemented persistent secure WebSocket tunnels linked to Orion ID accounts, replacing ephemeral/random Cloudflare Quick Tunnels.
- **CORE-150: Sandbox Scripting** — Created Maker Capability for autonomous runtime JS sandboxing inside secure enclaves.
- **CORE-151: Project Context Integration** — Integrated active project file state, Git branch tracking, and VCM (Virtual Context Manager) deep context scoring.

---

## [1.0.0] — 2026-04-28

### Summary
First public release of Aegis Core — the unified monorepo replacing the legacy
multi-repo architecture (Aegis-ANK + Aegis-Shell + Aegis-Installer).

### Added
- **Single Rust binary** (`ank-server`) — HTTP API, WebSocket, and embedded React UI in one executable with no runtime dependencies (Epic 32)
- **Citadel Protocol** — Zero-Trust multi-tenant authentication at every layer
- **Virtual Context Manager (VCM)** — L1/L2/L3 memory hierarchy per tenant
- **Cognitive Model Router (CMR)** — dynamic model selection and optimization (Epic 42)
- **Hierarchical Multi-Agent Orchestration** — AgentTree, AgentOrchestrator, ProjectRegistry, SYS_AGENT_SPAWN syscall (Epic 43)
- **Developer Workspace** — integrated terminal, file browser, Git operations, GitHub PR manager with CI polling (Epic 44)
- **Cognitive Agent Architecture** — per-agent context budgets, instruction loading, state persistence, WebSocket agent event stream (Epic 45)
- **Cloudflare Tunnel integration** — automatic HTTPS remote access without port forwarding
- **Unified installer** — `install.sh` with native (systemd) and Docker modes, interactive setup, multi-arch (x86_64 + arm64)
- **Aegis CLI** — `aegis status/version/logs/diag/start/stop/restart/token/update`
- **Mobile app** — React Native / Expo client (in progress)
- **Plugin system** — Wasm-based plugin loading
- **Siren Protocol** — VAD + STT + TTS audio pipeline
- **MCP client** — Model Context Protocol integration

### Architecture
- Replaced Python BFF translation layer with direct Axum HTTP server
- SQLCipher encrypted data store per tenant
- DAG compiler for cognitive process scheduling
- gRPC :50051 for external CLI and multi-node federation

### Epics completed
- Epic 32: Unification (single binary)
- Epic 42: Realignment (auth, OAuth, model router, technical debt)
- Epic 43: Hierarchical Multi-Agent Orchestration
- Epic 44: Developer Workspace
- Epic 45: Cognitive Agent Architecture
- Epic 46: Public Launch (docs, community, open source health)

---

*Older development history predates this changelog and is available in git log.*
