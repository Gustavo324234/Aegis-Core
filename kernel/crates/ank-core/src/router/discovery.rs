//! Provider model discovery.
//!
//! Most LLM providers expose a `GET /models` endpoint that lists every model
//! the supplied API key is authorised to call. This module wraps the
//! provider-specific quirks (URL, auth header, response shape) behind a
//! single `fetch_provider_models()` function so the router can keep its
//! catalog in sync without us hardcoding model names that go stale the
//! moment Google ships a new Gemini.
//!
//! Used by:
//! - The HTTP layer when a tenant adds/updates an API key (auto-populates
//!   `active_models` if the caller didn't pass them explicitly).
//! - A `/keys/probe-models` endpoint the UI calls before saving a key so
//!   the user can choose from the actual available list.

use serde::Serialize;
use std::time::Duration;
use tracing::warn;

/// A model exposed by a provider's `/models` endpoint, normalised across
/// providers so callers don't have to reason about response shape.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredModel {
    /// Canonical id as the provider's chat-completions endpoint expects it
    /// (e.g. `gemini-2.5-flash`, `gpt-4o`, `claude-sonnet-4-5`).
    pub model_id: String,
    /// Human-readable name when the provider supplies one, otherwise `model_id`.
    pub display_name: String,
    /// Context window in tokens when the provider reports it, else None.
    pub context_window: Option<u32>,
    /// Best-effort flag: true when the provider explicitly advertises tool/
    /// function-calling support. None when unknown — assume nothing.
    pub supports_tools: Option<bool>,
}

/// Discover the models a given API key has access to.
///
/// `api_url` is the optional override the user typed into the "Custom URL"
/// field (used for Ollama remote, custom OpenAI-compat gateways, etc.). It
/// is the base of the provider's API; we append the appropriate `/models`
/// path. When None we use the provider's well-known default.
///
/// Returns an empty vec — not an error — when the provider isn't supported
/// for discovery (e.g. `"custom"`, `"qwen"` where we don't know a stable
/// listing endpoint). Callers should fall back to the user-supplied
/// `active_models` in that case.
pub async fn fetch_provider_models(
    provider: &str,
    api_url: Option<&str>,
    api_key: &str,
) -> anyhow::Result<Vec<DiscoveredModel>> {
    // CORE-FIX: normalise before matching. Avoids "google" vs "gemini" vs
    // "Google AI" all being treated as distinct unsupported providers.
    let provider_lc = crate::router::normalize_provider_id(provider);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    match provider_lc.as_str() {
        "gemini" => fetch_gemini(&client, api_url, api_key).await,
        "openai" => {
            fetch_openai_compatible(
                &client,
                api_url.unwrap_or("https://api.openai.com/v1"),
                api_key,
            )
            .await
        }
        "groq" => {
            fetch_openai_compatible(
                &client,
                api_url.unwrap_or("https://api.groq.com/openai/v1"),
                api_key,
            )
            .await
        }
        "openrouter" => {
            fetch_openai_compatible(
                &client,
                api_url.unwrap_or("https://openrouter.ai/api/v1"),
                api_key,
            )
            .await
        }
        "mistral" => {
            fetch_openai_compatible(
                &client,
                api_url.unwrap_or("https://api.mistral.ai/v1"),
                api_key,
            )
            .await
        }
        "deepseek" => {
            fetch_openai_compatible(
                &client,
                api_url.unwrap_or("https://api.deepseek.com/v1"),
                api_key,
            )
            .await
        }
        "xai" => {
            fetch_openai_compatible(&client, api_url.unwrap_or("https://api.x.ai/v1"), api_key)
                .await
        }
        "anthropic" => fetch_anthropic(&client, api_url, api_key).await,
        "ollama" => fetch_ollama(&client, api_url, api_key, false).await,
        "ollama_cloud" => fetch_ollama(&client, api_url, api_key, true).await,
        other => {
            warn!(
                provider = other,
                "discovery: no /models endpoint configured for this provider — \
                 caller should fall back to user-supplied active_models"
            );
            Ok(Vec::new())
        }
    }
}

/// Normalises a user-supplied `api_url` to the API base prefix where `/models`
/// can be appended. The UI's "engine presets" point at the chat-completions
/// endpoint (e.g. `https://api.openai.com/v1/chat/completions`) for routing
/// LLM calls, but the `/models` listing endpoint lives on the API root
/// (`https://api.openai.com/v1/models`). Without this strip we end up requesting
/// `…/chat/completions/models`, which every provider returns as 404.
///
/// Also strips Gemini's `/openai` shim (`/v1beta/openai/chat/completions`) so
/// the listing call hits the native `/v1beta/models?key=…` route.
fn strip_to_api_base(url: &str) -> String {
    let mut s = url.trim_end_matches('/').to_string();

    // Drop the chat-completion suffix in any form.
    // Order matters: longer/more-specific suffixes must come FIRST so that
    // `/v1/messages` strips to `https://api.anthropic.com` (caller appends
    // `/v1/models`), not to `https://api.anthropic.com/v1` (would double up).
    for suffix in [
        "/v1/messages", // Anthropic native (long form)
        "/chat/completions",
        "/messages",  // Anthropic native (short form, defensive)
        "/api/chat",  // Ollama native chat endpoint (cloud + local)
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim_end_matches('/').to_string();
            break;
        }
    }

    // Gemini OpenAI-compat: drop the trailing /openai so we can hit the native
    // /v1beta/models?key=… listing endpoint.
    if let Some(stripped) = s.strip_suffix("/openai") {
        s = stripped.trim_end_matches('/').to_string();
    }

    s
}

/// CORE-FIX: extract just `scheme://host[:port]` from any URL the user pasted.
/// Ollama's tags endpoint is always at `/api/tags` relative to the host, so
/// we ignore whatever path the user typed (e.g. `https://ollama.com/api/chat`
/// or `https://ollama.com/v1/chat/completions`) and rebuild from the origin.
/// Falls back to a trimmed-trailing-slash string if URL parsing fails.
fn url_origin(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        let scheme = parsed.scheme();
        if let Some(host) = parsed.host_str() {
            if let Some(port) = parsed.port() {
                return format!("{}://{}:{}", scheme, host, port);
            }
            return format!("{}://{}", scheme, host);
        }
    }
    url.trim_end_matches('/').to_string()
}

/// OpenAI-compatible `GET {base}/models` with `Authorization: Bearer <key>`.
/// Response shape: `{ "data": [ { "id": "...", ... }, ... ] }`.
async fn fetch_openai_compatible(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> anyhow::Result<Vec<DiscoveredModel>> {
    // Tolerate trailing slash + chat-completion suffix on user-provided URLs.
    let base = strip_to_api_base(base_url);
    let url = format!("{}/models", base);
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let data = resp["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("response has no `data` array: {}", resp))?;

    Ok(data
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            // Some providers (OpenAI) also list embedding/audio/etc. models in /models.
            // Filter to entries that look like chat models — heuristic: skip ones
            // that obviously aren't (`whisper`, `tts`, `dall-e`, `embedding`).
            let lower = id.to_lowercase();
            if lower.contains("whisper")
                || lower.contains("tts")
                || lower.contains("dall-e")
                || lower.contains("embedding")
                || lower.contains("moderation")
            {
                return None;
            }
            let display_name = m["name"]
                .as_str()
                .or_else(|| m["display_name"].as_str())
                .unwrap_or(&id)
                .to_string();
            let context_window = m["context_length"]
                .as_u64()
                .or_else(|| m["context_window"].as_u64())
                .map(|v| v as u32);
            let supports_tools = m["supported_parameters"]
                .as_array()
                .map(|arr| arr.iter().any(|v| v.as_str() == Some("tools")));
            Some(DiscoveredModel {
                model_id: id,
                display_name,
                context_window,
                supports_tools,
            })
        })
        .collect())
}

/// Google Generative Language API. The native endpoint is
/// `https://generativelanguage.googleapis.com/v1beta/models?key=<KEY>` and
/// the response uses `models[].name = "models/gemini-x"`. We strip the
/// `models/` prefix because that's the form the chat-completions endpoint
/// expects everywhere else in Aegis.
async fn fetch_gemini(
    client: &reqwest::Client,
    api_url: Option<&str>,
    api_key: &str,
) -> anyhow::Result<Vec<DiscoveredModel>> {
    // Normalise whatever the caller passed (often the chat-completions URL
    // like `/v1beta/openai/chat/completions`) into the native API base
    // (`/v1beta`) where /models lives.
    let base = match api_url {
        Some(u) => strip_to_api_base(u),
        None => "https://generativelanguage.googleapis.com/v1beta".to_string(),
    };
    let url = format!("{}/models?key={}", base, api_key);
    let resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let models = resp["models"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("gemini response has no `models` array"))?;

    Ok(models
        .iter()
        .filter_map(|m| {
            // name looks like "models/gemini-2.5-flash"
            let full = m["name"].as_str()?;
            let id = full.strip_prefix("models/").unwrap_or(full).to_string();
            // Keep only models that actually support text generation.
            let supports_generate = m["supportedGenerationMethods"]
                .as_array()
                .map(|arr| arr.iter().any(|v| v.as_str() == Some("generateContent")))
                .unwrap_or(true);
            if !supports_generate {
                return None;
            }
            let display_name = m["displayName"].as_str().unwrap_or(&id).to_string();
            let context_window = m["inputTokenLimit"].as_u64().map(|v| v as u32);
            Some(DiscoveredModel {
                model_id: id,
                display_name,
                context_window,
                // Gemini doesn't advertise tool support per-model in /models,
                // but every modern Gemini supports function calling.
                supports_tools: None,
            })
        })
        .collect())
}

/// Anthropic native `/v1/models`. Auth uses `x-api-key` header plus the
/// required `anthropic-version` header.
async fn fetch_anthropic(
    client: &reqwest::Client,
    api_url: Option<&str>,
    api_key: &str,
) -> anyhow::Result<Vec<DiscoveredModel>> {
    // Strip /v1/messages (the chat endpoint) so the user can paste their
    // chat URL into the modal and we still hit /v1/models.
    let base = match api_url {
        Some(u) => strip_to_api_base(u),
        None => "https://api.anthropic.com".to_string(),
    };
    let url = format!("{}/v1/models", base);
    let resp = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let data = resp["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("anthropic response has no `data` array"))?;

    Ok(data
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let display_name = m["display_name"].as_str().unwrap_or(&id).to_string();
            // Anthropic doesn't include context_length in /v1/models response.
            Some(DiscoveredModel {
                model_id: id,
                display_name,
                context_window: None,
                supports_tools: Some(true), // every shipping Claude supports tools
            })
        })
        .collect())
}

/// Ollama. Local is `http://<host>:11434/api/tags`, cloud is
/// `https://ollama.com/api/tags` with Bearer auth. Tag response:
/// `{ "models": [ { "name": "llama3.1:8b", ... } ] }`.
async fn fetch_ollama(
    client: &reqwest::Client,
    api_url: Option<&str>,
    api_key: &str,
    is_cloud: bool,
) -> anyhow::Result<Vec<DiscoveredModel>> {
    // CORE-FIX: Ollama's `/api/tags` endpoint lives at the host root. The
    // UI usually configures the chat URL (e.g. `https://ollama.com/api/chat`)
    // and we used to append `/api/tags` to it verbatim, producing the
    // infamous 404 at `https://ollama.com/api/chat/api/tags` (smoke test
    // reproducer). Reduce whatever the user gave us to scheme+host+port so
    // discovery works no matter which endpoint they pasted.
    let base = api_url
        .map(url_origin)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if is_cloud {
                "https://ollama.com".to_string()
            } else {
                "http://localhost:11434".to_string()
            }
        });
    let url = format!("{}/api/tags", base.trim_end_matches('/'));

    let mut req = client.get(&url);
    if is_cloud && !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = req
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    let models = resp["models"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("ollama response has no `models` array"))?;

    Ok(models
        .iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_string();
            let context_window = m["details"]["parameter_size"]
                .as_str()
                .and_then(|_| m["details"]["context_length"].as_u64())
                .map(|v| v as u32);
            Some(DiscoveredModel {
                model_id: name.clone(),
                display_name: name,
                context_window,
                supports_tools: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovered_model_serialises_camel_friendly() {
        // The struct is what the HTTP layer will JSON back to the UI.
        // Just confirm the field names match what the front expects.
        let m = DiscoveredModel {
            model_id: "gemini-2.5-flash".into(),
            display_name: "Gemini 2.5 Flash".into(),
            context_window: Some(1_048_576),
            supports_tools: Some(true),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["model_id"], "gemini-2.5-flash");
        assert_eq!(v["display_name"], "Gemini 2.5 Flash");
        assert_eq!(v["context_window"], 1_048_576);
        assert_eq!(v["supports_tools"], true);
    }

    #[tokio::test]
    async fn unsupported_provider_returns_empty_not_error() {
        // We don't know a /models endpoint for "qwen" or "custom" — the
        // caller is expected to fall back to user-supplied active_models,
        // so this MUST be Ok(empty) rather than Err.
        let v = fetch_provider_models("qwen", None, "fake-key").await;
        assert!(v.is_ok(), "unsupported provider should not error");
        assert!(v.unwrap().is_empty());
    }

    /// CORE-FIX: regression — UI passes the chat-completions URL from the
    /// engine presets, but the /models endpoint lives on the API root.
    /// Without strip_to_api_base, every discovery hit a 404 like
    /// `…/chat/completions/models?key=…`.
    #[test]
    fn strip_to_api_base_normalises_every_provider_preset() {
        // The exact strings from shell/ui/src/constants/enginePresets.ts
        // plus a few common variants the user might paste manually.
        let cases = &[
            (
                "https://api.openai.com/v1/chat/completions",
                "https://api.openai.com/v1",
            ),
            (
                "https://api.groq.com/openai/v1/chat/completions",
                "https://api.groq.com/openai/v1",
            ),
            (
                "https://openrouter.ai/api/v1/chat/completions",
                "https://openrouter.ai/api/v1",
            ),
            (
                "https://api.mistral.ai/v1/chat/completions",
                "https://api.mistral.ai/v1",
            ),
            (
                "https://api.deepseek.com/v1/chat/completions",
                "https://api.deepseek.com/v1",
            ),
            (
                "https://api.x.ai/v1/chat/completions",
                "https://api.x.ai/v1",
            ),
            (
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                "https://generativelanguage.googleapis.com/v1beta",
            ),
            // Anthropic native — strips /v1/messages so /v1/models lands clean.
            (
                "https://api.anthropic.com/v1/messages",
                "https://api.anthropic.com",
            ),
            // Ollama Cloud native: strip /api/chat so the caller can rebuild
            // `…/api/tags` for discovery.
            (
                "https://ollama.com/api/chat",
                "https://ollama.com",
            ),
            // Already-clean base URLs should be left alone.
            ("https://api.openai.com/v1", "https://api.openai.com/v1"),
            ("https://api.openai.com/v1/", "https://api.openai.com/v1"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                strip_to_api_base(input),
                *expected,
                "strip_to_api_base({:?})",
                input
            );
        }
    }

    /// CORE-FIX: regression for the smoke-test 404 the user hit when
    /// verifying Ollama Cloud — the UI sent `https://ollama.com/api/chat`
    /// and we appended `/api/tags` to it verbatim, producing the
    /// nonsensical `https://ollama.com/api/chat/api/tags`. url_origin
    /// reduces any input to scheme+host[:port] so discovery works no
    /// matter which path the user pasted.
    #[test]
    fn url_origin_collapses_to_scheme_host_port() {
        let cases = &[
            ("https://ollama.com/api/chat", "https://ollama.com"),
            (
                "https://ollama.com/v1/chat/completions",
                "https://ollama.com",
            ),
            ("https://ollama.com/", "https://ollama.com"),
            ("https://ollama.com", "https://ollama.com"),
            ("http://localhost:11434/api/tags", "http://localhost:11434"),
            (
                "http://192.168.1.10:11434/api/chat/",
                "http://192.168.1.10:11434",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                url_origin(input),
                *expected,
                "url_origin({:?})",
                input
            );
        }
    }

    /// Anything we can't parse should pass through (trimmed) — never panic
    /// or empty-string the caller's input out from under them.
    #[test]
    fn url_origin_falls_back_for_unparseable_input() {
        assert_eq!(url_origin("not a url"), "not a url");
        assert_eq!(url_origin(""), "");
    }
}
