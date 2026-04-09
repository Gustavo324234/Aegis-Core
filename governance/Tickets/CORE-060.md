# CORE-060 — CI: GitHub Actions — build + clippy + test unificado

**Épica:** 32 — Unified Binary
**Fase:** 7 — CI/CD y Governance
**Repo:** Aegis-Core — `.github/workflows/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🔴 Alta — sin CI no hay gate de calidad
**Estado:** DONE
**Depende de:** CORE-020

---

## Contexto

CI unificado para el monorepo. Un solo pipeline valida todo:
Rust (clippy + test + audit) + UI (build + lint) + Bash (shellcheck).

**Referencia:** `Aegis-ANK/.github/workflows/pr_check.yml`
y `Aegis-Shell/.github/workflows/pr_check.yml`

---

## Workflows a crear

### `.github/workflows/pr_check.yml` — SRE Firewall en cada PR

```yaml
name: SRE Firewall

on:
  pull_request:
    branches: [main]

jobs:
  rust:
    name: Rust — clippy + test + audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: clippy
        run: cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
      - name: test
        run: cargo test --workspace
      - name: audit
        run: cargo install cargo-deny && cargo deny check

  ui:
    name: UI — build + lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: cd shell/ui && npm ci && npm run build && npm run lint

  installer:
    name: Installer — shellcheck
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: shellcheck installer/*.sh
```

### `.github/workflows/release.yml` — Release Please automation

Misma configuración que el legacy, pero para el monorepo completo.
Genera CHANGELOG.md y tags de versión al hacer merge a `main`.

---

## Criterios de aceptación

- [ ] El workflow `pr_check.yml` corre en cada PR contra `main`
- [ ] `cargo clippy` con `-D clippy::unwrap_used` es el gate obligatorio
- [ ] `npm run build` valida la UI en cada PR
- [ ] `shellcheck` valida los scripts de installer en cada PR
- [ ] Los tres jobs deben pasar para poder hacer merge (branch protection)

## Referencia

`Aegis-ANK/.github/workflows/pr_check.yml`
`Aegis-Shell/.github/workflows/pr_check.yml`
