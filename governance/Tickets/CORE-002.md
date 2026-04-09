# CORE-002 — ank-proto: contratos Protobuf

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core — `kernel/crates/ank-proto/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — bloquea CORE-003, CORE-010
**Estado:** TODO
**Depende de:** CORE-001

---

## Contexto

`ank-proto` compila los archivos `.proto` a código Rust usando `tonic-build`.
Es la fuente de todos los tipos gRPC que usa el resto del sistema.

**Referencia legacy:** `Aegis-ANK/crates/ank-proto/` + `Aegis-ANK/proto/`

---

## Trabajo requerido

### 1. Copiar archivos proto

Copiar desde legacy a `kernel/proto/`:
- `Aegis-ANK/proto/kernel.proto` → `kernel/proto/kernel.proto`
- `Aegis-ANK/proto/siren.proto` → `kernel/proto/siren.proto`

Sin modificaciones — el contrato gRPC externo no cambia.

### 2. `kernel/crates/ank-proto/Cargo.toml`

```toml
[package]
name = "ank-proto"
version = "0.1.0"
edition = "2021"

[dependencies]
tonic.workspace = true
prost.workspace = true
prost-types.workspace = true

[build-dependencies]
tonic-build.workspace = true
prost-build.workspace = true
```

### 3. `kernel/crates/ank-proto/build.rs`

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "../../proto/kernel.proto",
                "../../proto/siren.proto",
            ],
            &["../../proto"],
        )?;
    Ok(())
}
```

### 4. `kernel/crates/ank-proto/src/lib.rs`

```rust
pub mod v1 {
    tonic::include_proto!("ank.v1");

    pub mod siren {
        tonic::include_proto!("ank.v1.siren");
    }
}
```

---

## Criterios de aceptación

- [ ] `cargo build -p ank-proto` compila sin errores
- [ ] Los tipos `ank_proto::v1::TaskRequest`, `TaskResponse`, `TaskEvent`, `PCB` existen y son usables
- [ ] Los tipos `ank_proto::v1::siren::AudioChunk`, `SirenEvent` existen
- [ ] `KernelServiceServer` y `KernelServiceClient` están disponibles desde `ank_proto`
- [ ] `SirenServiceServer` y `SirenServiceClient` están disponibles desde `ank_proto`
- [ ] `cargo clippy -p ank-proto -- -D warnings` → 0 warnings

## Referencia

`Aegis-ANK/crates/ank-proto/` — implementación legacy completa
`Aegis-ANK/proto/kernel.proto` — contrato a copiar exacto
`Aegis-ANK/proto/siren.proto` — contrato a copiar exacto
