# Contributing to Aegis OS

Thank you for your interest in contributing. Aegis is a solo-developer project
opening up to the community — contributions of any kind are genuinely appreciated.

---

## Ways to Contribute

- **Bug reports** — open a GitHub Issue with reproduction steps
- **Feature requests** — open an Issue describing the use case and expected behavior
- **Code contributions** — see the workflow below
- **Documentation** — fixes, translations, examples
- **Spreading the word** — star the repo, share it, write about it

---

## ⚠️ Active Project Focus & Roadmap

With the successful completion of the stabilization and core intelligence epics (up to version 1.1.0), the project has transitioned to the next phase of its roadmap:
- **Active Focus:** Development effort is now directed toward:
  1. **Mobile Application (`app/`):** Completing the React Native / Expo client in both Satellite and Cloud modes.
  2. **Minimal Linux Distribution (`distro/`):** Designing the read-only immutable self-hosted system service image.
  3. **Cognitive Performance:** Scaling L3 Vector Memory (LanceDB) and optimizing cognitive scheduler loops.
- **General Contributions:** Bug reports, architectural enhancements, UI/UX refinement, and documentation translations are always welcome across all crates.

---

## 🌟 How to Find Open Issues

If you are looking to make your first contribution, check out the active roadmap items and untracked bugs:
1. Consult the single source of truth for tickets at [governance/TICKETS_MASTER.md](governance/TICKETS_MASTER.md).
2. Look for open issues on GitHub labeled `Good First Issue` or `Help Wanted`.
3. Open a design discussion issue before embarking on major architectural contributions so we can align on design principles.

---

## Before You Start

1. Check [governance/TICKETS_MASTER.md](governance/TICKETS_MASTER.md) for open tickets
2. If your change maps to an open ticket, mention it in your PR
3. If you're building something new, open an Issue first so we can discuss scope

---

## Development Setup

**Requirements:**
- Rust 1.80+
- Node.js 20+
- Linux (Ubuntu 22.04+ / Debian 12+) recommended for full testing (Windows/macOS are supported for local development)

### 1. Full Build (Embedded UI)
To compile a single production-ready binary with the web UI fully embedded (via Axum asset routing):
```bash
git clone https://github.com/Gustavo324234/Aegis-Core.git
cd Aegis-Core

# Build UI and embed it in the kernel binary
make build-embed

# Run the server
./target/release/ank-server
```
The web interface will be available at `http://localhost:8000`.

### 2. Isolated Backend Compilation
If you are only editing Rust kernel files and do not want to rebuild the React frontend every time, you can compile just the backend crates in isolation:
```bash
# Compile the main server binary without UI embedding
cargo build -p ank-server

# Run the server directly
cargo run -p ank-server
```

### 3. Development Mode (Hot-Reload / Split Dev)
To run both backend and frontend concurrently in development mode (avoiding constant rebuilds):
1. Start the React frontend dev server:
   ```bash
   cd shell/ui
   npm ci
   npm run dev
   ```
2. In another terminal, point the kernel to your built web assets via the `UI_DIST_PATH` env variable and run:
   * **Linux/macOS:**
     ```bash
     export UI_DIST_PATH=$(pwd)/shell/ui/dist
     cargo run -p ank-server
     ```
   * **Windows (PowerShell):**
     ```powershell
     $env:UI_DIST_PATH="$(Get-Location)\shell\ui\dist"
     cargo run -p ank-server
     ```

---

## Multi-Tenant & Local Test Simulation

Aegis OS uses a secure multi-tenant architecture enforced by the **Citadel Protocol**. Every cognitive cycle, agent loop, database record, and command execution is strictly bound to a `tenant_id`.

### Simulating Tenants via API Headers
To simulate requests from different tenants in your local test environment, pass the Citadel authentication and isolation headers in your HTTP/WebSocket requests:
- `x-aegis-tenant-id`: The unique identifier for the tenant (e.g., `tenant_test_123`).
- `x-aegis-session-key`: The cryptographic session key generated upon tenant linkage.

### Simulating Tenants via Administrative CLI
The administrative CLI (`ank-cli`) is the easiest way to test and verify multi-tenant isolation locally:
1. Build the CLI:
   ```bash
   cargo build -p ank-cli
   ```
2. Execute commands on behalf of a specific tenant using the `--tenant-id` global flag (or by setting the `AEGIS_TENANT_ID` environment variable):
   ```bash
   # Send a chat message on behalf of user_alpha
   ./target/debug/ank-cli --tenant-id user_alpha chat send "Evaluate current system status"
   
   # Send a message on behalf of user_beta to test complete database and file isolation
   ./target/debug/ank-cli --tenant-id user_beta chat send "List active files"
   ```
Workspaces and project databases are automatically provisioned and isolated at `users/<tenant_id>/workspace` in the data directory.

---

## Commit Convention

Aegis uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

Types:  fix | feat | chore | docs | refactor | test
Scope:  the affected crate or module (ank-core, ank-http, shell, installer, governance)

Examples:
  fix(ank-http): correct passphrase hashing in authenticate_tenant
  feat(ank-core): add context budget per AgentNode
  docs(installer): update CLI command reference
  chore(ci): add arm64 native runner
```

The release pipeline reads commit types to generate version bumps:
- `fix:` → patch
- `feat:` → minor
- `chore:` → no bump

---

## Branch Naming

```
fix/<short-description>         fix/auth-passphrase-hash
feat/<short-description>        feat/maker-capability
docs/<short-description>        docs/cli-reference
chore/<short-description>       chore/ci-arm64
```

---

## Pull Request Checklist

Before opening a PR, verify:

- [ ] `cargo fmt --all` passes
- [ ] `cargo clippy --workspace` passes with no warnings
- [ ] `cargo build --release` succeeds
- [ ] Commit messages follow Conventional Commits format
- [ ] PR description explains what changed and why

CI runs format → audit → clippy → test automatically on every PR.

---

## Code Standards

- **Zero-Panic policy** — `unwrap()` and `expect()` are denied at CI level via `clippy::unwrap_used`. Use `?` propagation and proper error handling.
- **Multi-tenant safety** — every data access must be scoped to a `tenant_id`. Never mix tenant data.
- **No runtime dependencies** — the kernel binary must remain dependency-free at runtime. No Python, no Node, no external services required for basic operation.
- **Citadel Protocol** — authentication and authorization must be enforced at every HTTP handler.

---

## Project Structure

| Directory | Language | Description |
|---|---|---|
| `kernel/crates/ank-server/` | Rust | Main binary entrypoint (Axum + gRPC) |
| `kernel/crates/ank-core/` | Rust | Cognitive engine — scheduler, VCM, agents, DAG |
| `kernel/crates/ank-http/` | Rust | HTTP/WebSocket server (Axum) with embedded React UI |
| `kernel/crates/ank-cli/` | Rust | Administrative CLI |
| `kernel/crates/ank-mcp/` | Rust | Model Context Protocol client |
| `kernel/crates/ank-proto/` | Rust | Protobuf contracts & generated Rust code |
| `kernel/crates/aegis-supervisor/` | Rust | Process manager |
| `kernel/crates/aegis-sdk/` | Rust | Wasm plugin SDK |
| `shell/ui/` | TypeScript/React | Web interface |
| `app/` | TypeScript/React Native | Mobile client |
| `installer/` | Bash/PowerShell | Deployment scripts and multiplatform installer |
| `governance/` | Markdown | Tickets, active epics, architecture, codex |

---

## Language

Code, comments, and PR descriptions should be in **English**.

Issue discussions and commit messages can be in English or Spanish — both are fine.

---

## Code of Conduct

Be respectful. Critique ideas, not people. Constructive feedback is always welcome.
If something feels off, open an issue or reach out directly.

---

## Questions?

Open a GitHub Issue with the `question` label. There are no stupid questions.
