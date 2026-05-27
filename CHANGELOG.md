# Changelog

All notable changes to Aegis OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

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
