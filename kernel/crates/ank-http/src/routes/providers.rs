use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::router::discovery::fetch_provider_models;
use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new().route("/models", post(list_provider_models))
}

#[derive(Deserialize)]
pub struct ProviderModelsRequest {
    pub provider: String,
    pub api_key: String,
    pub api_url: String,
}

#[derive(Serialize)]
pub struct ProviderModelsResponse {
    pub models: Vec<String>,
}

/// Allowlist de hosts permitidos para requests HTTP salientes.
/// Aplica a custom URLs que el operador pueda configurar; los hosts canónicos
/// que `fetch_provider_models` resuelve por sí solo siempre son seguros.
const ALLOWED_API_HOSTS: &[&str] = &[
    "api.openai.com",
    "api.anthropic.com",
    "api.groq.com",
    "openrouter.ai",
    "generativelanguage.googleapis.com",
    "api.together.xyz",
    "api.mistral.ai",
    "api.deepseek.com",
    "api.x.ai",
    "localhost",
    "127.0.0.1",
    "ollama.com",
];

/// Valida que `api_url` apunte a un host en la allowlist.
/// Previene SSRF hacia hosts internos arbitrarios.
fn validate_api_url(url: &str) -> Result<(), AegisHttpError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AegisHttpError::BadRequest("Invalid api_url".into()))?;

    let host = parsed.host_str().unwrap_or("");

    let allowed = ALLOWED_API_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed)));

    if allowed {
        Ok(())
    } else {
        Err(AegisHttpError::BadRequest(format!(
            "api_url host '{}' is not in the allowlist",
            host
        )))
    }
}

/// Lists the models a provider's API key has access to.
///
/// CORE-FIX: previously this endpoint had `GEMINI_MODELS` and `ANTHROPIC_MODELS`
/// as hardcoded constants, so the UI showed stale model lists that drifted
/// every time a provider shipped a new model (Gemini 2.5/3.x, Claude Sonnet 4.6,
/// etc. were invisible). Now we delegate to `ank_core::router::discovery::
/// fetch_provider_models`, which actually hits each provider's `/models`
/// endpoint with the supplied key.
///
/// The response shape stays `{ "models": ["id1", "id2", ...] }` so the existing
/// `ProvidersTab.tsx` UI keeps working unchanged. The richer
/// `{model_id, display_name, context_window, supports_tools}` shape is
/// available via the newer `/api/router/keys/probe-models` endpoint when the
/// UI is ready to consume it.
async fn list_provider_models(
    State(_state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(req): Json<ProviderModelsRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    // Only validate when the operator passed a non-empty custom URL — the
    // canonical defaults built into discovery.rs are trusted and don't need to
    // be re-validated against the allowlist.
    let api_url = if req.api_url.trim().is_empty() {
        None
    } else {
        validate_api_url(&req.api_url)?;
        Some(req.api_url.as_str())
    };

    let discovered = fetch_provider_models(&req.provider, api_url, &req.api_key)
        .await
        .map_err(|e| {
            AegisHttpError::BadGateway(format!(
                "Provider '{}' did not return a model list: {}",
                req.provider, e
            ))
        })?;

    let mut models: Vec<String> = discovered.into_iter().map(|m| m.model_id).collect();
    models.sort();
    Ok(Json(json!({ "models": models })))
}
