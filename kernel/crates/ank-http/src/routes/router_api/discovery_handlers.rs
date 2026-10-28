use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::router::discovery::{fetch_provider_models, DiscoveredModel};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;

/// Providers that don't require an API key (local / unauthenticated endpoints).
/// Ollama runs on localhost with no auth by default; "custom" may or may not
/// need one but we don't enforce it — if the endpoint needs a key the caller
/// can still supply it.
pub(crate) fn is_keyless_provider(provider: &str) -> bool {
    matches!(provider.to_lowercase().as_str(), "ollama" | "custom")
}

#[derive(Deserialize, ToSchema)]
pub struct ProbeModelsRequest {
    pub provider: String,
    /// Optional stored key_id to look up the saved API key.
    #[serde(default)]
    pub key_id: Option<String>,
    /// Required for paid providers (gemini, openai, anthropic, groq, etc.) if key_id is not given.
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
pub async fn probe_models(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<ProbeModelsRequest>,
) -> Result<Json<ProbeModelsResponse>, AegisHttpError> {
    let router = state.router.read().await;

    let (api_key, api_url) = if let Some(key_id) = &req.key_id {
        if let Some(entry) = router.get_raw_key_by_id(key_id, None).await.or(router
            .get_raw_key_by_id(key_id, Some(&auth.tenant_id))
            .await)
        {
            let key = req
                .api_key
                .filter(|k| !k.trim().is_empty())
                .unwrap_or(entry.api_key);
            let url = req.api_url.or(entry.api_url);
            (key, url)
        } else {
            let key = req.api_key.filter(|k| !k.trim().is_empty());
            if is_keyless_provider(&req.provider) {
                (key.unwrap_or_default(), req.api_url.clone())
            } else {
                let k = key.ok_or_else(|| {
                    AegisHttpError::BadRequest("api_key is required to probe this provider".into())
                })?;
                (k, req.api_url.clone())
            }
        }
    } else if let Some(key) = req.api_key.filter(|k| !k.trim().is_empty()) {
        (key, req.api_url.clone())
    } else if is_keyless_provider(&req.provider) {
        (String::new(), req.api_url.clone())
    } else {
        return Err(AegisHttpError::BadRequest(
            "api_key or key_id is required to probe this provider".into(),
        ));
    };

    // Normalise so the response echoes the canonical id the catalog uses,
    // not whatever spelling the UI happened to send.
    let canonical_provider = ank_core::router::normalize_provider_id(&req.provider);
    match fetch_provider_models(&canonical_provider, api_url.as_deref(), &api_key).await {
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
pub(crate) async fn auto_discover_models(
    provider: &str,
    api_url: Option<&str>,
    api_key: &str,
) -> Vec<String> {
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
