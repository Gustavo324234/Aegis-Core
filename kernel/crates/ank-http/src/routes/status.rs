use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use utoipa::ToSchema;

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
        let hal = state.hal.read().await;
        let mut monitor = hal.hardware.lock().await;
        let status = monitor.get_status();
        (status.cpu_usage, status.used_mem_mb, status.total_mem_mb)
    };

    let metrics = state.telemetry.metrics().await;

    Ok(Json(SystemStatusResponse {
        cpu_load,
        vram_allocated_mb,
        vram_total_mb,
        hw_profile: hw_profile_name.to_string(),
        state: "STATE_OPERATIONAL".to_string(),
        total_processes: 0,
        active_workers: 0,
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

#[utoipa::path(
    get,
    path = "/api/system/connection-info",
    tag = "status",
    responses(
        (status = 200, description = "Connection information", body = ConnectionInfoResponse)
    )
)]
pub async fn get_connection_info(State(state): State<AppState>) -> Json<ConnectionInfoResponse> {
    let tunnel_url = state.tunnel_url.read().await.clone();

    // Determine tunnel status
    let tunnel_status = if tunnel_url.is_some() {
        "active".to_string()
    } else {
        // Best effort check if cloudflared is available
        let bin = if cfg!(windows) {
            "cloudflared.exe"
        } else {
            "cloudflared"
        };
        let exists = std::process::Command::new(bin)
            .arg("--version")
            .output()
            .is_ok();

        if exists {
            "connecting".to_string()
        } else {
            "disabled".to_string()
        }
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
    let local_url = format!("{}://{}:8000", protocol, local_ip);

    // CORE-146: If tunnel is active, it's the preferred URL for the app
    let qr_url = tunnel_url.clone().unwrap_or_else(|| local_url.clone());

    tracing::info!(
        local = %local_url,
        tunnel = ?tunnel_url,
        status = %tunnel_status,
        "System connection info requested"
    );

    Json(ConnectionInfoResponse {
        local_url,
        tunnel_url,
        tunnel_status,
        qr_url,
    })
}

fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}
