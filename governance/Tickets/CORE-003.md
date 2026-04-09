# CORE-003 — ank-core: motor cognitivo

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core — `kernel/crates/ank-core/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — bloquea CORE-010, CORE-020
**Estado:** DONE
**Depende de:** CORE-002

---

## Contexto

`ank-core` es el motor cognitivo central. Contiene toda la lógica del sistema:
scheduler, DAG, VCM, Citadel, HAL, plugins, Siren, MCP.

Este ticket porta el contenido de `Aegis-ANK/crates/ank-core/` a
`kernel/crates/ank-core/` **sin cambios funcionales**. El objetivo es que
compile idénticamente al legacy. Las mejoras (ank-http, etc.) vienen en tickets
posteriores.

**Referencia legacy:** `Aegis-ANK/crates/ank-core/src/`

---

## Módulos a portar

Leer cada módulo del legacy y recrearlo en `kernel/crates/ank-core/src/`:

| Módulo | Path legacy | Descripción |
|---|---|---|
| `scheduler` | `src/scheduler/` | CognitiveScheduler, PCB, SchedulerEvent |
| `chal` | `src/chal/` | CognitiveHAL, drivers (cloud, native) |
| `dag` | `src/dag/` | S-DAG compiler, GraphManager |
| `vcm` | `src/vcm/` | Virtual Context Manager, paginación |
| `citadel` | `src/citadel/` | Protocolo Citadel, identity management |
| `enclave` | `src/enclave/` | MasterEnclave, SQLCipher |
| `plugins` | `src/plugins/` | PluginManager, Wasm loader, hot-reload |
| `syscalls` | `src/syscalls/` | Neural Syscalls, StreamInterceptor |
| `scribe` | `src/scribe/` | ScribeManager, workspace I/O |
| `router` | `src/router/` | CognitiveRouter, KeyPool, ModelCatalog, CatalogSyncer |
| `siren` | `src/router/siren.rs` | SirenEngine trait, SirenRouter, VoxtralDriver |
| `persistence` | `src/scheduler/persistence.rs` | SQLCipherPersistor, StatePersistor |
| `lib.rs` | `src/lib.rs` | exports públicos |

### `kernel/crates/ank-core/Cargo.toml`

```toml
[package]
name = "ank-core"
version = "0.1.0"
edition = "2021"

[dependencies]
ank-proto  = { path = "../ank-proto" }
ank-mcp    = { path = "../ank-mcp" }

tokio.workspace       = true
tonic.workspace       = true
serde.workspace       = true
serde_json.workspace  = true
serde_yaml.workspace  = true
anyhow.workspace      = true
thiserror.workspace   = true
uuid.workspace        = true
chrono.workspace      = true
tracing.workspace     = true
async-trait.workspace = true
tokio-stream.workspace = true
futures.workspace     = true
futures-util.workspace = true
regex.workspace       = true
reqwest.workspace     = true
bytes.workspace       = true
prost-types.workspace = true
wasmtime.workspace    = true
wasmtime-wasi.workspace = true
mdns-sd.workspace     = true
sha2.workspace        = true
hmac.workspace        = true
base64.workspace      = true
ed25519-dalek.workspace = true
argon2.workspace      = true
rusqlite.workspace    = true
sysinfo.workspace     = true
git2.workspace        = true
arrow.workspace       = true
arrow-array.workspace = true
arrow-schema.workspace = true
arrow-buffer.workspace = true
notify.workspace      = true
notify-debouncer-mini.workspace = true
dirs.workspace        = true
encoding_rs           = "0.8.35"

llama-cpp-2 = { workspace = true, optional = true }

[features]
default     = []
local_llm   = ["dep:llama-cpp-2"]
full_local  = ["local_llm"]

[dev-dependencies]
tempfile = "3"
```

---

## Criterios de aceptación

- [x] `cargo build -p ank-core` compila sin errores
- [x] `cargo test -p ank-core` pasa (mismos tests que en legacy)
- [x] `cargo clippy -p ank-core -- -D warnings -D clippy::unwrap_used -D clippy::expect_used` → 0 warnings
- [x] `CognitiveScheduler`, `Citadel`, `CognitiveHAL`, `MasterEnclave` son públicos desde `ank_core`
- [x] `CognitiveRouter`, `KeyPool`, `ModelCatalog` son públicos desde `ank_core::router`
- [x] `SirenRouter`, `SirenEngine` son públicos desde `ank_core::router` (o `ank_core::siren`)
- [x] `SQLCipherPersistor`, `StatePersistor` son públicos desde `ank_core`

## Notas

- No agregar funcionalidad nueva en este ticket — solo portar
- Si hay código que depende de paths `/app/data/` hardcodeados en el legacy,
  reemplazar con `dirs::data_dir().join("aegis")` (ya resuelto en Epic 31 del legacy,
  verificar que esté correctamente portado)
- El trait `StatePersistor` y `SQLCipherPersistor` deben estar en este crate,
  no en `ank-server`

## Referencia

`Aegis-ANK/crates/ank-core/src/` — fuente completa a portar
