# Aegis OS

> **A cognitive operating system.** One binary. Zero runtime dependencies. LLMs as ALUs under a deterministic execution engine.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Build](https://github.com/Gustavo324234/Aegis-Core/actions/workflows/ci.yml/badge.svg)](https://github.com/Gustavo324234/Aegis-Core/actions)
[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink?logo=github)](https://github.com/sponsors/Gustavo324234)

---

## What is Aegis?

Aegis is a self-hosted cognitive operating system — a platform where AI agents run as first-class processes, with memory, scheduling, multi-tenancy, and tool execution built into the kernel.

It is not a chatbot wrapper. It is not a LangChain pipeline. It is a kernel-level runtime for autonomous cognitive workloads.

**Core ideas:**

- **LLMs as ALUs** — language models are probabilistic compute units under a deterministic scheduler, not oracles
- **Zero-Panic kernel** — written in Rust with `clippy::unwrap_used` denied at CI level
- **Citadel Protocol** — Zero-Trust multi-tenant authentication at every layer
- **One binary** — `ank-server` serves the HTTP API, WebSocket streams, and the React UI with no external runtime
- **Distro-ready** — designed to run as a system service, eventually embedded in a minimal Linux distribution

---

## Architecture

```
Browser / Mobile App
        │  HTTP + WebSocket
        ▼
 ank-server  (single Rust binary)
        │
        ├── ank-http    HTTP :8000  — REST API, WebSocket, embedded React UI
        ├── ank-core    Cognitive engine — scheduler, VCM, agents, DAG, plugins
        └── gRPC :50051 — external CLI, multi-node federation
```

The system is multi-tenant: each tenant gets an isolated cognitive environment with its own memory layers (L1/L2/L3), agent tree, and encrypted data store (SQLCipher).

See [ARCHITECTURE.md](ARCHITECTURE.md) for full detail.

---

## Quick Install

**Requirements:** Linux (Ubuntu 22.04+ / Debian 12+), `sudo`, `curl`

```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

The installer will guide you through:
1. **Installation mode** — Native (recommended) or Docker
2. **Inference profile** — Cloud (API keys), Local (Ollama), or Hybrid
3. **Hardware tier** — Laptop/VPS, Workstation, or SRE-grade server

After install, Aegis starts automatically and prints your setup URL:

```
################################################################
#          AEGIS OS — INSTALLATION COMPLETE                    #
################################################################

  Remote Access (HTTPS): https://your-tunnel.trycloudflare.com
  Local Setup URL:        http://192.168.1.x:8000?setup_token=...

  Token expires in 30 minutes.
  To regenerate: sudo aegis token
################################################################
```

Open the URL in your browser to complete onboarding.

---

## Aegis CLI

After installation, the `aegis` command is available system-wide.

### Status & Info

```bash
aegis status          # Service health and API connectivity
aegis version         # Installed version
aegis logs            # Follow live logs (last 50 lines)
aegis logs 100        # Follow last 100 lines
aegis diag            # Deep SRE diagnostic report
```

### Service Control

```bash
aegis start           # Start the service
aegis stop            # Stop the service
aegis restart         # Restart the service
aegis token           # Print setup URL with fresh token
```

### Updates

```bash
aegis update          # Update to latest stable release
aegis update --beta   # Update to latest nightly build (from main)
aegis update --stable # Explicitly target stable channel
```

---

## Build from Source

**Requirements:** Rust 1.80+, Node.js 20+

```bash
git clone https://github.com/Gustavo324234/Aegis-Core.git
cd Aegis-Core

# Full build: UI + embedded binary
make build-embed

# Run
./target/release/ank-server
```

Build options:

| Command | Output | Notes |
|---|---|---|
| `make build` | Binary + separate UI assets | Set `UI_DIST_PATH=shell/ui/dist` at runtime |
| `make build-embed` | Single self-contained binary | No external files needed |
| `./build.sh` | Same as `make build` | Shell script alternative |

---

## Repository Structure

```
aegis-core/
├── kernel/          Rust kernel — ank-server, ank-core, ank-http, ank-cli
├── shell/ui/        Web interface — React 18 / Vite / TypeScript / Tailwind
├── app/             Mobile client — React Native / Expo
├── installer/       Deployment — install.sh, aegis CLI, systemd service
├── governance/      Tickets, architecture docs, codex
└── distro/          (future) Linux distribution
```

---

## Completed Milestones

| Epic | Title | Status |
|---|---|---|
| Epic 32 | Unification — single Rust binary | ✅ Done |
| Epic 42 | Realignment — auth, OAuth, model router | ✅ Done |
| Epic 43 | Hierarchical Multi-Agent Orchestration | ✅ Done |
| Epic 44 | Developer Workspace (terminal, file browser, Git, PR manager) | ✅ Done |
| Epic 45 | Cognitive Agent Architecture | ✅ Done |

---

## Roadmap

- [ ] Epic 46 — Public Launch (docs, community, open source health)
- [ ] Sandbox scripting (Maker Capability) — CORE-150
- [ ] Project context integration (Git/VCM) — CORE-151
- [ ] Mobile app completion
- [ ] `distro/` — minimal Linux distribution

---

## Contributing

Aegis is open source and welcomes contributions.

Read [CONTRIBUTING.md](CONTRIBUTING.md) to get started. All contributions — code, docs, translations, bug reports — are valued.

The project uses a ticket-driven workflow. Check [governance/TICKETS_MASTER.md](governance/TICKETS_MASTER.md) for open work.

---

## Supporting the Project

Aegis is built and maintained by a solo developer. If it's useful to you, consider supporting its development:

- ⭐ **Star the repo** — helps with visibility
- 🐛 **Report bugs** — open an issue
- 💬 **Spread the word** — share with people building AI systems
- ❤️ **Sponsor** — [github.com/sponsors/Gustavo324234](https://github.com/sponsors/Gustavo324234)

Sponsorships go directly toward development infrastructure: compute, API costs, and tooling.

---

## Philosophy

Aegis is built on a few firm beliefs:

**LLMs are ALUs, not oracles.** A language model is a probabilistic compute unit that transforms tokens. The system's intelligence comes from the deterministic layer that orchestrates those transforms — the scheduler, the memory hierarchy, the agent tree. The model is a tool, not the mind.

**Kernel-level cognition.** AI workloads should be managed the same way an OS manages processes: scheduling, isolation, resource limits, inter-process communication. Not as a library call, as a kernel service.

**One binary.** Operational complexity is a form of technical debt. A system that runs as a single executable, with no Python runtime, no Node daemon, no Docker required, is a system that can actually be maintained.

---

## License

MIT — see [LICENSE](LICENSE)

Copyright (c) 2026 Gustavo Aversente
