use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/configure", post(configure))
}

#[derive(Deserialize)]
pub struct EngineConfig {
    pub api_url: String,
    pub model_name: String,
    pub api_key: String,
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_provider() -> String {
    "custom".to_string()
}

pub async fn get_status(State(state): State<AppState>) -> Result<Json<Value>, AegisHttpError> {
    let config_path = state.config.data_dir.join("engine_config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            return Ok(Json(val));
        }
    }
    Ok(Json(json!({ "configured": false })))
}

pub async fn configure(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<EngineConfig>,
) -> Result<Json<Value>, AegisHttpError> {
    // Auth ya validada por CitadelAuthenticated extractor (headers x-citadel-tenant / x-citadel-key)

    // Actualizar HAL en memoria
    {
        let mut hal = state.hal.write().await;
        hal.update_cloud_credentials(
            body.api_url.clone(),
            body.model_name.clone(),
            body.api_key.clone(),
        );
    }

    // Persistir config en data_dir (CORE-075)
    let config_to_save = json!({
        "configured": true,
        "provider": body.provider,
        "api_url": body.api_url,
        "model_name": body.model_name,
        "configured_by": auth.tenant_id,
    });

    let config_json = serde_json::to_string_pretty(&config_to_save)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    let config_path = state.config.data_dir.join("engine_config.json");
    fs::write(&config_path, config_json)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Cognitive Engine dynamically configured."
    })))
}

#[derive(Deserialize)]
pub struct HwProfileRequest {
    pub admin_tenant_id: String,
    pub session_key: String,
    pub profile: String,
}

pub async fn set_hw_profile(
    State(state): State<AppState>,
    Json(body): Json<HwProfileRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = crate::citadel::hash_passphrase(&body.session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    if !["1", "2", "3"].contains(&body.profile.as_str()) {
        return Err(AegisHttpError::BadRequest(
            "Invalid profile. Use 1, 2 or 3.".into(),
        ));
    }

    std::env::set_var("HW_PROFILE", &body.profile);

    Ok(Json(json!({
        "success": true,
        "profile": body.profile
    })))
}
