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

## Before You Start

1. Check [governance/TICKETS_MASTER.md](governance/TICKETS_MASTER.md) for open tickets
2. If your change maps to an open ticket, mention it in your PR
3. If you're building something new, open an Issue first so we can discuss scope

---

## Development Setup

**Requirements:**
- Rust 1.80+
- Node.js 20+
- Linux (Ubuntu 22.04+ / Debian 12+) recommended for full testing

```bash
git clone https://github.com/Gustavo324234/Aegis-Core.git
cd Aegis-Core

# Build UI + kernel
make build-embed

# Run
./target/release/ank-server
```

The web interface will be available at `http://localhost:8000`.

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
| `kernel/crates/ank-core/` | Rust | Cognitive engine — scheduler, VCM, agents, DAG |
| `kernel/crates/ank-http/` | Rust | HTTP/WebSocket server (Axum) |
| `kernel/crates/ank-server/` | Rust | Main binary entrypoint |
| `kernel/crates/ank-cli/` | Rust | Administrative CLI |
| `shell/ui/` | TypeScript/React | Web interface |
| `app/` | TypeScript/React Native | Mobile client |
| `installer/` | Bash | Deployment scripts |
| `governance/` | Markdown | Tickets, architecture, codex |

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
