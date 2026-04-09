# CORE-020 — ank-server: main.rs — Axum + Tonic en mismo proceso Tokio

**Épica:** 32 — Unified Binary
**Fase:** 3 — Entrypoint unificado
**Repo:** Aegis-Core — `kernel/crates/ank-server/src/main.rs`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — integra todo
**Estado:** DONE
**Depende de:** CORE-003, CORE-010, CORE-016

---

## Contexto

`ank-server` es el binario final. Su `main.rs` inicializa todos los componentes
del kernel y levanta Axum (:8000) y Tonic (:50051) en el mismo runtime Tokio.

**Referencia:** `Aegis-ANK/crates/ank-server/src/main.rs` — portar y extender.

---

## Trabajo requerido

### Secuencia de arranque en `main.rs`

```
1. Inicializar tracing (stdout + file rolling)
2. Leer AEGIS_ROOT_KEY del entorno
3. resolve_data_dir() → OS-native o AEGIS_DATA_DIR override
4. Inicializar SQLCipherPersistor (scheduler_state.db)
5. Inicializar MasterEnclave (admin.db)
6. Generar setup_token si no existe admin (imprimir URL en logs)
7. Inicializar CognitiveScheduler (Tokio spawn)
8. Inicializar PluginManager + watch_plugins_dir (Tokio spawn)
9. Inicializar CognitiveHAL + registrar drivers
10. Inicializar CognitiveRouter + KeyPool + CatalogSyncer
11. Construir AppState (compartido entre Axum y Tonic)
12. Construir AnkRpcServer (Tonic) usando AppState
13. Construir AegisHttpServer (Axum) usando AppState
14. Tokio::spawn(tonic_server.serve("0.0.0.0:50051"))
15. axum_server.serve("0.0.0.0:8000").await  ← tarea principal
```

### Clave: AppState compartido

```rust
// main.rs
let shared_state = AppState {
    scheduler_tx: scheduler_tx.clone(),
    event_broker:  Arc::clone(&event_broker),
    citadel:       Arc::clone(&citadel_lock),
    hal:           Arc::clone(&hal),
    catalog_syncer: catalog_syncer_opt.clone(),
    persistence:   Arc::clone(&persistence),
    config:        HttpConfig::from_env(),
};

// Tonic usa los mismos Arc<> — sin copia de datos
let tonic_server = AnkRpcServer::from_state(&shared_state);

// Axum usa el AppState directamente
let http_server = AegisHttpServer::new(shared_state);

tokio::spawn(async move {
    tonic_server.serve().await.expect("gRPC server failed");
});

http_server.serve().await?;
```

### TLS / mTLS

Misma lógica que el legacy:
- Si `AEGIS_TLS_CERT` y `AEGIS_TLS_KEY` están seteados → TLS en Tonic
- Si `AEGIS_MTLS_STRICT=true` y no hay certs → `bail!()` (no arrancar inseguro)
- Si `AEGIS_MTLS_STRICT=false` → arrancar en modo desarrollo sin TLS

Axum no usa TLS directamente (está detrás del supervisor o un reverse proxy).

### `kernel/crates/ank-server/Cargo.toml`

```toml
[package]
name = "ank-server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ank-server"
path = "src/main.rs"

[dependencies]
ank-core       = { path = "../ank-core" }
ank-proto      = { path = "../ank-proto" }
ank-http       = { path = "../ank-http" }

tokio.workspace             = true
tonic.workspace             = true
anyhow.workspace            = true
tracing.workspace           = true
tracing-subscriber.workspace = true
tracing-appender.workspace  = true
dirs.workspace              = true
uuid.workspace              = true

[features]
full_local = ["ank-core/full_local"]
```

---

## Criterios de aceptación

- [ ] `cargo build -p ank-server` produce el binario `ank-server`
- [ ] El binario arranca y levanta Axum en :8000 y Tonic en :50051
- [ ] `GET http://localhost:8000/health` retorna 200
- [ ] `grpcurl -plaintext localhost:50051 list` muestra `ank.v1.KernelService`
- [ ] El setup_token se imprime en logs si no hay admin configurado
- [ ] Con `AEGIS_MTLS_STRICT=false` arranca sin certificados TLS
- [ ] `cargo clippy -p ank-server -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-ANK/crates/ank-server/src/main.rs` — lógica de arranque a portar y extender
`Aegis-ANK/crates/ank-server/src/server.rs` — AnkRpcServer a portar
