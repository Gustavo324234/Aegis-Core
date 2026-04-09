# Aegis Core

**Cognitive Operating System — Unified Codebase**

Aegis Core is the unified implementation of the Aegis OS ecosystem. A single repository
containing the cognitive kernel, web interface, mobile app, and deployment tooling —
built around a single Rust binary that serves everything.

**Legacy repositories (read-only reference):**
- `Aegis-ANK` — original Rust kernel (reference for kernel logic)
- `Aegis-Shell` — original Python BFF + React UI (reference for UI and endpoints)
- `Aegis-Installer` — original installer scripts (reference for deployment logic)
- `Aegis-App` — original React Native app (reference for mobile)

---

## Architecture

```
Browser / Mobile App
        │  HTTP + WebSocket
        ▼
 ank-server  (single Rust binary)
        ├── HTTP :8000   ← REST API + WebSocket + serves React UI
        └── gRPC :50051  ← external clients, CLI, multi-node federation
```

No Python runtime. No translation layer. One process.

## Repository structure

```
aegis-core/
├── kernel/      Aegis Neural Kernel — Rust/Tokio (ank-server + ank-core + ank-http)
├── shell/       Web UI — React/Vite/TypeScript
├── app/         Mobile client — React Native/Expo
├── installer/   Deployment — Bash/systemd/Docker
├── governance/  Tickets, architecture docs, codex
└── distro/      (future) Linux distribution
```

## Status

| Component | Status |
|---|---|
| Kernel (ANK unified) | In progress — Epic 32 |
| Web UI | In progress — migrating from Aegis-Shell |
| Mobile App | In progress — migrating from Aegis-App |
| Installer | In progress — migrating from Aegis-Installer |
| Linux distro | Planned — post-Epic 32 |

## Build

To build the entire project (UI + Kernel) in sequence:

```bash
./build.sh
```

Or using `make`:

```bash
make build
```

### Build Options

*   **Standard Build**: Compiles the UI to `shell/ui/dist` and the Kernel. When running, you must provide `UI_DIST_PATH` unless the UI is at the default location.
    ```bash
    make build
    # Run with:
    UI_DIST_PATH=shell/ui/dist ./target/release/ank-server
    ```
*   **Embedded Build**: Compiles the UI and embeds it directly into the `ank-server` binary. No external files are needed at runtime.
    ```bash
    make build-embed
    # Run with:
    ./target/release/ank-server
    ```

## Philosophy

- **LLMs as ALUs** — not oracles. Deterministic execution engine over probabilistic compute.
- **Zero-Panic** — Rust kernel with `clippy::unwrap_used` denied at CI level.
- **Citadel Protocol** — Zero-Trust multi-tenant auth at every layer.
- **One binary** — single executable, no runtime dependencies.
- **Distro-ready** — designed to embed into a minimal Linux distribution.

## License

Apache 2.0
