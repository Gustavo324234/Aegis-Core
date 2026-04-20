use ank_core::plugins::watcher::watch_plugins_dir;
use ank_core::plugins::PluginManager;
use ank_core::router::catalog::ModelProfile;
use ank_core::telemetry::{CompletedInference, TelemetryState};
use ank_core::{
    citadel::identity::Citadel, enclave::master::MasterEnclave, router::CognitiveRouter,
    router::SirenRouter, CognitiveHAL, CognitiveScheduler, SQLCipherPersistor, SchedulerEvent,
    StatePersistor, PCB,
};
use ank_http::{
    rate_limiter::{AuthRateLimiter, RateLimitConfig},
    AegisHttpServer, AppState, HttpConfig,
};
use ank_proto::v1::kernel_service_server::KernelServiceServer;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tonic::transport::Server;
use tracing::{error, info, warn};

mod server;
use server::{auth_interceptor, AnkRpcServer};

fn resolve_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("AEGIS_DATA_DIR") {
        return std::path::PathBuf::from(dir);
    }
    let base = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let data_dir = base.join("aegis");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir
}

#[tokio::main]
async fn main() -> Result<()> {
    // 0. Handle immediate flags
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("Aegis Core v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // 1. Inicializar tracing
    let data_dir = resolve_data_dir();
    let logs_dir = data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir).ok();

    let file_appender = tracing_appender::rolling::daily(logs_dir, "ank.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(tracing_subscriber::fmt::writer::MakeWriterExt::and(
            std::io::stdout,
            non_blocking,
        ))
        .init();

    info!("Aegis Core — Unified Binary Starting...");

    // 2. Leer AEGIS_ROOT_KEY
    let root_key = std::env::var("AEGIS_ROOT_KEY")
        .context("FATAL: AEGIS_ROOT_KEY environment variable is missing.")?;

    // 3. Data Dir
    info!("ANK data directory: {}", data_dir.display());

    // 4. Persistence
    let scheduler_db_path = data_dir.join("scheduler_state.db");
    info!(
        "Persistence: Initializing SQLCipher at {}",
        scheduler_db_path.display()
    );
    let persistence = Arc::new(SQLCipherPersistor::new(
        scheduler_db_path.to_str().context("Invalid db path")?,
        &root_key,
    )?);

    // 5. Master Enclave
    let admin_db_path = data_dir.join("admin.db");
    info!(
        "Identity: Initializing Master Enclave at {}",
        admin_db_path.display()
    );
    let master_enclave = MasterEnclave::open(
        admin_db_path.to_str().context("Invalid admin db path")?,
        &root_key,
    )
    .await?;
    let citadel = Arc::new(Mutex::new(Citadel {
        enclave: master_enclave,
    }));

    // 6. Setup Token
    {
        let c = citadel.lock().await;
        if !c.enclave.admin_exists().await? {
            let token = uuid::Uuid::new_v4().to_string().replace("-", "");
            c.enclave.store_setup_token(&token, 30).await?;
            info!("╔══════════════════════════════════════════════════╗");
            info!("║         AEGIS OS — FIRST TIME SETUP              ║");
            info!("║  URL: http://localhost:8000?setup_token={} ║", token);
            info!("╚══════════════════════════════════════════════════╝");
        }
    }

    // 7. Scheduler
    let (scheduler_tx, scheduler_rx) = mpsc::channel(1024);
    let (execution_tx, mut execution_rx) = mpsc::channel::<Box<PCB>>(64);
    let mut scheduler =
        CognitiveScheduler::new(Arc::clone(&persistence) as Arc<dyn StatePersistor>);
    scheduler.execution_tx = Some(execution_tx);
    let scheduler_tx_clone = scheduler_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = scheduler.start(scheduler_rx, scheduler_tx_clone).await {
            error!("Scheduler crashed: {}", e);
        }
    });

    // 8. Plugin Manager
    let plugin_manager = Arc::new(RwLock::new(PluginManager::new()?));
    let pm_clone = Arc::clone(&plugin_manager);
    tokio::spawn(async move {
        let p_dir = resolve_data_dir().join("plugins");
        std::fs::create_dir_all(&p_dir).ok();
        if let Some(s) = p_dir.to_str() {
            let _ = watch_plugins_dir(s.to_string(), pm_clone).await;
        }
    });

    // 9. HAL
    let hal = Arc::new(RwLock::new(CognitiveHAL::new(Arc::clone(&plugin_manager))?));

    // 10. Router & Catalog — filtrado por AEGIS_MODEL_PROFILE
    let model_profile = ModelProfile::from_env();
    info!("Model profile: {:?}", model_profile);

    let catalog = Arc::new(
        ank_core::router::catalog::ModelCatalog::load_bundled_with_profile(model_profile)?
    );
    let key_pool = Arc::new(ank_core::router::key_pool::KeyPool::new(
        Arc::clone(&persistence) as Arc<dyn StatePersistor>,
    ));
    let _ = key_pool.load().await;

    let router = Arc::new(RwLock::new(CognitiveRouter::new(
        catalog.clone(),
        key_pool.clone(),
    )));
    hal.write().await.set_router(router.clone());

    let catalog_syncer = Arc::new(ank_core::router::syncer::CatalogSyncer::new(
        catalog, key_pool,
    ));
    catalog_syncer.clone().start_background_sync();

    let siren_router = Arc::new(SirenRouter::new(
        Arc::clone(&persistence) as Arc<dyn StatePersistor>
    ));

    // 11. AppState
    let event_broker = Arc::new(RwLock::new(HashMap::new()));
    let telemetry = TelemetryState::new();

    // 11.5. HAL Runner — Scheduler → CognitiveHAL → event_broker → WebSocket
    // CORE-092: Al cerrarse execution_rx, notifica al scheduler via HalRunnerDied
    // para que limpie current_running y evite un deadlock silencioso.
    {
        let hal_runner = Arc::clone(&hal);
        let event_broker_runner = Arc::clone(&event_broker);
        let scheduler_tx_runner = scheduler_tx.clone();
        let scheduler_tx_watchdog = scheduler_tx.clone();
        let telemetry_runner = telemetry.clone();

        tokio::spawn(async move {
            while let Some(pcb) = execution_rx.recv().await {
                let pid = pcb.pid.clone();
                let shared_pcb = Arc::new(RwLock::new(*pcb));

                let event_tx = {
                    let mut broker = event_broker_runner.write().await;
                    broker
                        .entry(pid.clone())
                        .or_insert_with(|| {
                            let (tx, _) = tokio::sync::broadcast::channel(256);
                            tx
                        })
                        .clone()
                };

                let started_at = std::time::Instant::now();
                let hal_read = hal_runner.read().await;

                // CORE-097: Extraer cancel_token del PCB para soportar preemption
                let cancel_token = {
                    let pcb_lock = shared_pcb.read().await;
                    pcb_lock.cancel_token.clone()
                };

                // Resolver model_id para telemetría (best-effort, no bloquea ejecución)
                let model_id = {
                    if let Some(ref router_rw) = hal_read.router {
                        let router = router_rw.read().await;
                        let pcb_snapshot = shared_pcb.read().await;
                        router
                            .decide(&pcb_snapshot)
                            .await
                            .ok()
                            .map(|d| d.model_id)
                            .unwrap_or_else(|| "unknown".to_string())
                    } else {
                        "unknown".to_string()
                    }
                };

                match hal_read.route_and_execute(Arc::clone(&shared_pcb)).await {
                    Ok(mut stream) => {
                        use tokio_stream::StreamExt as _;
                        let mut tokens_emitted: u32 = 0;

                        while let Some(token_result) = stream.next().await {
                            // CORE-097: Verificar cancelación en cada chunk
                            if cancel_token.is_cancelled() {
                                tracing::warn!(pid = %pid, "Process preempted via CancellationToken");
                                let cancel_event = ank_proto::v1::TaskEvent {
                                    pid: pid.clone(),
                                    timestamp: None,
                                    payload: Some(
                                        ank_proto::v1::task_event::Payload::StatusUpdate(Box::new(
                                            ank_proto::v1::Pcb {
                                                state: 5, // STATE_PREEMPTED
                                                ..Default::default()
                                            },
                                        )),
                                    ),
                                };
                                let _ = event_tx.send(cancel_event);
                                let _ = scheduler_tx_runner
                                    .send(SchedulerEvent::ProcessCompleted {
                                        pid: pid.clone(),
                                        output: "preempted".to_string(),
                                    })
                                    .await;
                                break;
                            }

                            match token_result {
                                Ok(token) => {
                                    tokens_emitted = tokens_emitted.saturating_add(1);
                                    let event = ank_proto::v1::TaskEvent {
                                        pid: pid.clone(),
                                        timestamp: None,
                                        payload: Some(ank_proto::v1::task_event::Payload::Output(
                                            token,
                                        )),
                                    };
                                    let _ = event_tx.send(event);
                                }
                                Err(e) => {
                                    let event = ank_proto::v1::TaskEvent {
                                        pid: pid.clone(),
                                        timestamp: None,
                                        payload: Some(ank_proto::v1::task_event::Payload::Error(
                                            e.to_string(),
                                        )),
                                    };
                                    let _ = event_tx.send(event);
                                    break;
                                }
                            }
                        }

                        // CORE-105: Registrar telemetría de la inferencia completada
                        let duration_ms = started_at.elapsed().as_millis() as u64;
                        let tokens_per_second = if duration_ms > 0 {
                            (tokens_emitted as f64 / (duration_ms as f64 / 1000.0)).min(1_000_000.0)
                        } else {
                            0.0
                        };

                        let inference = CompletedInference {
                            tokens_per_second,
                            tokens_emitted,
                            model_id: model_id.clone(),
                            duration_ms,
                            cost_usd: None,
                        };
                        telemetry_runner.add_inference(inference).await;

                        let _ = scheduler_tx_runner
                            .send(SchedulerEvent::ProcessCompleted {
                                pid: pid.clone(),
                                output: "stream_complete".to_string(),
                            })
                            .await;

                        let done_event = ank_proto::v1::TaskEvent {
                            pid: pid.clone(),
                            timestamp: None,
                            payload: Some(ank_proto::v1::task_event::Payload::StatusUpdate(
                                Box::new(ank_proto::v1::Pcb {
                                    state: 4, // STATE_COMPLETED
                                    ..Default::default()
                                }),
                            )),
                        };
                        let _ = event_tx.send(done_event);

                        tracing::debug!(
                            pid = %pid,
                            model = %model_id,
                            tokens = tokens_emitted,
                            tps = %format!("{:.2}", tokens_per_second),
                            "Inference completed"
                        );
                    }
                    Err(e) => {
                        tracing::error!(pid = %pid, "HAL execution failed: {}", e);
                        let event = ank_proto::v1::TaskEvent {
                            pid: pid.clone(),
                            timestamp: None,
                            payload: Some(ank_proto::v1::task_event::Payload::Error(e.to_string())),
                        };
                        let _ = event_tx.send(event);
                        let _ = scheduler_tx_runner
                            .send(SchedulerEvent::ProcessCompleted {
                                pid,
                                output: format!("error: {}", e),
                            })
                            .await;
                    }
                }
            }

            // CORE-092: El canal execution_rx se cerró — el Scheduler debe saberlo
            // para limpiar current_running y evitar que quede bloqueado indefinidamente.
            warn!("HAL Runner: execution_rx channel closed. Notifying scheduler.");
            let _ = scheduler_tx_watchdog
                .send(SchedulerEvent::HalRunnerDied {
                    reason: "execution_rx channel closed unexpectedly".to_string(),
                })
                .await;
        });
    }

    let mut config = HttpConfig::from_env();
    config.port = 8000;

    let auth_rate_limiter = AuthRateLimiter::new(RateLimitConfig::from_env());

    let state = AppState {
        scheduler_tx: scheduler_tx.clone(),
        event_broker: Arc::clone(&event_broker),
        citadel: Arc::clone(&citadel),
        hal: Arc::clone(&hal),
        router: Arc::clone(&router),
        siren_router: Arc::clone(&siren_router),
        catalog_syncer: Some(catalog_syncer),
        persistence: Arc::clone(&persistence) as Arc<dyn StatePersistor>,
        config,
        auth_rate_limiter,
        telemetry,
    };

    // 12. Tonic Server
    let ank_rpc = AnkRpcServer::from_state(&state);
    let tonic_svc = KernelServiceServer::with_interceptor(ank_rpc, auth_interceptor);

    let grpc_addr = "0.0.0.0:50051".parse()?;
    let mut tonic_builder = Server::builder();

    match (
        std::env::var("AEGIS_TLS_CERT"),
        std::env::var("AEGIS_TLS_KEY"),
    ) {
        (Ok(cert_p), Ok(key_p)) => {
            info!("TLS enabled for gRPC (Tonic)");
            let cert = tokio::fs::read(cert_p).await?;
            let key = tokio::fs::read(key_p).await?;
            let id = tonic::transport::Identity::from_pem(cert, key);
            tonic_builder =
                tonic_builder.tls_config(tonic::transport::ServerTlsConfig::new().identity(id))?;
        }
        _ => {
            let strict = std::env::var("AEGIS_MTLS_STRICT")
                .unwrap_or_default()
                .to_lowercase()
                == "true";
            if strict {
                anyhow::bail!("AEGIS_MTLS_STRICT=true but certificates are missing.");
            }
            warn!("gRPC running in INSECURE mode");
        }
    }

    tokio::spawn(async move {
        if let Err(e) = tonic_builder.add_service(tonic_svc).serve(grpc_addr).await {
            error!("gRPC server failed: {}", e);
        }
    });

    // 13. Axum Server
    info!("Starting Axum on port 8000");
    let http_server = AegisHttpServer::new(state);
    http_server.serve().await?;

    Ok(())
}
