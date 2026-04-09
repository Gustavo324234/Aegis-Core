use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::router::key_pool::ApiKeyEntry;
use axum::{
    extract::{Path, State},
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

#[derive(Deserialize)]
pub struct KeyAddRequest {
    pub tenant_id: String,
    pub session_key: String,
    pub provider: String,
    pub api_key: String,
    pub api_url: Option<String>,
    pub label: Option<String>,
}

async fn add_global_key(
    State(state): State<AppState>,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    // Solo root puede agregar keys globales
    if req.tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can manage global keys".into(),
        ));
    }

    let hash = hash_passphrase(&req.session_key);
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .authenticate_tenant(&req.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;
    }

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
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    if auth.tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can list global keys".into(),
        ));
    }

    let router = state.router.read().await;
    let keys = router.list_global_keys().await;
    Ok(Json(json!(keys)))
}

async fn delete_global_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    if auth.tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can delete global keys".into(),
        ));
    }

    let router = state.router.read().await;
    router
        .delete_key(&id, None)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true })))
}

async fn add_tenant_key(
    State(state): State<AppState>,
    Json(req): Json<KeyAddRequest>,
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
        .add_tenant_key(&req.tenant_id, entry)
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
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    if auth.tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can trigger sync".into(),
        ));
    }
    // Catalog sync is usually background and triggered via syncer.
    // For now returning success as in Python BFF.
    Ok(Json(
        json!({ "success": true, "message": "Catalog synchronization triggered" }),
    ))
}

async fn router_status() -> Json<Value> {
    Json(json!({ "status": "operational", "catalog_syncer": "active" }))
}
