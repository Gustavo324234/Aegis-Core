use crate::citadel::CitadelAuthenticated;
use crate::error::AegisHttpError;
use ank_core::enclave::TenantDB;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct OAuthTokensBody {
    provider: String,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: u64,
    scope: String,
}

#[derive(Serialize)]
pub struct ProviderStatus {
    connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

#[derive(Serialize)]
pub struct OAuthStatusResponse {
    google: ProviderStatus,
    spotify: ProviderStatus,
}

pub async fn receive_tokens(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<OAuthTokensBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    if !["google", "spotify"].contains(&body.provider.as_str()) {
        return Err(AegisHttpError::BadRequest("Unknown provider".into()));
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(e))?;

    db.set_oauth_token(
        &body.provider,
        &body.access_token,
        body.refresh_token.as_deref(),
        body.expires_in,
        &body.scope,
    )
    .map_err(|e| AegisHttpError::Internal(e))?;

    tracing::info!(
        tenant = %auth.tenant_id,
        provider = %body.provider,
        "OAuth tokens stored"
    );

    Ok(Json(json!({ "success": true })))
}

pub async fn get_status(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<OAuthStatusResponse>, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(e))?;

    let google_connected = db
        .is_oauth_connected("google")
        .map_err(|e| AegisHttpError::Internal(e))?;
    let google_scope = db
        .get_oauth_scope("google")
        .map_err(|e| AegisHttpError::Internal(e))?;

    let spotify_connected = db
        .is_oauth_connected("spotify")
        .map_err(|e| AegisHttpError::Internal(e))?;
    let spotify_scope = db
        .get_oauth_scope("spotify")
        .map_err(|e| AegisHttpError::Internal(e))?;

    Ok(Json(OAuthStatusResponse {
        google: ProviderStatus {
            connected: google_connected,
            scope: google_scope,
        },
        spotify: ProviderStatus {
            connected: spotify_connected,
            scope: spotify_scope,
        },
    }))
}

pub async fn disconnect_provider(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    Path(provider): Path<String>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    if !["google", "spotify"].contains(&provider.as_str()) {
        return Err(AegisHttpError::BadRequest("Unknown provider".into()));
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(e))?;

    db.revoke_oauth(&provider)
        .map_err(|e| AegisHttpError::Internal(e))?;

    tracing::info!(
        tenant = %auth.tenant_id,
        provider = %provider,
        "OAuth disconnected"
    );

    Ok(Json(json!({ "success": true })))
}

pub fn router() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/tokens", axum::routing::post(receive_tokens))
        .route("/status", axum::routing::get(get_status))
        .route("/{provider}", axum::routing::delete(disconnect_provider))
}
