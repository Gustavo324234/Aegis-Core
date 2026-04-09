# CORE-001 — Workspace Cargo.toml raíz + estructura de crates

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — bloquea toda la Fase 1
**Estado:** TODO
**Depende de:** ninguno

---

## Contexto

El `Cargo.toml` raíz de Aegis-Core debe definir el workspace completo con todas
las dependencias compartidas. El archivo actual en `Cargo.toml` es un placeholder —
este ticket lo convierte en el workspace funcional.

**Referencia legacy:** `Aegis-ANK/Cargo.toml`

---

## Trabajo requerido

### 1. Reemplazar `Cargo.toml` raíz con workspace funcional

El archivo debe quedar así (expandir el placeholder existente):

```toml
[workspace]
resolver = "2"
members = [
    "kernel/crates/ank-proto",
    "kernel/crates/ank-core",
    "kernel/crates/ank-http",
    "kernel/crates/ank-server",
    "kernel/crates/ank-cli",
    "kernel/crates/ank-mcp",
    "kernel/crates/aegis-supervisor",
    "kernel/crates/aegis-sdk",
    "kernel/plugins_src",
]
resolver = "2"

[workspace.dependencies]
# Async runtime
tokio          = { version = "1.41", features = ["full", "tracing"] }
tokio-stream   = "0.1"
futures        = "0.3"
futures-util   = "0.3"
async-trait    = "0.1"
async-stream   = "0.3"

# gRPC
tonic          = { version = "0.13", features = ["tls-ring", "tls-webpki-roots", "transport"] }
tonic-build    = "0.13"
prost          = "0.13.3"
prost-types    = "0.13.3"
prost-build    = "0.13.3"

# HTTP (ank-http)
axum           = { version = "0.7", features = ["ws", "multipart"] }
tower          = "0.4"
tower-http     = { version = "0.5", features = ["cors", "fs", "trace"] }
tokio-tungstenite = "0.21"
http-body-util = "0.1"

# Serialization
serde          = { version = "1.0", features = ["derive"] }
serde_json     = "1.0"
serde_yaml     = "0.9"
bytes          = "1.0"

# Error handling
anyhow         = "1.0"
thiserror      = "2.0"

# Logging / Tracing
tracing        = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender   = "0.2"

# Crypto / Security
sha2           = "0.10"
hmac           = "0.12"
base64         = "0.22"
ed25519-dalek  = "2.1"
argon2         = "0.5.3"

# Utilities
uuid           = { version = "1.10", features = ["v4", "serde"] }
chrono         = { version = "0.4.40", features = ["serde"] }
regex          = "1.10"
dirs           = "5.0"
toml           = "0.8"
hex            = "0.4"

# Database
rusqlite       = { version = "0.33.0", features = ["bundled-sqlcipher"] }

# System
sysinfo        = "0.32"
git2           = "0.20"
notify         = "6.1.1"
notify-debouncer-mini = "0.4.1"
mdns-sd        = "0.11"

# AI / Wasm
llama-cpp-2    = "0.1"
wasmtime       = { version = "36", features = ["async"] }
wasmtime-wasi  = { version = "36", features = ["preview1"] }
reqwest        = { version = "0.12", default-features = false, features = ["rustls-tls", "stream", "json", "multipart"] }

# Arrow (VCM)
arrow          = "58"
arrow-array    = "58"
arrow-schema   = "58"
arrow-buffer   = "58"

# CLI
clap           = { version = "4", features = ["derive"] }

[profile.release]
opt-level     = 3
lto           = true
codegen-units = 1
strip         = true

[profile.dev]
opt-level     = 0
debug         = true
```

### 2. Crear los directorios de crates vacíos

Crear un `Cargo.toml` mínimo en cada crate para que el workspace compile:

```
kernel/crates/ank-proto/Cargo.toml
kernel/crates/ank-core/Cargo.toml
kernel/crates/ank-http/Cargo.toml
kernel/crates/ank-server/Cargo.toml
kernel/crates/ank-cli/Cargo.toml
kernel/crates/ank-mcp/Cargo.toml
kernel/crates/aegis-supervisor/Cargo.toml
kernel/crates/aegis-sdk/Cargo.toml
kernel/plugins_src/Cargo.toml
```

Cada uno con formato mínimo:
```toml
[package]
name = "ank-proto"   # ajustar por crate
version = "0.1.0"
edition = "2021"
```

### 3. Crear `kernel/crates/*/src/lib.rs` vacíos

Cada crate necesita al menos un archivo fuente para compilar:
```rust
// placeholder — ver ticket CORE-00X para implementación
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` desde la raíz de Aegis-Core termina sin errores de resolución
- [ ] Todos los crates aparecen en `cargo metadata --workspace`
- [ ] `kernel/crates/ank-http/` existe como crate (es nuevo, no existe en legacy)
- [ ] El workspace compila aunque los crates estén vacíos (solo `lib.rs` placeholder)

## Referencia

`Aegis-ANK/Cargo.toml` — dependencias actuales del legacy
