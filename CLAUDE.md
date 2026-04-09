# CLAUDE.md — Aegis Core

> Contexto para todos los agentes IA que trabajan en este repositorio.
> **Repo activo único:** `Aegis-Core` — todo el desarrollo ocurre acá.

---

## Repositorio de trabajo

```
Aegis-Core/        ← ÚNICO REPO ACTIVO — todo se escribe acá
```

### Repos legacy (solo lectura, nunca modificar)
```
Aegis-ANK/         → referencia de lógica del kernel Rust
Aegis-Shell/       → referencia de endpoints HTTP y UI
Aegis-Installer/   → referencia de scripts de deployment
Aegis-App/         → referencia de lógica mobile
Aegis-Governance/  → referencia de normativa histórica
```

---

## Protocolo de inicio de sesión (OBLIGATORIO para todos los agentes)

```
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/TICKETS_MASTER.md")
3. read_file("Aegis-Core", "governance/AEGIS_CONTEXT.md")
```

---

## Estructura del repo

```
Aegis-Core/
├── kernel/        ANK — Rust/Tokio (Kernel Engineer)
├── shell/ui/      Web UI — React/TypeScript (Shell Engineer)
├── app/           Mobile — React Native/Expo (Mobile Engineer)
├── installer/     Deploy — Bash/Docker (DevOps Engineer)
├── governance/    Tickets, docs, codex (Arquitecto IA)
└── distro/        Linux distro — futuro
```

---

## Leyes SRE (no negociables)

### Zero-Panic (Rust)
- Prohibido: `.unwrap()`, `.expect()`, `panic!()`
- Obligatorio: errores via `Result<T, E>` con `anyhow` o `thiserror`
- CI gate: `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used`

### Strict Shell (Bash)
- Obligatorio: `set -euo pipefail` en todos los scripts
- CI gate: `shellcheck` sin warnings

### TypeScript estricto
- Obligatorio: `strict: true` en `tsconfig.json`
- CI gate: `npm run build` sin errores TypeScript

---

## Protocolo Citadel

- Auth: `tenant_id` + SHA-256(passphrase)
- Headers HTTP: `x-citadel-tenant` + `x-citadel-key`
- WebSocket: `Sec-WebSocket-Protocol: session-key.<hash>`
- Nunca exponer credenciales en logs, URLs ni respuestas

---

## Reglas absolutas

- NUNCA modificar repos legacy
- NUNCA hardcodear paths — usar crate `dirs` o `AEGIS_DATA_DIR`
- NUNCA commitear `.env` ni `AEGIS_ROOT_KEY`
- NUNCA hacer push a git (Tavo lo hace manualmente)
- NUNCA correr `cargo test` localmente — CI los corre en cada PR
- NUNCA crear archivos de tickets fuera de `governance/Tickets/`
- Un ticket = un archivo `.md` — nunca batch

---

## Tickets

- IDs: `CORE-XXX`
- Ubicación: `governance/Tickets/CORE-XXX.md`
- Estado global: `governance/TICKETS_MASTER.md`
- Al cerrar un ticket: actualizar estado en `TICKETS_MASTER.md`
- Commits: Conventional Commits con ID del ticket (`feat(ank-http): CORE-012 ...`)
