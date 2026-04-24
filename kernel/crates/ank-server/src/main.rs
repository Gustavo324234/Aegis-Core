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
    // CORE-147: Initialize rustls crypto provider (required for rustls 0.23+)
    // We explicitly use 'ring' as the provider.
    let _ = rustls::crypto::ring::default_provider().install_default();

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

    // 9. Scribe (Git-backed filesystem)
    let scribe = Arc::new(ank_core::scribe::ScribeManager::new(
        data_dir.to_str().unwrap_or("."),
    ));

    // 10. HAL
    let hal = Arc::new(CognitiveHAL::new(Arc::clone(&plugin_manager))?);

    // 10. Router & Catalog — filtrado por AEGIS_MODEL_PROFILE
    let model_profile = ModelProfile::from_env();
    info!("Model profile: {:?}", model_profile);

    let catalog = Arc::new(
        ank_core::router::catalog::ModelCatalog::load_bundled_with_profile(model_profile)?,
    );
    let key_pool = Arc::new(ank_core::router::key_pool::KeyPool::new(
        Arc::clone(&persistence) as Arc<dyn StatePersistor>,
    ));
    let _ = key_pool.load().await;

    let router = Arc::new(RwLock::new(CognitiveRouter::new(
        catalog.clone(),
        key_pool.clone(),
    )));
    hal.set_router(router.clone()).await;

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
        let hal_runner = hal.clone();
        let event_broker_runner = Arc::clone(&event_broker);
        let scheduler_tx_runner = scheduler_tx.clone();
        let telemetry_runner = telemetry.clone();
        let scribe_runner = Arc::clone(&scribe);
        let scheduler_tx_watchdog = scheduler_tx.clone();

        tokio::spawn(async move {
            while let Some(pcb) = execution_rx.recv().await {
                let hal_runner = hal_runner.clone();
                let event_broker_runner = Arc::clone(&event_broker_runner);
                let scheduler_tx_runner = scheduler_tx_runner.clone();
                let telemetry_runner = telemetry_runner.clone();
                let scribe_runner = Arc::clone(&scribe_runner);

                tokio::spawn(async move {
                    let pid = pcb.pid.clone();
                    let shared_pcb = Arc::new(RwLock::new(*pcb));

                    let event_tx = {
                        let mut broker = event_broker_runner.write().await;
                        broker
                            .entry(pid.clone())
                            .or_insert_with(|| {
                                let (tx, _) = tokio::sync::broadcast::channel(512);
                                tx
                            })
                            .clone()
                    };

                    let started_at = std::time::Instant::now();
                    let (tenant_id, session_key) = {
                        let p = shared_pcb.read().await;
                        (
                            p.tenant_id.clone().unwrap_or_default(),
                            p.session_key.clone().unwrap_or_default(),
                        )
                    };

                    let persona = if !tenant_id.is_empty() && !session_key.is_empty() {
                        ank_core::enclave::TenantDB::open(&tenant_id, &session_key)
                            .ok()
                            .and_then(|db| db.get_persona().unwrap_or(None))
                    } else {
                        None
                    };

                    let mcp_registry = hal_runner.mcp_registry.clone();
                    let http_client = hal_runner.http_client.clone();
                    let vcm = Arc::new(hal_runner.vcm);
                    let swap = hal_runner.swap_manager.clone();
                    let plugin_manager = hal_runner.plugin_manager.clone();

                    let executor = ank_core::syscalls::SyscallExecutor::new(
                        plugin_manager,
                        vcm,
                        scribe_runner,
                        swap,
                        mcp_registry,
                        http_client,
                        scheduler_tx_runner.clone(),
                    );

                    match hal_runner
                        .route_and_execute(Arc::clone(&shared_pcb), persona)
                        .await
                    {
                        Ok(stream) => {
                            use ank_core::syscalls::StreamInterceptor;
                            let mut interceptor = StreamInterceptor::new(stream);
                            let mut tokens_emitted = 0;
                            let mut full_output = String::new();

                            while let Some(item) = interceptor.next_item().await {
                                match item {
                                    ank_core::syscalls::StreamItem::Token(token) => {
                                        tokens_emitted += 1;
                                        full_output.push_str(&token);
                                    }
                                    ank_core::syscalls::StreamItem::Syscall(syscall) => {
                                        let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                            pid: pid.clone(),
                                            timestamp: None,
                                            payload: Some(
                                                ank_proto::v1::task_event::Payload::Syscall(
                                                    ank_proto::v1::Syscall {
                                                        name: format!("{:?}", syscall),
                                                        ..Default::default()
                                                    },
                                                ),
                                            ),
                                        });

                                        let pcb_snapshot = shared_pcb.read().await.clone();
                                        match executor.execute(&pcb_snapshot, syscall).await {
                                            Ok(result) => {
                                                info!(pid = %pid, "Syscall executed successfully.");
                                                // Inject result back into PCB context for next turns if needed,
                                                // or just send to UI.
                                                let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                                    pid: pid.clone(),
                                                    timestamp: None,
                                                    payload: Some(
                                                        ank_proto::v1::task_event::Payload::Output(
                                                            format!("\n{}", result),
                                                        ),
                                                    ),
                                                });
                                                full_output.push_str(&result);
                                            }
                                            Err(e) => {
                                                error!(pid = %pid, "Syscall failed: {}", e);
                                                let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                                    pid: pid.clone(),
                                                    timestamp: None,
                                                    payload: Some(
                                                        ank_proto::v1::task_event::Payload::Error(
                                                            e.to_string(),
                                                        ),
                                                    ),
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            // Telemetry & Finalization
                            let duration_ms = started_at.elapsed().as_millis() as u64;
                            telemetry_runner
                                .add_inference(CompletedInference {
                                    tokens_per_second: if duration_ms > 0 {
                                        tokens_emitted as f64 / (duration_ms as f64 / 1000.0)
                                    } else {
                                        0.0
                                    },
                                    tokens_emitted,
                                    model_id: "routed-model".to_string(),
                                    duration_ms,
                                    cost_usd: None,
                                })
                                .await;

                            let _ = scheduler_tx_runner
                                .send(SchedulerEvent::ProcessCompleted {
                                    pid: pid.clone(),
                                    output: full_output,
                                })
                                .await;

                            let _ = event_tx.send(ank_proto::v1::TaskEvent {
                                pid: pid.clone(),
                                timestamp: None,
                                payload: Some(ank_proto::v1::task_event::Payload::StatusUpdate(
                                    Box::new(ank_proto::v1::Pcb {
                                        state: 4,
                                        ..Default::default()
                                    }),
                                )),
                            });
                        }
                        Err(e) => {
                            error!(pid = %pid, "HAL Runner failed: {}", e);
                            let _ = scheduler_tx_runner
                                .send(SchedulerEvent::ProcessCompleted {
                                    pid,
                                    output: format!("error: {}", e),
                                })
                                .await;
                        }
                    }
                });
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
        hal: hal.clone(),
        scribe: Arc::clone(&scribe),
        router: Arc::clone(&router),
        siren_router: Arc::clone(&siren_router),
        catalog_syncer: Some(catalog_syncer),
        persistence: Arc::clone(&persistence) as Arc<dyn StatePersistor>,
        config,
        auth_rate_limiter,
        telemetry,
        tunnel_url: Arc::new(RwLock::new(None)),
    };

    // 12. Tonic Server
    let ank_rpc = AnkRpcServer::from_state(&state);
    let tonic_svc = KernelServiceServer::with_interceptor(ank_rpc, auth_interceptor);

    let grpc_addr = "0.0.0.0:50051".parse()?;
    let mut tonic_builder = Server::builder();

    warn!("gRPC running in INSECURE mode (h2c)");

    tokio::spawn(async move {
        if let Err(e) = tonic_builder.add_service(tonic_svc).serve(grpc_addr).await {
            error!("gRPC server failed: {}", e);
        }
    });

    // CORE-147: Log informative message for HTTP + Tunnel
    info!("🌐 Aegis serving HTTP on port 8000");
    info!("   For HTTPS access: cloudflared tunnel --url http://localhost:8000");
    info!("   Or run: sudo aegis tunnel");

    // 13. Axum Server
    info!("Starting Axum on port 8000");

    // CORE-146: Cloudflare Tunnel Manager
    {
        let tunnel_url_state = Arc::clone(&state.tunnel_url);
        tokio::spawn(async move {
            loop {
                info!("Tunnel Manager: Starting cloudflared...");
                match run_tunnel_and_monitor(8000, Arc::clone(&tunnel_url_state)).await {
                    Ok(_) => {
                        warn!("Tunnel Manager: cloudflared exited normally.");
                    }
                    Err(e) => {
                        warn!("Tunnel Manager: cloudflared error: {}", e);
                    }
                }

                {
                    let mut lock = tunnel_url_state.write().await;
                    *lock = None;
                }

                info!("Tunnel Manager: Restarting in 10 seconds...");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
    }

    let http_server = AegisHttpServer::new(state);
    http_server.serve().await?;

    Ok(())
}

async fn run_tunnel_and_monitor(
    port: u16,
    tunnel_url_state: Arc<RwLock<Option<String>>>,
) -> Result<()> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    // CORE-147: Support both Linux and Windows binary paths
    let bin = if cfg!(windows) {
        "cloudflared.exe"
    } else if std::path::Path::new("/usr/bin/cloudflared").exists() {
        "/usr/bin/cloudflared"
    } else if std::path::Path::new("/usr/local/bin/cloudflared").exists() {
        "/usr/local/bin/cloudflared"
    } else {
        "cloudflared"
    };

    let mut child = Command::new(bin)
        .args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn {}: {}. Is it installed?", bin, e))?;

    let stderr = child
        .stderr
        .take()
        .context("Failed to capture cloudflared stderr")?;

    let mut lines = BufReader::new(stderr).lines();
    let mut url_found = false;

    // Monitor stderr for the URL and keeps reading to prevent buffer fill
    while let Ok(Some(line)) = lines.next_line().await {
        if !url_found {
            if let Some(url) = extract_tunnel_url(&line) {
                info!("Cloudflare tunnel active: {}", url);
                {
                    let mut lock = tunnel_url_state.write().await;
                    *lock = Some(url);
                }
                url_found = true;
            }
        }
        // Log tunnel output for debugging (optional, keep it quiet for now)
        // tracing::debug!("cloudflared: {}", line);
    }

    let status = child.wait().await?;
    if !status.success() {
        anyhow::bail!("cloudflared exited with status: {}", status);
    }

    Ok(())
}

fn extract_tunnel_url(line: &str) -> Option<String> {
    if line.contains("trycloudflare.com") {
        line.split_whitespace()
            .find(|s| s.starts_with("https://") && s.contains("trycloudflare.com"))
            .map(|s| s.to_string())
    } else {
        None
    }
}
