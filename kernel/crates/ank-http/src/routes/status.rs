use crate::{
    citadel::{hash_passphrase, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::SchedulerEvent;
use axum::{
    extract::{ConnectInfo, FromRequestParts, State},
    http::{request::Parts, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use std::convert::Infallible;
use std::net::SocketAddr;
use utoipa::ToSchema;

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static CLOUDFLARED_INSTALLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

pub struct OptionalPeerAddr(pub Option<SocketAddr>);

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for OptionalPeerAddr {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let addr = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0);
        Ok(OptionalPeerAddr(addr))
    }
}

#[derive(serde::Serialize, ToSchema)]
pub struct SystemStatusResponse {
    pub cpu_load: f32,
    pub vram_allocated_mb: u64,
    pub vram_total_mb: u64,
    pub hw_profile: String,
    pub state: String,
    pub total_processes: u32,
    pub active_workers: u32,
    pub tokens_per_second: f64,
    pub total_tokens_session: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ConnectionInfoResponse {
    pub local_url: String,
    pub tunnel_url: Option<String>,
    pub tunnel_status: String,
    pub qr_url: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct PublicSystemStateResponse {
    pub state: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_system_status))
        .route("/health", get(health_check))
}

pub fn system_router() -> Router<AppState> {
    Router::new()
        .route("/state", get(get_public_system_state))
        .route("/sync_version", get(get_sync_version))
        .route("/connection-info", get(get_connection_info))
        .route("/hw_profile", post(crate::routes::engine::set_hw_profile))
        .route("/service/status", get(get_service_status))
        .route("/service/restart", post(service_restart))
        .route("/service/stop", post(service_stop))
        .route("/service/logs", get(get_service_logs))
}

#[derive(serde::Serialize, ToSchema)]
pub struct ServiceStatusResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub pid: u32,
}

async fn require_service_master_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), AegisHttpError> {
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingTenant))?;

    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingKey))?;

    let hash = hash_passphrase(raw_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(tenant_id, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/system/service/status",
    tag = "status",
    responses(
        (status = 200, description = "Service status", body = ServiceStatusResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin key (plaintext)")
    )
)]
pub async fn get_service_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ServiceStatusResponse>, AegisHttpError> {
    require_service_master_auth(&state, &headers).await?;

    let uptime_secs = PROCESS_START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs();

    Ok(Json(ServiceStatusResponse {
        status: "running".to_string(),
        uptime_secs,
        pid: std::process::id(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/system/service/restart",
    tag = "status",
    responses(
        (status = 202, description = "Restart scheduled"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin key (plaintext)")
    )
)]
pub async fn service_restart(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AegisHttpError> {
    require_service_master_auth(&state, &headers).await?;

    tracing::warn!(
        "CORE-256: service restart requested by master admin — exiting for process manager restart"
    );
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        std::process::exit(0);
    });

    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path = "/api/system/service/stop",
    tag = "status",
    responses(
        (status = 202, description = "Stop scheduled"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin key (plaintext)")
    )
)]
pub async fn service_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AegisHttpError> {
    require_service_master_auth(&state, &headers).await?;

    tracing::warn!("CORE-256: service stop requested by master admin — exiting");
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        std::process::exit(0);
    });

    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    get,
    path = "/api/status",
    tag = "status",
    responses(
        (status = 200, description = "System status", body = SystemStatusResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
pub async fn get_system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SystemStatusResponse>, AegisHttpError> {
    // Auth desde headers Citadel — consistente con todos los demás endpoints
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(
            crate::citadel::CitadelError::MissingTenant,
        ))?;

    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(
            crate::citadel::CitadelError::MissingKey,
        ))?;

    let hash = hash_passphrase(raw_key);

    {
        let citadel = state.citadel.lock().await;

        // El admin (master) también puede consultar el status
        let is_master = citadel
            .enclave
            .authenticate_master(tenant_id, &hash)
            .await
            .unwrap_or(false);

        if !is_master {
            let is_tenant = citadel
                .enclave
                .authenticate_tenant(tenant_id, &hash)
                .await
                .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;

            if !is_tenant {
                return Err(AegisHttpError::Citadel(
                    crate::citadel::CitadelError::Unauthorized,
                ));
            }
        }
    }

    let hw_profile = std::env::var("HW_PROFILE").unwrap_or_else(|_| "1".to_string());
    let hw_profile_name = match hw_profile.as_str() {
        "1" => "cloud",
        "2" => "local",
        "3" => "hybrid",
        _ => "cloud",
    };

    let (cpu_load, vram_allocated_mb, vram_total_mb) = {
        let mut monitor = state.hal.hardware.lock().await;
        let status = monitor.get_status();
        (status.cpu_usage, status.used_mem_mb, status.total_mem_mb)
    };

    let metrics = state.telemetry.metrics().await;

    // CORE-154: Query live scheduler stats
    let (total_processes, active_workers) = {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if state
            .scheduler_tx
            .send(SchedulerEvent::GetStats(tx))
            .await
            .is_ok()
        {
            if let Ok(stats) = rx.await {
                (stats.total_processes, stats.active_workers)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    Ok(Json(SystemStatusResponse {
        cpu_load,
        vram_allocated_mb,
        vram_total_mb,
        hw_profile: hw_profile_name.to_string(),
        state: "STATE_OPERATIONAL".to_string(),
        total_processes,
        active_workers,
        tokens_per_second: metrics.tokens_per_second,
        total_tokens_session: metrics.total_tokens_session,
        estimated_cost_usd: metrics.estimated_cost_usd,
    }))
}

#[utoipa::path(
    get,
    path = "/api/system/state",
    tag = "status",
    responses(
        (status = 200, description = "Public system state", body = PublicSystemStateResponse)
    )
)]
pub async fn get_public_system_state(
    State(state): State<AppState>,
) -> Json<PublicSystemStateResponse> {
    let citadel = state.citadel.lock().await;
    let exists = citadel.enclave.admin_exists().await.unwrap_or(false);
    Json(PublicSystemStateResponse {
        state: if exists {
            "STATE_OPERATIONAL".to_string()
        } else {
            "STATE_INITIALIZING".to_string()
        },
    })
}

#[utoipa::path(
    get,
    path = "/api/system/sync_version",
    tag = "status",
    responses(
        (status = 200, description = "Sync version")
    )
)]
pub async fn get_sync_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }))
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "status",
    responses(
        (status = 200, description = "Health check", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "Online".to_string(),
    })
}

fn is_cloudflared_installed() -> bool {
    *CLOUDFLARED_INSTALLED.get_or_init(|| {
        let bin = if cfg!(windows) {
            "cloudflared.exe"
        } else {
            "cloudflared"
        };
        std::process::Command::new(bin)
            .arg("--version")
            .output()
            .is_ok()
    })
}

fn is_direct_local_cli_probe(peer_addr: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    let Some(addr) = peer_addr else {
        return false;
    };
    if !addr.ip().is_loopback() {
        return false;
    }

    // Reject requests forwarded through cloudflared, reverse proxies, or aegis-connect-relay
    const PROXY_HEADERS: [&str; 5] = [
        "cf-connecting-ip",
        "cf-ray",
        "x-forwarded-for",
        "x-real-ip",
        "forwarded",
    ];
    if PROXY_HEADERS.iter().any(|h| headers.contains_key(*h)) {
        return false;
    }

    let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let host_only = host
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.split(']').next())
        .unwrap_or_else(|| host.trim().split(':').next().unwrap_or(""));

    matches!(host_only, "localhost" | "127.0.0.1" | "::1")
}

pub async fn get_connection_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
) -> Result<Json<ConnectionInfoResponse>, AegisHttpError> {
    // CORE-368: Allow authenticated Citadel sessions from any origin, or direct non-proxied
    // loopback probes used by `aegis status` on the local host.
    let citadel_tenant = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok());
    let citadel_key = headers.get("x-citadel-key").and_then(|v| v.to_str().ok());

    let authorized = if let (Some(t), Some(k)) = (citadel_tenant, citadel_key) {
        crate::citadel::verify_ws_tenant(&state, t, k).await.is_ok()
    } else {
        is_direct_local_cli_probe(peer_addr, &headers)
    };

    if !authorized {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    let tunnel_url = state.tunnel_url.read().await.clone();

    // Determine tunnel status without spawning a subprocess per HTTP request (CORE-368)
    let tunnel_status = if tunnel_url.is_some() {
        "active".to_string()
    } else if is_cloudflared_installed() {
        "connecting".to_string()
    } else {
        "disabled".to_string()
    };

    // Get local IP
    let local_ip = get_local_ip().unwrap_or_else(|| {
        tracing::warn!("Failed to detect local IP, falling back to localhost");
        "localhost".to_string()
    });

    let protocol = if std::env::var("AEGIS_TLS_CERT").is_ok() {
        "https"
    } else {
        "http"
    };
    let local_url = format!("{}://{}:{}", protocol, local_ip, state.config.port);

    // CORE-146: If tunnel is active, it's the preferred URL for the app
    let qr_url = tunnel_url.clone().unwrap_or_else(|| local_url.clone());

    tracing::info!(
        local = %local_url,
        tunnel = ?tunnel_url,
        status = %tunnel_status,
        "System connection info requested"
    );

    Ok(Json(ConnectionInfoResponse {
        local_url,
        tunnel_url,
        tunnel_status,
        qr_url,
    }))
}

fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

#[derive(serde::Deserialize, ToSchema)]
pub struct LogsQuery {
    pub lines: Option<usize>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct LogsResponse {
    pub logs: String,
}

#[utoipa::path(
    get,
    path = "/api/system/service/logs",
    tag = "status",
    responses(
        (status = 200, description = "Service logs", body = LogsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal Server Error")
    ),
    params(
        ("lines" = Option<usize>, Query, description = "Number of lines to return"),
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin key (plaintext)")
    )
)]
pub async fn get_service_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> Result<Json<LogsResponse>, AegisHttpError> {
    require_service_master_auth(&state, &headers).await?;

    let lines_to_read = query.lines.unwrap_or(100);
    let logs_dir = state.config.data_dir.join("logs");

    if !logs_dir.exists() {
        return Ok(Json(LogsResponse {
            logs: "[System Logs] Logs directory does not exist yet.".to_string(),
        }));
    }

    let mut entries = std::fs::read_dir(&logs_dir)
        .map_err(|e| AegisHttpError::Kernel(format!("Failed to read logs directory: {}", e)))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("ank.log"))
        .collect::<Vec<_>>();

    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

    let latest = match entries.last() {
        Some(e) => e,
        None => {
            return Ok(Json(LogsResponse {
                logs: "[System Logs] No active log files found.".to_string(),
            }));
        }
    };

    let file = std::fs::File::open(latest.path())
        .map_err(|e| AegisHttpError::Kernel(format!("Failed to open log file: {}", e)))?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead;
    let all_lines = reader.lines().map_while(Result::ok).collect::<Vec<_>>();

    let start = all_lines.len().saturating_sub(lines_to_read);
    let selected_lines = &all_lines[start..];
    let logs_content = selected_lines.join("\n");

    Ok(Json(LogsResponse { logs: logs_content }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn direct_local_cli_probe_accepts_loopback_localhost() {
        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("localhost:8000"));
        let peer = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 51234));
        assert!(is_direct_local_cli_probe(peer, &headers));
    }

    #[test]
    fn direct_local_cli_probe_rejects_proxied_or_public_host() {
        let peer = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 51234));

        // Forwarded via cloudflared / reverse proxy
        let mut cf_headers = HeaderMap::new();
        cf_headers.insert("host", HeaderValue::from_static("localhost:8000"));
        cf_headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.42"));
        assert!(!is_direct_local_cli_probe(peer, &cf_headers));

        // Forwarded via aegis-connect-relay with public Host header
        let mut relay_headers = HeaderMap::new();
        relay_headers.insert("host", HeaderValue::from_static("node1.relay.aegis-os.com"));
        assert!(!is_direct_local_cli_probe(peer, &relay_headers));

        // Non-loopback LAN client without Citadel headers
        let lan_peer = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)),
            51234,
        ));
        let mut lan_headers = HeaderMap::new();
        lan_headers.insert("host", HeaderValue::from_static("192.168.1.10:8000"));
        assert!(!is_direct_local_cli_probe(lan_peer, &lan_headers));
    }
}
