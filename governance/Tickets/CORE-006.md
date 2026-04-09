# CORE-006 — ank-cli: CLI administrativa

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core — `kernel/crates/ank-cli/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟢 Baja
**Estado:** DONE
**Depende de:** CORE-002

---

## Contexto

CLI administrativa que habla gRPC directo con `ank-server:50051`.
Permite gestionar tenants, keys y estado del kernel desde la terminal.

**Referencia legacy:** `Aegis-ANK/crates/ank-cli/src/`

---

## Trabajo requerido

Portar `Aegis-ANK/crates/ank-cli/src/` a `kernel/crates/ank-cli/src/` sin cambios.

### `kernel/crates/ank-cli/Cargo.toml`

```toml
[package]
name = "ank-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ank"
path = "src/main.rs"

[dependencies]
ank-proto = { path = "../ank-proto" }
tonic.workspace     = true
tokio.workspace     = true
anyhow.workspace    = true
clap.workspace      = true
serde_json.workspace = true
tracing.workspace   = true
tracing-subscriber.workspace = true
```

---

## Criterios de aceptación

- [x] `cargo build -p ank-cli` compila sin errores
- [x] `ank --help` muestra los subcomandos disponibles
- [x] `cargo clippy -p ank-cli -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-ANK/crates/ank-cli/src/` — fuente completa
