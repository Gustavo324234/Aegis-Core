use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use ank_core::scheduler::persistence::VoiceProfile;
use axum::{
    extract::{Query, State},
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
pub struct SirenQuery {
    pub tenant_id: String,
    pub session_key: String,
}

#[derive(Deserialize)]
pub struct SirenConfigRequest {
    pub tenant_id: String,
    pub session_key: String,
    pub provider: String,
    pub api_key: String,
    pub voice_id: String,
}

async fn get_siren_config(
    State(state): State<AppState>,
    Query(query): Query<SirenQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&query.session_key);
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .authenticate_tenant(&query.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;
    }

    let profile = state
        .persistence
        .get_voice_profile(&query.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    match profile {
        Some(p) => Ok(Json(json!({
            "provider": p.engine_id,
            "voice_id": p.voice_id,
            "configured": true,
            "settings": p.settings_json
        }))),
        None => Ok(Json(json!({
            "provider": "mock",
            "voice_id": "",
            "configured": false
        }))),
    }
}

async fn set_siren_config(
    State(state): State<AppState>,
    Json(req): Json<SirenConfigRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&req.session_key);
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .authenticate_tenant(&req.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;
    }

    let profile = VoiceProfile {
        tenant_id: req.tenant_id,
        engine_id: req.provider,
        voice_id: req.voice_id,
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

async fn list_siren_voices(
    State(_state): State<AppState>,
    Query(_query): Query<SirenQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    // For now, return a list reflecting supported engines
    Ok(Json(json!({
        "voices": [
            { "id": "aura-asteria-en", "name": "Asteria (EN)", "provider": "voxtral" },
            { "id": "aura-luna-en", "name": "Luna (EN)", "provider": "voxtral" },
            { "id": "mock-voice", "name": "Mock Voice", "provider": "mock" }
        ]
    })))
}
