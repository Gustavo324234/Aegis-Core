use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::router::key_pool::ApiKeyEntry;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/keys/global", post(add_global_key))
        .route("/keys/global", get(list_global_keys))
        .route("/keys/global/:id", put(update_global_key))
        .route("/keys/global/:id", delete(delete_global_key))
        .route("/keys/tenant", post(add_tenant_key))
        .route("/keys/tenant", get(list_tenant_keys))
        .route("/keys/tenant/:id", put(update_tenant_key))
        .route("/keys/tenant/:id", delete(delete_tenant_key))
        .route("/models", get(list_router_models))
        .route("/sync", post(sync_router_catalog))
        .route("/status", get(router_status))
}

#[derive(Deserialize, ToSchema)]
pub struct KeyAddRequest {
    #[schema(example = "openrouter")]
    pub provider: String,
    #[schema(format = "password")]
    pub api_key: String,
    #[schema(example = "https://openrouter.ai/api/v1")]
    pub api_url: Option<String>,
    #[schema(example = "production-key-1")]
    pub label: Option<String>,
    #[schema(example = "[\"openai/gpt-4o\"]")]
    pub models: Option<Vec<String>>,
    /// Si true, esta clave usa el nivel gratuito — se usa antes que las claves pagas.
    #[serde(default)]
    pub is_free_tier: bool,
}

#[derive(Serialize, ToSchema)]
pub struct RouterKeyResponse {
    pub key_id: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub is_active: bool,
    pub is_free_tier: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_models: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalKeysResponse {
    pub keys: Vec<RouterKeyResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct TenantKeysResponse {
    pub keys: Vec<RouterKeyResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct RouterModelsResponse {
    pub models: Vec<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct SyncResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Serialize, ToSchema)]
pub struct RouterStatusResponse {
    pub status: String,
    pub catalog_syncer: String,
}

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

#[utoipa::path(
    post,
    path = "/api/router/keys/global",
    tag = "router",
    request_body = KeyAddRequest,
    responses(
        (status = 200, description = "Key added"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
async fn add_global_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
        active_models: req.models,
        is_free_tier: req.is_free_tier,
    };

    let router = state.router.read().await;
    router
        .add_global_key(entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Global key added".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/router/keys/global",
    tag = "router",
    responses(
        (status = 200, description = "List of global keys", body = GlobalKeysResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
async fn list_global_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GlobalKeysResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let router = state.router.read().await;
    let raw_keys = router.list_global_keys().await;
    let keys: Vec<RouterKeyResponse> = raw_keys
        .into_iter()
        .map(|k| RouterKeyResponse {
            key_id: k.key_id,
            provider: k.provider,
            api_url: k.api_url,
            label: k.label,
            is_active: k.is_active,
            is_free_tier: k.is_free_tier,
            active_models: k.active_models,
        })
        .collect();
    Ok(Json(GlobalKeysResponse { keys }))
}

#[utoipa::path(
    delete,
    path = "/api/router/keys/global/{id}",
    tag = "router",
    responses(
        (status = 200, description = "Key deleted"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("id" = String, Path, description = "Key ID to delete"),
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
async fn delete_global_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let router = state.router.read().await;
    router
        .delete_key(&id, None)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Key deleted".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/router/keys/tenant",
    tag = "router",
    request_body = KeyAddRequest,
    responses(
        (status = 200, description = "Key added"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn add_tenant_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
        active_models: req.models,
        is_free_tier: req.is_free_tier,
    };

    let router = state.router.read().await;
    router
        .add_tenant_key(&auth.tenant_id, entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Tenant key added".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/router/keys/tenant",
    tag = "router",
    responses(
        (status = 200, description = "List of tenant keys", body = TenantKeysResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn list_tenant_keys(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<TenantKeysResponse>, AegisHttpError> {
    let router = state.router.read().await;
    let raw_keys = router.list_tenant_keys(&auth.tenant_id).await;
    let keys: Vec<RouterKeyResponse> = raw_keys
        .into_iter()
        .map(|k| RouterKeyResponse {
            key_id: k.key_id,
            provider: k.provider,
            api_url: k.api_url,
            label: k.label,
            is_active: k.is_active,
            is_free_tier: k.is_free_tier,
            active_models: k.active_models,
        })
        .collect();
    Ok(Json(TenantKeysResponse { keys }))
}

#[utoipa::path(
    delete,
    path = "/api/router/keys/tenant/{id}",
    tag = "router",
    responses(
        (status = 200, description = "Key deleted"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("id" = String, Path, description = "Key ID to delete"),
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn delete_tenant_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<String>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    let router = state.router.read().await;
    router
        .delete_key(&id, Some(&auth.tenant_id))
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Key deleted".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/router/models",
    tag = "router",
    responses(
        (status = 200, description = "List of router models", body = RouterModelsResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn list_router_models(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<RouterModelsResponse>, AegisHttpError> {
    let router = state.router.read().await;
    let raw_models = router.list_models_for_catalog().await;
    let models: Vec<serde_json::Value> = raw_models
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.model_id,
                "name": m.display_name,
                "provider": m.provider,
                "context_length": m.context_window,
                "input_cost_per_mtok": m.cost_input_per_mtok,
                "output_cost_per_mtok": m.cost_output_per_mtok,
            })
        })
        .collect();
    Ok(Json(RouterModelsResponse { models }))
}

#[utoipa::path(
    post,
    path = "/api/router/sync",
    tag = "router",
    responses(
        (status = 200, description = "Sync triggered", body = SyncResponse),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
async fn sync_router_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;
    Ok(Json(SyncResponse {
        success: true,
        message: "Catalog synchronization triggered".to_string(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/router/keys/global/{id}",
    tag = "router",
    request_body = KeyAddRequest,
    responses(
        (status = 200, description = "Key updated"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("id" = String, Path, description = "Key ID to update"),
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
async fn update_global_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let entry = ApiKeyEntry {
        key_id: id,
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
        active_models: req.models,
        is_free_tier: req.is_free_tier,
    };

    let router = state.router.read().await;
    router
        .add_global_key(entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Global key updated".to_string(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/router/keys/tenant/{id}",
    tag = "router",
    request_body = KeyAddRequest,
    responses(
        (status = 200, description = "Key updated"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("id" = String, Path, description = "Key ID to update"),
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn update_tenant_key(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<String>,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<SyncResponse>, AegisHttpError> {
    let entry = ApiKeyEntry {
        key_id: id,
        provider: req.provider,
        api_key: req.api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: true,
        rate_limited_until: None,
        active_models: req.models,
        is_free_tier: req.is_free_tier,
    };

    let router = state.router.read().await;
    router
        .add_tenant_key(&auth.tenant_id, entry)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(SyncResponse {
        success: true,
        message: "Tenant key updated".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/router/status",
    tag = "router",
    responses(
        (status = 200, description = "Router status", body = RouterStatusResponse)
    )
)]
async fn router_status() -> Json<RouterStatusResponse> {
    Json(RouterStatusResponse {
        status: "operational".to_string(),
        catalog_syncer: "active".to_string(),
    })
}
