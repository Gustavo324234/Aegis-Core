use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::scheduler::persistence::VoiceProfile;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_siren_config))
        .route("/config", post(set_siren_config))
        .route("/voices", get(list_siren_voices))
}

#[derive(Deserialize)]
pub struct SirenConfigBody {
    pub provider: String,
    pub api_key: String,
    pub voice_id: String,
}

async fn get_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let stt_model_path = state.config.data_dir.join("models").join("ggml-base.bin");
    let stt_available = stt_model_path.exists();

    let profile = state
        .persistence
        .get_voice_profile(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    match profile {
        Some(p) => Ok(Json(json!({
            "provider": p.engine_id,
            "voice_id": p.voice_id,
            "configured": true,
            "settings": p.settings_json,
            "stt_available": stt_available
        }))),
        None => Ok(Json(json!({
            "provider": "mock",
            "voice_id": "",
            "configured": false,
            "stt_available": stt_available
        }))),
    }
}

async fn set_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<SirenConfigBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let existing = state
        .persistence
        .get_voice_profile(&auth.tenant_id)
        .await
        .unwrap_or(None);

    let profile = VoiceProfile {
        tenant_id: auth.tenant_id.clone(),
        engine_id: req.provider,
        voice_id: req.voice_id,
        model_pref: existing
            .map(|p| p.model_pref)
            .unwrap_or_else(|| "HybridSmart".to_string()),
        settings_json: json!({ "api_key": req.api_key }).to_string(),
    };

    state
        .persistence
        .update_voice_profile(profile)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Siren config updated successfully."
    })))
}

async fn list_siren_voices() -> Json<Value> {
    Json(json!({
        "voices": [
            { "id": "aura-asteria-en", "name": "Asteria (EN)", "provider": "voxtral" },
            { "id": "aura-luna-en", "name": "Luna (EN)", "provider": "voxtral" },
            { "id": "mock-voice", "name": "Mock Voice", "provider": "mock" }
        ]
    }))
}
