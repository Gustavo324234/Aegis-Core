use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_system_status))
        .route("/health", get(health_check))
}

pub fn system_router() -> Router<AppState> {
    Router::new()
        .route("/state", get(get_public_system_state))
        .route("/sync_version", get(get_sync_version))
        .route("/hw_profile", post(crate::routes::engine::set_hw_profile))
}

#[derive(Deserialize)]
pub struct StatusQuery {
    pub tenant_id: String,
}

pub async fn get_system_status(
    State(state): State<AppState>,
    Query(query): Query<StatusQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, AegisHttpError> {
    let x_citadel_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AegisHttpError::Citadel(crate::citadel::CitadelError::MissingKey))?;

    let hash = hash_passphrase(x_citadel_key);

    // Validar contra Citadel
    {
        let citadel = state.citadel.lock().await;
        let is_auth = citadel
            .enclave
            .authenticate_tenant(&query.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;

        if !is_auth {
            return Err(AegisHttpError::Citadel(
                crate::citadel::CitadelError::Unauthorized,
            ));
        }
    }

    // Obtener hardware status del HAL
    let hw_info = {
        let hal = state.hal.write().await;
        let mut monitor = hal
            .hardware
            .lock()
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e.to_string())))?;
        let status = monitor.get_status();
        json!({
            "cpu_load": status.cpu_usage,
            "vram_allocated_mb": status.used_mem_mb,
            "vram_total_mb": status.total_mem_mb,
        })
    };

    let hw_profile = std::env::var("HW_PROFILE").unwrap_or_else(|_| "1".to_string());
    let hw_profile_name = match hw_profile.as_str() {
        "1" => "cloud",
        "2" => "local",
        "3" => "hybrid",
        _ => "cloud",
    };

    let mut res = hw_info;
    if let Some(obj) = res.as_object_mut() {
        obj.insert("hw_profile".to_string(), json!(hw_profile_name));
        obj.insert("state".to_string(), json!("STATE_OPERATIONAL"));
    }

    Ok(Json(res))
}

pub async fn get_public_system_state(State(state): State<AppState>) -> Json<Value> {
    let citadel = state.citadel.lock().await;
    let exists = citadel.enclave.admin_exists().await.unwrap_or(false);

    let state_str = if exists {
        "STATE_OPERATIONAL"
    } else {
        "STATE_INITIALIZING"
    };

    Json(json!({ "state": state_str }))
}

pub async fn get_sync_version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "Aegis HTTP Online" }))
}
