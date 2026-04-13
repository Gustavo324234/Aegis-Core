use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
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

const ANTHROPIC_MODELS: &[&str] = &[
    "claude-opus-4-5",
    "claude-sonnet-4-5",
    "claude-haiku-4-5-20251001",
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
    "claude-3-opus-20240229",
];

const GEMINI_MODELS: &[&str] = &[
    "gemini-2.0-flash",
    "gemini-2.0-flash-lite",
    "gemini-1.5-pro",
    "gemini-1.5-flash",
];

/// Allowlist de hosts permitidos para requests HTTP salientes.
const ALLOWED_API_HOSTS: &[&str] = &[
    "api.openai.com",
    "api.anthropic.com",
    "api.groq.com",
    "openrouter.ai",
    "generativelanguage.googleapis.com",
    "api.together.xyz",
    "localhost",
    "127.0.0.1",
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

async fn list_provider_models(
    State(_state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(req): Json<ProviderModelsRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let mut models = Vec::new();

    match req.provider.as_str() {
        "anthropic" => {
            models = ANTHROPIC_MODELS.iter().map(|s| s.to_string()).collect();
        }
        "gemini" => {
            models = GEMINI_MODELS.iter().map(|s| s.to_string()).collect();
        }
        "ollama" => {
            let client = reqwest::Client::new();
            let res = client
                .get("http://localhost:11434/api/tags")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
                .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

            let data: Value = res
                .json()
                .await
                .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
            if let Some(list) = data.get("models").and_then(|m| m.as_array()) {
                for m in list {
                    if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                        models.push(name.to_string());
                    }
                }
            }
        }
        _ => {
            validate_api_url(&req.api_url)?;

            let client = reqwest::Client::new();
            let mut base_url = req
                .api_url
                .split("/chat/completions")
                .next()
                .unwrap_or(&req.api_url)
                .to_string();

            let models_url = if base_url.ends_with("/v1") {
                format!("{}/models", base_url)
            } else {
                if base_url.ends_with('/') {
                    base_url.pop();
                }
                format!("{}/v1/models", base_url)
            };

            let res = client
                .get(&models_url)
                .header("Authorization", format!("Bearer {}", req.api_key))
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
                .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

            let data: Value = res
                .json()
                .await
                .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
            if let Some(list) = data.get("data").and_then(|d| d.as_array()) {
                for m in list {
                    if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                        models.push(id.to_string());
                    }
                }
            }
        }
    }

    models.sort();
    Ok(Json(json!({ "models": models })))
}
