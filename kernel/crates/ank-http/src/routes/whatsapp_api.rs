use crate::citadel::CitadelAuthenticated;
use crate::error::AegisHttpError;
use crate::state::AppState;
use ank_core::enclave::TenantDB;
use ank_core::whatsapp::{
    cancel_pending_messages, get_rate_usage, get_require_confirmation, set_rate_config,
    set_require_confirmation, WhatsAppRateConfig,
};
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
pub struct DisconnectBody {
    #[serde(default)]
    logout: bool,
}

#[derive(Deserialize)]
pub struct SettingsBody {
    #[serde(default)]
    require_confirmation: Option<bool>,
    #[serde(default)]
    rate_config: Option<WhatsAppRateConfig>,
}

#[derive(Deserialize, Default)]
pub struct ChatsQuery {
    #[serde(default)]
    search: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

async fn get_status(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let snapshot = state
        .whatsapp
        .get_status(&auth.tenant_id, &auth.session_key_hash, true)
        .await
        .map_err(AegisHttpError::Internal)?;
    Ok(Json(snapshot))
}

async fn connect(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let conn = state
        .whatsapp
        .connect(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::BadRequest(e.to_string()))?;
    Ok(Json(json!({ "connection": conn })))
}

async fn disconnect(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<DisconnectBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let conn = state
        .whatsapp
        .disconnect(&auth.tenant_id, body.logout)
        .await
        .map_err(AegisHttpError::Internal)?;
    Ok(Json(json!({ "connection": conn })))
}

async fn update_settings(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<SettingsBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let res = TenantDB::with_async(auth.tenant_id, auth.session_key_hash, move |db| {
        if let Some(enabled) = body.require_confirmation {
            set_require_confirmation(db, enabled)?;
        }
        if let Some(cfg) = body.rate_config {
            set_rate_config(db, cfg)?;
        }
        Ok(json!({
            "success": true,
            "require_confirmation": get_require_confirmation(db),
            "rate_usage": get_rate_usage(db),
        }))
    })
    .await
    .map_err(AegisHttpError::Internal)?;

    Ok(Json(res))
}

async fn list_chats(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Query(query): Query<ChatsQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let data = state
        .whatsapp
        .list_chats(&auth.tenant_id, Some(&query.search), query.limit)
        .await
        .map_err(|e| AegisHttpError::BadRequest(e.to_string()))?;
    Ok(Json(data))
}

async fn confirm_pending(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(pending_id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    let res = state
        .whatsapp
        .confirm_pending_and_send(&auth.tenant_id, &auth.session_key_hash, Some(&pending_id))
        .await
        .map_err(|e| AegisHttpError::BadRequest(e.to_string()))?;
    Ok(Json(res))
}

async fn cancel_pending(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(pending_id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    let removed = TenantDB::with_async(auth.tenant_id, auth.session_key_hash, move |db| {
        cancel_pending_messages(db, Some(&pending_id))
    })
    .await
    .map_err(AegisHttpError::Internal)?;
    Ok(Json(json!({ "success": true, "removed": removed })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/connect", post(connect))
        .route("/disconnect", post(disconnect))
        .route("/settings", post(update_settings))
        .route("/chats", get(list_chats))
        .route("/pending/:id/confirm", post(confirm_pending))
        .route("/pending/:id", delete(cancel_pending))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatsapp_request_body_deserialization() {
        let d_default: DisconnectBody = serde_json::from_str("{}").unwrap();
        assert!(!d_default.logout);

        let d_logout: DisconnectBody = serde_json::from_str(r#"{"logout":true}"#).unwrap();
        assert!(d_logout.logout);

        let q: ChatsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.search, "");
        assert_eq!(q.limit, 20);

        let s: SettingsBody = serde_json::from_str(r#"{"require_confirmation":false}"#).unwrap();
        assert_eq!(s.require_confirmation, Some(false));
        assert!(s.rate_config.is_none());
    }
}
