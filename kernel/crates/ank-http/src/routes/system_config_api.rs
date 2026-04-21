use crate::{
    citadel::{hash_passphrase, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tls/status", get(get_tls_status))
        .route("/tls/generate", post(generate_tls))
        .route("/restart", post(restart_service))
}

#[derive(Serialize, Deserialize)]
pub struct TlsStatusResponse {
    pub tls_enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub cert_exists: bool,
}

#[derive(Serialize, Deserialize)]
pub struct GenerateTlsResponse {
    pub success: bool,
    pub restart_required: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct RestartResponse {
    pub success: bool,
    pub message: String,
}

async fn require_master_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, AegisHttpError> {
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingTenant))?;

    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingKey))?;

    let hash = hash_passphrase(&raw_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&tenant_id, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    Ok(tenant_id)
}

pub async fn get_tls_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TlsStatusResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;

    let tls_enabled = match citadel.enclave.get_config("tls_enabled").await {
        Ok(Some(v)) => v == "true",
        _ => false,
    };

    let cert_path = match citadel.enclave.get_config("tls_cert_path").await {
        Ok(Some(p)) => Some(p),
        _ => None,
    };

    let key_path = match citadel.enclave.get_config("tls_key_path").await {
        Ok(Some(p)) => Some(p),
        _ => None,
    };

    let cert_exists = if let Some(ref p) = cert_path {
        std::path::Path::new(p).exists()
    } else {
        false
    };

    Ok(Json(TlsStatusResponse {
        tls_enabled,
        cert_path,
        key_path,
        cert_exists,
    }))
}

pub async fn generate_tls(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<GenerateTlsResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let cert_path = "/etc/aegis/cert.pem";
    let key_path = "/etc/aegis/key.pem";
    let ip = addr.ip().to_string();

    let status = std::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:4096",
            "-keyout",
            key_path,
            "-out",
            cert_path,
            "-days",
            "365",
            "-nodes",
            "-subj",
            "/CN=aegis-local",
            "-addext",
            &format!("subjectAltName=IP:{},IP:127.0.0.1,DNS:localhost", ip),
        ])
        .status()
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    if !status.success() {
        return Err(AegisHttpError::Internal(anyhow::anyhow!("openssl failed")));
    }

    let _ = std::process::Command::new("chmod")
        .args(["640", cert_path, key_path])
        .status();

    let citadel = state.citadel.lock().await;
    citadel
        .enclave
        .set_config("tls_enabled", "true")
        .await
        .map_err(AegisHttpError::Internal)?;
    citadel
        .enclave
        .set_config("tls_cert_path", cert_path)
        .await
        .map_err(AegisHttpError::Internal)?;
    citadel
        .enclave
        .set_config("tls_key_path", key_path)
        .await
        .map_err(AegisHttpError::Internal)?;

    Ok(Json(GenerateTlsResponse {
        success: true,
        restart_required: true,
        message: format!(
            "Certificado generado para IP {}. Reiniciá Aegis para activar HTTPS.",
            ip
        ),
    }))
}

pub async fn restart_service(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RestartResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let mode = std::fs::read_to_string("/etc/aegis/mode")
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    let result = if mode.trim() == "docker" {
        std::process::Command::new("docker")
            .args([
                "compose",
                "-f",
                "/opt/aegis/docker-compose.yml",
                "restart",
                "ank-server",
            ])
            .output()
    } else {
        std::process::Command::new("systemctl")
            .args(["restart", "aegis"])
            .output()
    };

    match result {
        Ok(output) if output.status.success() => Ok(Json(RestartResponse {
            success: true,
            message: "Servicio reiniciado correctamente".to_string(),
        })),
        Ok(output) => Err(AegisHttpError::Internal(anyhow::anyhow!(
            "Error al reiniciar: {}",
            String::from_utf8_lossy(&output.stderr)
        ))),
        Err(e) => Err(AegisHttpError::Internal(anyhow::anyhow!(e))),
    }
}
