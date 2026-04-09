# CLAUDE.md — Aegis Core

> Contexto para todos los agentes IA que trabajan en este repositorio.

---

## Rol de este repositorio

**Aegis-Core es el sistema nuevo.** No es una migración del código legacy —
es la implementación correcta de la arquitectura final.

Los repos legacy (`Aegis-ANK`, `Aegis-Shell`, `Aegis-Installer`, `Aegis-App`)
existen como **referencia de lectura**. Se consultan para entender qué hace
el sistema hoy, qué contratos existen, y qué debe quedar integrado.
**Nunca se modifica el legacy desde acá.**

---

## Estructura

```
aegis-core/
├── kernel/      ANK — Rust/Tokio (Kernel Engineer)
├── shell/       UI — React/TypeScript (Shell Engineer)
├── app/         Mobile — React Native/Expo (Mobile Engineer)
├── installer/   Deploy — Bash/Docker (DevOps Engineer)
├── governance/  Docs/Tickets (Arquitecto IA)
└── distro/      Linux distro — futuro
```

---

## Protocolo de inicio de sesión (OBLIGATORIO)

1. `get_workspace_overview()`
2. `get_governance_docs(AEGIS_CONTEXT)`
3. `get_governance_docs(TICKETS_MASTER)`

---

## Reglas universales

### Zero-Panic (Rust)
- Prohibido `.unwrap()`, `.expect()`, `panic!()`
- Errores via `Result<T, E>` con `anyhow` o `thiserror`
- CI gate: `cargo clippy -- -D warnings -D clippy::unwrap_used -D clippy::expect_used`

### Protocolo Citadel
- Auth: `tenant_id` + `session_key` (SHA-256 del passphrase)
- Headers HTTP: `x-citadel-tenant` + `x-citadel-key`
- Headers gRPC: mismos nombres via metadata
- Nunca exponer credenciales en logs, URLs ni respuestas

### Arquitectura del binario único
- `ank-server` levanta Axum (:8000) + Tonic (:50051) en el mismo proceso Tokio
- La UI React habla HTTP/WS directo con `ank-server` — sin BFF Python
- Aegis-App habla HTTP/WS con `ank-server` (ADR-022)
- `ank-cli` habla gRPC directo con `:50051`
- `kernel/proto/` es el contrato gRPC externo — cambios impactan clientes externos

### Trazabilidad
- Todo cambio → ticket en `governance/Tickets/`
- Estado global → `governance/TICKETS_MASTER.md`
- CHANGELOG.md por componente afectado
- Commits: Conventional Commits con ID de ticket

---

## Por agente

### Kernel Engineer (kernel/)
- Stack: Rust, Tokio, Tonic, Axum
- Consulta legacy: `Aegis-ANK` (leer, no modificar)
- Gate: `cargo build -p <crate>` + clippy

### Shell Engineer (shell/)
- Stack: React 18, TypeScript strict, Zustand, Tailwind, Vite
- Consulta legacy: `Aegis-Shell/ui/` (leer, no modificar)
- Gate: `npm run build` sin errores TypeScript

### DevOps Engineer (installer/)
- Stack: Bash 5+ con `set -euo pipefail`
- Consulta legacy: `Aegis-Installer` (leer, no modificar)
- Gate: `shellcheck` sin warnings

### Mobile Engineer (app/)
- Stack: React Native, Expo SDK 52, TypeScript
- Consulta legacy: `Aegis-App` (leer, no modificar)
- Gate: `npx expo export` sin errores

---

## Prohibiciones absolutas

- NUNCA modificar repos legacy desde este contexto
- NUNCA hardcodear paths — usar `dirs` crate o `AEGIS_DATA_DIR`
- NUNCA commitear `.env` ni credenciales
- NUNCA crear archivos batch de tickets — un ticket = un `.md`
- NUNCA hacer push a git (Tavo lo hace manualmente)
- NUNCA correr `cargo test` o `pytest` localmente (CI los corre)
