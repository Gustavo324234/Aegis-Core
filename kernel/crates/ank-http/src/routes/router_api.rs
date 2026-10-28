use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::router::key_pool::ApiKeyEntry;
use ank_core::router::syncer;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;

pub mod discovery_handlers;
pub mod modules_handlers;

pub use discovery_handlers::{probe_models, ProbeModelsRequest, ProbeModelsResponse};
pub use modules_handlers::{
    enable_module, execute_module_tool, list_modules, EnableModuleBody, ExecuteToolBody,
    ModuleUiResponse, ModulesListResponse,
};

use discovery_handlers::{auto_discover_models, is_keyless_provider};

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
        // CORE-FIX: probe a provider's /models endpoint before the user commits
        // to a key, so the UI can show the actual available list instead of a
        // hand-curated (and inevitably stale) selection.
        .route("/keys/probe-models", post(probe_models))
        .route("/keys/export", post(export_keys))
        .route("/keys/import", post(import_keys))
        .route("/models", get(list_router_models))
        .route("/sync", post(sync_router_catalog))
        .route("/status", get(router_status))
        // CORE-322: routing telemetry — outcomes, open circuits, observed
        // latency and sticky decisions, so the UI can answer "why did the
        // router pick X?" instead of digging through server logs.
        .route("/stats", get(router_stats))
        .route("/modules", get(list_modules))
        .route("/modules/:module_id/enable", post(enable_module))
        .route("/modules/:module_id/execute", post(execute_module_tool))
}

#[derive(Deserialize, ToSchema)]
pub struct KeyAddRequest {
    #[schema(example = "openrouter")]
    pub provider: Option<String>,
    /// La api_key es requerida al crear (POST). En edición (PUT), si se omite o
    /// viene vacía se preserva la key almacenada — evita sobrescribirla accidentalmente.
    #[serde(default)]
    #[schema(format = "password")]
    pub api_key: Option<String>,
    #[schema(example = "https://openrouter.ai/api/v1")]
    pub api_url: Option<String>,
    #[schema(example = "production-key-1")]
    pub label: Option<String>,
    #[schema(example = "[\"openai/gpt-4o\"]")]
    pub models: Option<Vec<String>>,
    /// Si true, esta clave usa el nivel gratuito — se usa antes que las claves pagas.
    #[serde(default)]
    pub is_free_tier: Option<bool>,
    /// Si false, esta clave se desactiva para el enrutamiento.
    #[serde(default)]
    pub is_active: Option<bool>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
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

/// Sincroniza inmediatamente los modelos asociados a una clave recién agregada o editada (CORE-356).
async fn sync_key_models_into_catalog(
    router: &ank_core::router::CognitiveRouter,
    entry: &ApiKeyEntry,
) {
    let catalog = router.catalog_ref();
    if entry.provider == "openrouter" {
        match syncer::sync_openrouter_free_models(&entry.api_key, &catalog).await {
            Ok(n) if n > 0 => info!("CatalogSyncer: added {} free OpenRouter models", n),
            Ok(_) => {}
            Err(e) => warn!(
                "CatalogSyncer: failed to sync OpenRouter free models: {}",
                e
            ),
        }
    } else if let Some(models) = &entry.active_models {
        let n = syncer::register_provider_models(&entry.provider, models, &catalog).await;
        if n > 0 {
            info!(
                "CatalogSyncer: registered {} {} models into catalog",
                n, entry.provider
            );
        }
    }
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

    // Al crear (POST), la api_key es obligatoria — salvo para providers locales
    // (ollama, custom) que no necesitan autenticación.
    let provider_raw = req.provider.as_ref().ok_or_else(|| {
        AegisHttpError::BadRequest("provider is required when adding a new key".into())
    })?;

    let api_key = if is_keyless_provider(provider_raw) {
        req.api_key.unwrap_or_default()
    } else {
        req.api_key
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                AegisHttpError::BadRequest("api_key is required when adding a new key".into())
            })?
    };

    let active_models = match req.models {
        Some(ms) if !ms.is_empty() => Some(ms),
        _ => {
            let discovered =
                auto_discover_models(provider_raw, req.api_url.as_deref(), &api_key).await;
            if discovered.is_empty() {
                None
            } else {
                Some(discovered)
            }
        }
    };

    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: ank_core::router::normalize_provider_id(provider_raw),
        api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: req.is_active.unwrap_or(true),
        rate_limited_until: None,
        active_models,
        is_free_tier: req.is_free_tier.unwrap_or(false),
    };

    let router = state.router.read().await;
    router
        .add_global_key(entry.clone())
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    router.invalidate_all_sticky().await;
    sync_key_models_into_catalog(&router, &entry).await;

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
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn list_global_keys(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<GlobalKeysResponse>, AegisHttpError> {
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
    router.invalidate_all_sticky().await;

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
    let provider_raw = req.provider.as_ref().ok_or_else(|| {
        AegisHttpError::BadRequest("provider is required when adding a new key".into())
    })?;

    let api_key = if is_keyless_provider(provider_raw) {
        req.api_key.unwrap_or_default()
    } else {
        req.api_key
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                AegisHttpError::BadRequest("api_key is required when adding a new key".into())
            })?
    };

    let active_models = match req.models {
        Some(ms) if !ms.is_empty() => Some(ms),
        _ => {
            let discovered =
                auto_discover_models(provider_raw, req.api_url.as_deref(), &api_key).await;
            if discovered.is_empty() {
                None
            } else {
                Some(discovered)
            }
        }
    };

    let entry = ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider: ank_core::router::normalize_provider_id(provider_raw),
        api_key,
        api_url: req.api_url,
        label: req.label,
        is_active: req.is_active.unwrap_or(true),
        rate_limited_until: None,
        active_models,
        is_free_tier: req.is_free_tier.unwrap_or(false),
    };

    let router = state.router.read().await;
    router
        .add_tenant_key(&auth.tenant_id, entry.clone())
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    router.invalidate_sticky(&auth.tenant_id).await;
    sync_key_models_into_catalog(&router, &entry).await;

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
    router.invalidate_sticky(&auth.tenant_id).await;

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
    let synced_at = router.last_synced().await.map(|dt| dt.to_rfc3339());
    let models: Vec<serde_json::Value> = raw_models
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "model_id": m.model_id,
                "display_name": m.display_name,
                "provider": m.provider,
                "context_window": m.context_window,
                "cost_input_per_mtok": m.cost_input_per_mtok,
                "cost_output_per_mtok": m.cost_output_per_mtok,
                "is_local": m.is_local,
                "task_scores": {
                    "chat": m.task_scores.chat,
                    "coding": m.task_scores.coding,
                    "planning": m.task_scores.planning,
                    "analysis": m.task_scores.analysis,
                    "summarization": m.task_scores.summarization,
                    "extraction": m.task_scores.extraction,
                },
            })
        })
        .collect();
    Ok(Json(RouterModelsResponse { models, synced_at }))
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
    if let Some(syncer) = &state.catalog_syncer {
        syncer
            .sync_now()
            .await
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Catalog sync failed: {}", e)))?;
    }
    Ok(Json(SyncResponse {
        success: true,
        message: "Catalog synchronization completed".to_string(),
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

    let router = state.router.read().await;

    let existing_key = router.get_raw_key_by_id(&id, None).await;
    let existing = existing_key
        .as_ref()
        .ok_or_else(|| AegisHttpError::BadRequest(format!("Key '{}' not found", id)))?;

    let provider = match &req.provider {
        Some(p) => ank_core::router::normalize_provider_id(p),
        None => existing.provider.clone(),
    };

    let api_key = match req.api_key.filter(|k| !k.trim().is_empty()) {
        Some(k) => k,
        None => existing.api_key.clone(),
    };

    let entry = ApiKeyEntry {
        key_id: id,
        provider,
        api_key,
        api_url: req.api_url.or(existing.api_url.clone()),
        label: req.label.or(existing.label.clone()),
        is_active: req.is_active.unwrap_or(existing.is_active),
        rate_limited_until: existing.rate_limited_until,
        active_models: req.models.or(existing.active_models.clone()),
        is_free_tier: req.is_free_tier.unwrap_or(existing.is_free_tier),
    };

    router
        .add_global_key(entry.clone())
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    router.invalidate_all_sticky().await;
    sync_key_models_into_catalog(&router, &entry).await;

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
    let router = state.router.read().await;

    let existing_key = router.get_raw_key_by_id(&id, Some(&auth.tenant_id)).await;
    let existing = existing_key
        .as_ref()
        .ok_or_else(|| AegisHttpError::BadRequest(format!("Key '{}' not found", id)))?;

    let provider = match &req.provider {
        Some(p) => ank_core::router::normalize_provider_id(p),
        None => existing.provider.clone(),
    };

    let api_key = match req.api_key.filter(|k| !k.trim().is_empty()) {
        Some(k) => k,
        None => existing.api_key.clone(),
    };

    let entry = ApiKeyEntry {
        key_id: id,
        provider,
        api_key,
        api_url: req.api_url.or(existing.api_url.clone()),
        label: req.label.or(existing.label.clone()),
        is_active: req.is_active.unwrap_or(existing.is_active),
        rate_limited_until: existing.rate_limited_until,
        active_models: req.models.or(existing.active_models.clone()),
        is_free_tier: req.is_free_tier.unwrap_or(existing.is_free_tier),
    };

    router
        .add_tenant_key(&auth.tenant_id, entry.clone())
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    router.invalidate_sticky(&auth.tenant_id).await;
    sync_key_models_into_catalog(&router, &entry).await;

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

/// CORE-322: routing telemetry for the UI — per-model success/failure
/// outcomes, observed latency, currently-open circuits (with retry
/// countdowns) and the active sticky decisions. Secrets-free by
/// construction (`CognitiveRouter::stats` exposes ids and counters only).
async fn router_stats(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<ank_core::router::RouterStats>, AegisHttpError> {
    let router = state.router.read().await;
    Ok(Json(router.stats().await))
}

#[derive(Deserialize, ToSchema)]
pub struct KeyExportRequest {
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
pub struct KeyImportRequest {
    pub password: String,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Serialize, ToSchema)]
pub struct ImportResponse {
    pub success: bool,
    pub count: usize,
}

/// Resuelve si las credenciales en headers corresponden al Master Admin (`Ok(None)`)
/// o a un tenant autenticado (`Ok(Some(tenant_id))`) para operaciones de backup (CORE-356).
async fn resolve_backup_tenant_scope(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<String>, AegisHttpError> {
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
    let key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
    let hash = hash_passphrase(key);

    let citadel = state.citadel.lock().await;
    if citadel
        .enclave
        .authenticate_master(tenant_id, &hash)
        .await
        .unwrap_or(false)
    {
        return Ok(None);
    }

    let is_tenant = citadel
        .enclave
        .authenticate_tenant(tenant_id, &hash)
        .await
        .map_err(|_| AegisHttpError::Citadel(CitadelError::Unauthorized))?;
    if !is_tenant {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    Ok(Some(tenant_id.to_string()))
}

#[utoipa::path(
    post,
    path = "/api/router/keys/export",
    tag = "router",
    request_body = KeyExportRequest,
    responses(
        (status = 200, description = "Keys exported successfully"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn export_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KeyExportRequest>,
) -> Result<Json<ank_core::router::key_pool::EncryptedKeysBackup>, AegisHttpError> {
    let tenant_scope = resolve_backup_tenant_scope(&state, &headers).await?;

    let router = state.router.read().await;
    let backup = router
        .key_pool_ref()
        .export_keys_encrypted(tenant_scope.as_deref(), &req.password)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Export failed: {}", e)))?;

    Ok(Json(backup))
}

#[utoipa::path(
    post,
    path = "/api/router/keys/import",
    tag = "router",
    request_body = KeyImportRequest,
    responses(
        (status = 200, description = "Keys imported successfully"),
        (status = 401, description = "Unauthorized")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
async fn import_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<KeyImportRequest>,
) -> Result<Json<ImportResponse>, AegisHttpError> {
    let tenant_scope = resolve_backup_tenant_scope(&state, &headers).await?;

    let backup = ank_core::router::key_pool::EncryptedKeysBackup {
        salt: req.salt,
        nonce: req.nonce,
        ciphertext: req.ciphertext,
    };

    let router = state.router.read().await;
    let count = router
        .key_pool_ref()
        .import_keys_encrypted(tenant_scope.as_deref(), &req.password, backup)
        .await
        .map_err(|e| AegisHttpError::BadRequest(format!("Import failed: {}", e)))?;

    match tenant_scope.as_deref() {
        Some(tid) => router.invalidate_sticky(tid).await,
        None => router.invalidate_all_sticky().await,
    }

    if let Some(syncer) = &state.catalog_syncer {
        if let Err(e) = syncer.sync_now().await {
            warn!("Import: catalog sync failed after key import: {}", e);
        }
    }

    Ok(Json(ImportResponse {
        success: true,
        count,
    }))
}
