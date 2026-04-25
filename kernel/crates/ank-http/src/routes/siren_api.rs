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
    #[serde(default)]
    pub stt_provider: String,
    #[serde(default)]
    pub stt_api_key: String,
}

async fn get_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let models_dir = state.config.data_dir.join("models");
    let active_model_path = models_dir.join("active_model.txt");
    let (stt_available, active_model) =
        if let Ok(name) = std::fs::read_to_string(&active_model_path) {
            let name = name.trim().to_string();
            let model_file = models_dir.join(format!("ggml-{}.bin", name));
            (model_file.exists(), Some(name))
        } else {
            // backwards-compat: accept ggml-base.bin without active_model.txt
            let legacy = models_dir.join("ggml-base.bin");
            if legacy.exists() {
                (true, Some("base".to_string()))
            } else {
                (false, None)
            }
        };

    let profile = state
        .persistence
        .get_voice_profile(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    match profile {
        Some(p) => {
            let settings = serde_json::from_str::<serde_json::Value>(&p.settings_json).ok();
            let api_key = settings
                .as_ref()
                .and_then(|v| v["api_key"].as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            let stt_provider = settings
                .as_ref()
                .and_then(|v| v["stt_provider"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "browser".to_string());
            let stt_api_key = settings
                .as_ref()
                .and_then(|v| v["stt_api_key"].as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            Ok(Json(json!({
                "provider": p.engine_id,
                "voice_id": p.voice_id,
                "api_key": api_key,
                "stt_provider": stt_provider,
                "stt_api_key": stt_api_key,
                "configured": true,
                "stt_available": stt_available,
                "active_model": active_model
            })))
        }
        None => Ok(Json(json!({
            "provider": "mock",
            "voice_id": "",
            "api_key": "",
            "stt_provider": "browser",
            "stt_api_key": "",
            "configured": false,
            "stt_available": stt_available,
            "active_model": active_model
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
        settings_json: json!({
            "api_key": req.api_key,
            "stt_provider": req.stt_provider,
            "stt_api_key": req.stt_api_key
        })
        .to_string(),
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
            { "id": "mock-voice", "name": "Mock Voice", "provider": "mock" },
            { "id": "21m00Tcm4TlvDq8ikWAM", "name": "Rachel (EN)", "provider": "elevenlabs" },
            { "id": "AZnzlk1XvdvUeBnXmlld", "name": "Domi (EN)", "provider": "elevenlabs" },
            { "id": "EXAVITQu4vr4xnSDxMaL", "name": "Bella (EN)", "provider": "elevenlabs" },
            { "id": "ErXwobaYiN019PkySvjV", "name": "Antoni (EN)", "provider": "elevenlabs" },
            { "id": "MF3mGyEYCl7XYWbV9V6O", "name": "Elli (EN)", "provider": "elevenlabs" },
            { "id": "TxGEqnHWrfWFTfGW9XjX", "name": "Josh (EN)", "provider": "elevenlabs" },
            { "id": "pNInz6obpgDQGcFmaJgB", "name": "Adam (EN)", "provider": "elevenlabs" }
        ]
    }))
}
