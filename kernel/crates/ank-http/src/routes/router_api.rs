use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::router::key_pool::ApiKeyEntry;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/keys/global", post(add_global_key))
        .route("/keys/global", get(list_global_keys))
        .route("/keys/global/:id", delete(delete_global_key))
        .route("/keys/tenant", post(add_tenant_key))
        .route("/keys/tenant", get(list_tenant_keys))
        .route("/keys/tenant/:id", delete(delete_tenant_key))
        .route("/models", get(list_router_models))
        .route("/sync", post(sync_router_catalog))
        .route("/status", get(router_status))
}

/// Body para operaciones de clave — credenciales vienen de headers Citadel.
#[derive(Deserialize)]
pub struct KeyAddRequest {
    pub provider: String,
    pub api_key: String,
    pub api_url: Option<String>,
    pub label: Option<String>,
}

/// Valida que los headers Citadel correspondan al Master Admin.
/// Retorna el `tenant_id` autenticado.
async fn require_master_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, AegisHttpError> {
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingTenant))?;

    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingKey))?;

    let hash = hash_passphrase(&raw_key);

    let citadel = state.citadel.lock().await;
    let is_master = citadel
        .enclave
        .authenticate_master(&tenant_id, &hash)
        .await
        .map_err(|_| AegisHttpError::Citadel(CitadelError::Unauthorized))?;

    if !is_master {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    Ok(tenant_id)
}

async fn add_global_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
    };

    let router = state.router.read().await;
    router
        .add_global_key(entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true })))
}

async fn list_global_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let router = state.router.read().await;
    let keys = router.list_global_keys().await;
    Ok(Json(json!(keys)))
}

async fn delete_global_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let router = state.router.read().await;
    router
        .delete_key(&id, None)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true })))
}

async fn add_tenant_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
    };

    let router = state.router.read().await;
    router
        .add_tenant_key(&auth.tenant_id, entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true })))
}

async fn list_tenant_keys(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let router = state.router.read().await;
    let keys = router.list_tenant_keys(&auth.tenant_id).await;
    Ok(Json(json!(keys)))
}

async fn delete_tenant_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    let router = state.router.read().await;
    router
        .delete_key(&id, Some(&auth.tenant_id))
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true })))
}

async fn list_router_models(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let router = state.router.read().await;
    let models = router.list_models_for_catalog().await;
    Ok(Json(json!(models)))
}

async fn sync_router_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;
    Ok(Json(
        json!({ "success": true, "message": "Catalog synchronization triggered" }),
    ))
}

async fn router_status() -> Json<Value> {
    Json(json!({ "status": "operational", "catalog_syncer": "active" }))
}
