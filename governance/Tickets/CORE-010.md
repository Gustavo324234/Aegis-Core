# CORE-010 — ank-http: scaffolding del crate + AppState

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — bloquea CORE-011 a CORE-016
**Estado:** DONE
**Depende de:** CORE-003

---

## Contexto

`ank-http` es el crate nuevo que no existe en el legacy. Implementa el servidor
HTTP/WebSocket usando Axum, embebido en el mismo proceso Tokio que Tonic.

La UI React y la App mobile hablan con este servidor. Los endpoints son
funcionalmente idénticos a los que hoy sirve el BFF Python — mismas URLs,
mismos payloads JSON, mismo protocolo WebSocket. El BFF Python no se porta:
se reimplementa en Rust usando el legacy como especificación de comportamiento.

**Referencia de comportamiento:** `Aegis-Shell/bff/main.py`

---

## Trabajo requerido

### 1. `kernel/crates/ank-http/Cargo.toml`

```toml
[package]
name = "ank-http"
version = "0.1.0"
edition = "2021"

[dependencies]
ank-core  = { path = "../ank-core" }
ank-proto = { path = "../ank-proto" }

axum.workspace            = true
tower.workspace           = true
tower-http.workspace      = true
tokio.workspace           = true
tokio-stream.workspace    = true
serde.workspace           = true
serde_json.workspace      = true
anyhow.workspace          = true
thiserror.workspace       = true
tracing.workspace         = true
uuid.workspace            = true
sha2.workspace            = true
hex.workspace             = true
bytes.workspace           = true
futures.workspace         = true
futures-util.workspace    = true
async-trait.workspace     = true
dirs.workspace            = true

[dev-dependencies]
axum-test = "15"
tokio     = { workspace = true, features = ["test-util"] }
```

### 2. Estructura de módulos `kernel/crates/ank-http/src/`

```
src/
├── lib.rs              exports: AegisHttpServer, HttpConfig, AppState
├── config.rs           HttpConfig struct + from_env()
├── state.rs            AppState struct (Clone, compartido entre handlers)
├── citadel.rs          CitadelCredentials extractor + hash_passphrase()
├── error.rs            AegisHttpError → IntoResponse
├── routes/
│   ├── mod.rs          build_router() → axum::Router<AppState>
│   ├── auth.rs         POST /api/auth/login
│   ├── admin.rs        /api/admin/*
│   ├── engine.rs       /api/engine/*
│   ├── router_api.rs   /api/router/*
│   ├── workspace.rs    POST /api/workspace/upload
│   ├── providers.rs    POST /api/providers/models
│   ├── status.rs       GET /api/status, /api/system/*
│   └── siren_api.rs    GET/POST /api/siren/*
├── ws/
│   ├── mod.rs
│   ├── chat.rs         GET /ws/chat/{tenant_id}
│   └── siren.rs        GET /ws/siren/{tenant_id}
└── static_files.rs     GET /* → SPA React dist/
```

### 3. `src/state.rs` — AppState

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use ank_core::{
    chal::CognitiveHAL,
    citadel::identity::Citadel,
    router::syncer::CatalogSyncer,
    SchedulerEvent, StatePersistor,
};

#[derive(Clone)]
pub struct AppState {
    pub scheduler_tx: mpsc::Sender<SchedulerEvent>,
    pub event_broker: Arc<RwLock<HashMap<String, broadcast::Sender<SchedulerEvent>>>>,
    pub citadel: Arc<Mutex<Citadel>>,
    pub hal: Arc<RwLock<CognitiveHAL>>,
    pub catalog_syncer: Option<Arc<CatalogSyncer>>,
    pub persistence: Arc<dyn StatePersistor>,
    pub config: crate::config::HttpConfig,
}
```

### 4. `src/lib.rs` — AegisHttpServer

```rust
pub mod citadel;
pub mod config;
pub mod error;
pub mod routes;
pub mod state;
pub mod ws;
mod static_files;

pub use config::HttpConfig;
pub use state::AppState;

use anyhow::Result;
use std::net::SocketAddr;

pub struct AegisHttpServer {
    pub state: AppState,
}

impl AegisHttpServer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn serve(self) -> Result<()> {
        let port = self.state.config.port;
        let app = routes::build_router(self.state);
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
        tracing::info!("Aegis HTTP server listening on {}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}
```

---

## Criterios de aceptación

- [x] `cargo build -p ank-http` compila sin errores (cargo check ok, build blocked by env SQLCipher)
- [x] `AppState` es `Clone`
- [x] `AegisHttpServer::serve()` existe y es `async`
- [x] La estructura de módulos (`routes/`, `ws/`, `citadel`, `state`) compila aunque los handlers estén vacíos
- [x] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-Shell/bff/main.py` — especificación de endpoints a implementar en tickets siguientes
