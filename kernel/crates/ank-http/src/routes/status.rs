use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
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

pub async fn get_system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AegisHttpError> {
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
        obj.insert("total_processes".to_string(), json!(0));
        obj.insert("active_workers".to_string(), json!(0));
    }

    Ok(Json(res))
}

pub async fn get_public_system_state(State(state): State<AppState>) -> Json<Value> {
    let citadel = state.citadel.lock().await;
    let exists = citadel.enclave.admin_exists().await.unwrap_or(false);
    Json(json!({ "state": if exists { "STATE_OPERATIONAL" } else { "STATE_INITIALIZING" } }))
}

pub async fn get_sync_version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "Online" }))
}
