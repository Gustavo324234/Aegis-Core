use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use utoipa::ToSchema;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/configure", post(configure))
}

#[derive(Deserialize, ToSchema)]
pub struct EngineConfig {
    #[schema(example = "https://openrouter.ai/api/v1")]
    pub api_url: String,
    #[schema(example = "anthropic/claude-3-5-sonnet")]
    pub model_name: String,
    #[schema(format = "password")]
    pub api_key: String,
    #[serde(default = "default_provider")]
    #[schema(example = "openrouter")]
    pub provider: String,
}

fn default_provider() -> String {
    "custom".to_string()
}

#[derive(serde::Serialize, ToSchema)]
pub struct EngineStatusResponse {
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ConfigureResponse {
    pub success: bool,
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/api/engine/status",
    tag = "engine",
    responses(
        (status = 200, description = "Engine configuration status", body = EngineStatusResponse)
    )
)]
pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<EngineStatusResponse>, AegisHttpError> {
    // Primero verificar el archivo de configuración legacy
    let config_path = state.config.data_dir.join("engine_config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if val.get("configured").and_then(|v| v.as_bool()) == Some(true) {
                return Ok(Json(EngineStatusResponse {
                    configured: true,
                    provider: val
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    api_url: val
                        .get("api_url")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    model_name: val
                        .get("model_name")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                }));
            }
        }
    }

    // Si no hay engine_config.json, verificar si hay keys en el KeyPool global.
    // Si el admin configuró providers via IA Tools, el sistema está listo para los tenants.
    let has_global_keys = {
        let router = state.router.read().await;
        !router.list_global_keys().await.is_empty()
    };

    if has_global_keys {
        return Ok(Json(EngineStatusResponse {
            configured: true,
            provider: Some("router".to_string()),
            api_url: None,
            model_name: None,
        }));
    }

    Ok(Json(EngineStatusResponse {
        configured: false,
        provider: None,
        api_url: None,
        model_name: None,
    }))
}

#[utoipa::path(
    post,
    path = "/api/engine/configure",
    tag = "engine",
    request_body = EngineConfig,
    responses(
        (status = 200, description = "Engine configured", body = ConfigureResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
pub async fn configure(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<EngineConfig>,
) -> Result<Json<ConfigureResponse>, AegisHttpError> {
    state
        .hal
        .update_cloud_credentials(
            body.api_url.clone(),
            body.model_name.clone(),
            body.api_key.clone(),
        )
        .await;

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

    Ok(Json(ConfigureResponse {
        success: true,
        message: "Cognitive Engine dynamically configured.".to_string(),
    }))
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
