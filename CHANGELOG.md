# Changelog

All notable changes to Aegis OS are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### In Progress
- CORE-148: Natural conversational tone (prompt tuning)
- CORE-151: Project context integration (Git/VCM)

### Planned
- CORE-150: Sandbox scripting — Maker Capability
- Epic 46: Public launch infrastructure

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

---

*Older development history predates this changelog and is available in git log.*
