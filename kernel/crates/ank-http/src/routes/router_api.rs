use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use ank_core::router::discovery::{fetch_provider_models, DiscoveredModel};
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

    // CORE-FIX: when the caller didn't pre-specify which models the key
    // should expose, probe the provider's /models endpoint. This is how the
    // catalog stays in sync with what the provider actually offers today
    // (instead of hardcoded ids that go stale the moment a new Gemini
    // ships). Falls back to None/empty if discovery fails — the user can
    // always edit the list afterwards.
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

    // CORE-FIX: normalise provider id BEFORE persisting. The key, the catalog
    // entries it produces, and the router lookups all key off the same string;
    // if the UI submits "google" and we save "google" but catalog stores
    // "gemini", routing silently fails to find the key on chat.
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

    // Sync models into catalog immediately after key registration
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
    // Al crear (POST), la api_key es obligatoria — salvo para providers locales.
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

    // CORE-FIX: same auto-discovery as add_global_key — keep catalog fresh
    // instead of relying on the user remembering the exact model id strings.
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

    // Sync models into catalog immediately after key registration
    let catalog = router.catalog_ref();
    if entry.provider == "openrouter" {
        match syncer::sync_openrouter_free_models(&entry.api_key, &catalog).await {
            Ok(n) if n > 0 => info!("CatalogSyncer: added {} free OpenRouter models (tenant)", n),
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
                "CatalogSyncer: registered {} {} models into catalog (tenant)",
                n, entry.provider
            );
        }
    }

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

    let router = state.router.read().await;

    // Si api_key viene vacía, preservar la key almacenada para no sobrescribirla
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

    // Sync models into catalog after key update
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

    // Si api_key viene vacía, preservar la key almacenada para no sobrescribirla
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

    // Sync models into catalog after key update
    let catalog = router.catalog_ref();
    if entry.provider == "openrouter" {
        match syncer::sync_openrouter_free_models(&entry.api_key, &catalog).await {
            Ok(n) if n > 0 => info!("CatalogSyncer: added {} free OpenRouter models (tenant)", n),
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
                "CatalogSyncer: registered {} {} models into catalog (tenant)",
                n, entry.provider
            );
        }
    }

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

/// Providers that don't require an API key (local / unauthenticated endpoints).
/// Ollama runs on localhost with no auth by default; "custom" may or may not
/// need one but we don't enforce it — if the endpoint needs a key the caller
/// can still supply it.
fn is_keyless_provider(provider: &str) -> bool {
    matches!(provider.to_lowercase().as_str(), "ollama" | "custom")
}

// ── Model discovery ────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct ProbeModelsRequest {
    pub provider: String,
    /// Required for paid providers (gemini, openai, anthropic, groq, etc.).
    /// Ollama local can omit it.
    #[serde(default)]
    #[schema(format = "password")]
    pub api_key: Option<String>,
    /// Optional override for custom endpoints (Ollama remote, OpenAI-compat gateways).
    #[serde(default)]
    pub api_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProbeModelsResponse {
    pub provider: String,
    pub models: Vec<DiscoveredModel>,
}

/// Probe a provider's `/models` endpoint without persisting the key.
/// Lets the UI present the actual list of models the key has access to
/// (instead of a hardcoded option set that drifts as providers ship updates).
#[utoipa::path(
    post,
    path = "/api/router/keys/probe-models",
    tag = "router",
    request_body = ProbeModelsRequest,
    responses(
        (status = 200, description = "Discovered models", body = ProbeModelsResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 502, description = "Provider rejected the key or unreachable")
    )
)]
async fn probe_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ProbeModelsRequest>,
) -> Result<Json<ProbeModelsResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let api_key = if is_keyless_provider(&req.provider) {
        req.api_key.unwrap_or_default()
    } else {
        req.api_key
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                AegisHttpError::BadRequest("api_key is required to probe this provider".into())
            })?
    };

    // Normalise so the response echoes the canonical id the catalog uses,
    // not whatever spelling the UI happened to send.
    let canonical_provider = ank_core::router::normalize_provider_id(&req.provider);
    match fetch_provider_models(&canonical_provider, req.api_url.as_deref(), &api_key).await {
        Ok(models) => Ok(Json(ProbeModelsResponse {
            provider: canonical_provider,
            models,
        })),
        Err(e) => {
            warn!(
                provider = %req.provider,
                error = %e,
                "probe_models: provider rejected the key or is unreachable"
            );
            Err(AegisHttpError::BadGateway(format!(
                "Provider '{}' did not return a model list: {}. Verify the key, \
                 the api_url, and that the provider is reachable from this host.",
                req.provider, e
            )))
        }
    }
}

/// Auto-discover models when the caller didn't pass an explicit list.
/// Returns the discovered ids so the caller can set them on the key entry.
/// Empty result means "no discovery available for this provider" OR "discovery
/// failed" — either way the caller should keep going and just register the key
/// with whatever models the user already provided.
async fn auto_discover_models(provider: &str, api_url: Option<&str>, api_key: &str) -> Vec<String> {
    match fetch_provider_models(provider, api_url, api_key).await {
        Ok(models) => {
            let ids: Vec<String> = models.into_iter().map(|m| m.model_id).collect();
            if !ids.is_empty() {
                info!(
                    provider = provider,
                    count = ids.len(),
                    "router_api: auto-discovered models for new key"
                );
            }
            ids
        }
        Err(e) => {
            warn!(
                provider = provider,
                error = %e,
                "router_api: discovery failed; falling back to user-supplied models"
            );
            Vec::new()
        }
    }
}

// ── MODULES UI INTEGRATION ENDPOINTS (PHASE 4) ───────────────────────────────

#[derive(Serialize)]
pub struct ModuleUiResponse {
    pub module_id: String,
    pub display_name: String,
    pub version: String,
    pub active: bool,
    pub exposed_tools: Vec<ank_core::router::modules::ExposedTool>,
    pub ui_views: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ModulesListResponse {
    pub modules: Vec<ModuleUiResponse>,
}

#[derive(Deserialize)]
pub struct EnableModuleBody {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ExecuteToolBody {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

async fn list_modules(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<ModulesListResponse>, AegisHttpError> {
    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    // Open TenantDB to query module status
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let mut response_modules = Vec::new();

    for manifest in modules_registry.values() {
        let active = db
            .get_kv(&format!("module_active:{}", manifest.module_id))
            .unwrap_or(None)
            .map(|v| v == "true")
            .unwrap_or(false);

        response_modules.push(ModuleUiResponse {
            module_id: manifest.module_id.clone(),
            display_name: manifest.display_name.clone(),
            version: manifest.version.clone(),
            active,
            exposed_tools: manifest.exposed_tools.clone(),
            ui_views: manifest.ui_views.clone(),
        });
    }

    Ok(Json(ModulesListResponse {
        modules: response_modules,
    }))
}

async fn enable_module(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(module_id): Path<String>,
    Json(body): Json<EnableModuleBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    // Open TenantDB to toggle status
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    if !modules_registry.contains_key(&module_id) {
        return Err(AegisHttpError::BadRequest(format!(
            "Module '{}' not found in system",
            module_id
        )));
    }

    db.set_kv(
        &format!("module_active:{}", module_id),
        if body.enabled { "true" } else { "false" },
    )
    .map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to set active status in DB: {}", e))
    })?;

    info!(
        "HTTP API: Module '{}' active status set to {} for tenant '{}'",
        module_id, body.enabled, auth.tenant_id
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "module_id": module_id,
        "active": body.enabled
    })))
}

async fn execute_module_tool(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(module_id): Path<String>,
    Json(body): Json<ExecuteToolBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    // Verify that the module is active for this tenant
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let is_active = db
        .get_kv(&format!("module_active:{}", module_id))
        .unwrap_or(None)
        .map(|v| v == "true")
        .unwrap_or(false);

    if !is_active {
        return Err(AegisHttpError::BadRequest(format!(
            "Module '{}' is not enabled for this tenant",
            module_id
        )));
    }

    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    let manifest = modules_registry.get(&module_id).ok_or_else(|| {
        AegisHttpError::BadRequest(format!("Module '{}' not found in system", module_id))
    })?;

    // Verify the tool is exposed by this module
    if !manifest
        .exposed_tools
        .iter()
        .any(|t| t.name == body.tool_name)
    {
        return Err(AegisHttpError::BadRequest(format!(
            "Tool '{}' is not exposed by module '{}'",
            body.tool_name, module_id
        )));
    }

    let endpoint = manifest.ipc_transport.endpoint.clone();

    // Call the external module tool via gRPC (using tonic)
    let channel_url = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint
    } else {
        format!("http://{}", endpoint)
    };

    let endpoint_parsed = tonic::transport::Endpoint::from_shared(channel_url)
        .map_err(|e| AegisHttpError::BadRequest(format!("Invalid endpoint format: {}", e)))?
        .timeout(std::time::Duration::from_secs(5))
        .connect_timeout(std::time::Duration::from_secs(5));

    let channel = endpoint_parsed.connect().await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!(
            "Failed to connect to microkernel module: {}",
            e
        ))
    })?;

    let mut client =
        ank_proto::v1::domain_module_service_client::DomainModuleServiceClient::new(channel);

    // Convert arguments to string JSON
    let args_str = serde_json::to_string(&body.arguments)
        .map_err(|e| AegisHttpError::BadRequest(format!("Invalid arguments format: {}", e)))?;

    let request = tonic::Request::new(ank_proto::v1::ExecuteToolRequest {
        tool_name: body.tool_name,
        arguments_json: args_str,
        tenant_id: auth.tenant_id.clone(),
    });

    let response = client
        .execute_tool(request)
        .await
        .map_err(|e| {
            AegisHttpError::Internal(anyhow::anyhow!("Module tool execution failed: {}", e))
        })?
        .into_inner();

    if response.success {
        // Formulate result as a parsed JSON value
        let parsed_result: serde_json::Value = serde_json::from_str(&response.result_json)
            .unwrap_or(serde_json::Value::String(response.result_json));
        Ok(Json(serde_json::json!({
            "success": true,
            "result": parsed_result
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "error": response.result_json
        })))
    }
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
    let is_master = {
        if let (Some(tenant_id), Some(key)) = (
            headers.get("x-citadel-tenant").and_then(|v| v.to_str().ok()),
            headers.get("x-citadel-key").and_then(|v| v.to_str().ok()),
        ) {
            let hash = hash_passphrase(key);
            let citadel = state.citadel.lock().await;
            citadel.enclave.authenticate_master(tenant_id, &hash).await.unwrap_or(false)
        } else {
            false
        }
    };

    let tenant_id = if is_master {
        None
    } else {
        let tenant_id = headers.get("x-citadel-tenant")
            .and_then(|v| v.to_str().ok())
            .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        let key = headers.get("x-citadel-key")
            .and_then(|v| v.to_str().ok())
            .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        let hash = hash_passphrase(key);
        {
            let citadel = state.citadel.lock().await;
            citadel.enclave.authenticate_tenant(tenant_id, &hash).await
                .map_err(|_| AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        }
        Some(tenant_id)
    };

    let router = state.router.read().await;
    let backup = router.key_pool.export_keys_encrypted(tenant_id, &req.password).await
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
    let is_master = {
        if let (Some(tenant_id), Some(key)) = (
            headers.get("x-citadel-tenant").and_then(|v| v.to_str().ok()),
            headers.get("x-citadel-key").and_then(|v| v.to_str().ok()),
        ) {
            let hash = hash_passphrase(key);
            let citadel = state.citadel.lock().await;
            citadel.enclave.authenticate_master(tenant_id, &hash).await.unwrap_or(false)
        } else {
            false
        }
    };

    let tenant_id = if is_master {
        None
    } else {
        let tenant_id = headers.get("x-citadel-tenant")
            .and_then(|v| v.to_str().ok())
            .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        let key = headers.get("x-citadel-key")
            .and_then(|v| v.to_str().ok())
            .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        let hash = hash_passphrase(key);
        {
            let citadel = state.citadel.lock().await;
            citadel.enclave.authenticate_tenant(tenant_id, &hash).await
                .map_err(|_| AegisHttpError::Citadel(CitadelError::Unauthorized))?;
        }
        Some(tenant_id)
    };

    let backup = ank_core::router::key_pool::EncryptedKeysBackup {
        salt: req.salt,
        nonce: req.nonce,
        ciphertext: req.ciphertext,
    };

    let router = state.router.read().await;
    let count = router.key_pool.import_keys_encrypted(tenant_id, &req.password, backup).await
        .map_err(|e| AegisHttpError::BadRequest(format!("Import failed: {}", e)))?;

    if let Err(e) = router.catalog.sync_providers(&router.key_pool).await {
        warn!("Import: catalog sync failed after key import: {}", e);
    }

    Ok(Json(ImportResponse {
        success: true,
        count,
    }))
}
