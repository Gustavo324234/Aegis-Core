use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
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
    pub tenant_id: String,
    pub session_key: String,
    pub api_url: String,
    pub model_name: String,
    pub api_key: String,
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_provider() -> String {
    "custom".to_string()
}

pub async fn get_status(State(_state): State<AppState>) -> Result<Json<Value>, AegisHttpError> {
    // Intentar leer de engine_config.json en el DATA_DIR o relativo
    let config_path = "engine_config.json";
    if let Ok(content) = fs::read_to_string(config_path) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            return Ok(Json(val));
        }
    }

    Ok(Json(json!({ "configured": false })))
}

pub async fn configure(
    State(state): State<AppState>,
    Json(body): Json<EngineConfig>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.session_key);

    // 1. Validar contra Citadel
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .authenticate_tenant(&body.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;
    }

    // 2. Actualizar HAL
    {
        let mut hal = state.hal.write().await;
        hal.update_cloud_credentials(
            body.api_url.clone(),
            body.model_name.clone(),
            body.api_key.clone(),
        );
    }

    // 3. Persistir config
    let config_to_save = json!({
        "configured": true,
        "provider": body.provider,
        "api_url": body.api_url,
        "model_name": body.model_name,
    });

    let config_json = serde_json::to_string_pretty(&config_to_save)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    fs::write("engine_config.json", config_json)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Cognitive Engine dynamically configured."
    })))
}

#[derive(Deserialize)]
pub struct HwProfileRequest {
    pub admin_tenant_id: String,
    pub profile: String,
}

pub async fn set_hw_profile(
    State(_state): State<AppState>,
    Json(body): Json<HwProfileRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    if body.admin_tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can change HW profiles.".to_string(),
        ));
    }

    std::env::set_var("HW_PROFILE", &body.profile);

    Ok(Json(json!({
        "success": true,
        "profile": body.profile
    })))
}
