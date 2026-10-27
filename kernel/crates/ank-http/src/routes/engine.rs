use crate::{
    citadel::{hash_passphrase, CitadelAuthenticated, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use utoipa::ToSchema;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/configure", post(configure))
}

#[derive(Deserialize, ToSchema)]
pub struct EngineConfig {
    #[schema(example = "https://openrouter.ai/api/v1")]
    pub api_url: String,
    #[schema(example = "anthropic/claude-3-5-sonnet")]
    pub model_name: String,
    #[schema(format = "password")]
    pub api_key: String,
    #[serde(default = "default_provider")]
    #[schema(example = "openrouter")]
    pub provider: String,
}

fn default_provider() -> String {
    "custom".to_string()
}

#[derive(serde::Serialize, ToSchema, Debug, PartialEq, Eq)]
pub struct EngineStatusResponse {
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ConfigureResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallerScope {
    Unauthenticated,
    Tenant(String),
    Master,
}

fn is_cloud_metadata_or_special_v4(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    v4.is_unspecified()
        || v4.is_link_local() // 169.254.0.0/16 (AWS/GCP/Azure IMDS)
        || v4.is_multicast()
        || v4.is_broadcast()
        || (octets[0] == 100 && (octets[1] & 0xC0) == 64) // 100.64.0.0/10 CGNAT / Alibaba IMDS
}

fn is_disallowed_ip(ip: IpAddr, allow_local_admin: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            if is_cloud_metadata_or_special_v4(v4) {
                return true;
            }
            if !allow_local_admin && (v4.is_loopback() || v4.is_private()) {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                return is_disallowed_ip(IpAddr::V4(mapped_v4), allow_local_admin);
            }
            let segs = v6.segments();
            let is_link_local = (segs[0] & 0xffc0) == 0xfe80;
            let is_aws_imds_v6 =
                segs == Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254).segments();
            let is_ula = (segs[0] & 0xfe00) == 0xfc00;
            if v6.is_unspecified() || v6.is_multicast() || is_link_local || is_aws_imds_v6 {
                return true;
            }
            if !allow_local_admin && (v6.is_loopback() || is_ula) {
                return true;
            }
            false
        }
    }
}

/// CORE-371: Validate `api_url` for Cognitive Engine configuration.
/// - Master Admin (`allow_local_admin = true`): allows `http://` or `https://`
///   including loopback/LAN Ollama/vLLM, while blocking cloud metadata IPs and URL credentials.
/// - Regular Tenant (`allow_local_admin = false`): strictly requires `https://`
///   and rejects loopback, RFC1918 private LAN, `.local`/`.internal`, and metadata targets.
fn validate_engine_api_url(
    raw_url: &str,
    allow_local_admin: bool,
) -> Result<String, AegisHttpError> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(AegisHttpError::BadRequest(
            "api_url cannot be empty".to_string(),
        ));
    }

    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|_| AegisHttpError::BadRequest("Invalid api_url format".to_string()))?;

    let scheme = parsed.scheme();
    if allow_local_admin {
        if scheme != "http" && scheme != "https" {
            return Err(AegisHttpError::BadRequest(
                "api_url must use http:// or https://".to_string(),
            ));
        }
    } else if scheme != "https" {
        return Err(AegisHttpError::BadRequest(
            "Tenant custom api_url must use https://".to_string(),
        ));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AegisHttpError::BadRequest(
            "Credentials in api_url are not permitted".to_string(),
        ));
    }

    let host_str = parsed
        .host_str()
        .ok_or_else(|| AegisHttpError::BadRequest("api_url is missing host".to_string()))?;
    let lower_host = host_str.trim_matches(['[', ']']).to_ascii_lowercase();

    if matches!(
        lower_host.as_str(),
        "metadata.google.internal" | "metadata" | "instance-data" | "kubernetes.default"
    ) || lower_host.ends_with(".metadata.google.internal")
    {
        return Err(AegisHttpError::Forbidden(
            "Cloud metadata endpoints are forbidden".to_string(),
        ));
    }

    if !allow_local_admin
        && (lower_host == "localhost"
            || lower_host.ends_with(".localhost")
            || lower_host.ends_with(".local")
            || lower_host.ends_with(".internal"))
    {
        return Err(AegisHttpError::Forbidden(
            "Local or internal hosts are forbidden for tenant endpoints".to_string(),
        ));
    }

    if let Ok(ip) = lower_host.parse::<IpAddr>() {
        if is_disallowed_ip(ip, allow_local_admin) {
            return Err(AegisHttpError::Forbidden(
                "Target IP address is not permitted for api_url".to_string(),
            ));
        }
    }

    Ok(trimmed.to_string())
}

fn infer_tenant_provider(raw_provider: &str, api_url: &str) -> String {
    let normalized = ank_core::router::normalize_provider_id(raw_provider);
    if !normalized.is_empty() && normalized != "custom" {
        return normalized;
    }
    let lower_url = api_url.to_ascii_lowercase();
    if lower_url.contains("openrouter.ai") {
        "openrouter".to_string()
    } else if lower_url.contains("api.anthropic.com") {
        "anthropic".to_string()
    } else if lower_url.contains("generativelanguage.googleapis.com") {
        "gemini".to_string()
    } else if lower_url.contains("api.groq.com") {
        "groq".to_string()
    } else if lower_url.contains("api.deepseek.com") {
        "deepseek".to_string()
    } else if lower_url.contains("api.mistral.ai") {
        "mistral".to_string()
    } else if lower_url.contains("api.x.ai") {
        "xai".to_string()
    } else {
        "openai".to_string()
    }
}

async fn resolve_optional_caller(state: &AppState, headers: &HeaderMap) -> CallerScope {
    let Some(tenant_id) = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return CallerScope::Unauthenticated;
    };
    let Some(raw_key) = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
    else {
        return CallerScope::Unauthenticated;
    };

    let hash = hash_passphrase(raw_key);
    let citadel = state.citadel.lock().await;
    if citadel
        .enclave
        .authenticate_master(tenant_id, &hash)
        .await
        .unwrap_or(false)
    {
        return CallerScope::Master;
    }
    if citadel
        .enclave
        .authenticate_tenant(tenant_id, &hash)
        .await
        .unwrap_or(false)
    {
        return CallerScope::Tenant(tenant_id.to_string());
    }
    CallerScope::Unauthenticated
}

async fn read_global_engine_state(state: &AppState) -> EngineStatusResponse {
    let config_path = state.config.data_dir.join("engine_config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if val.get("configured").and_then(|v| v.as_bool()) == Some(true) {
                return EngineStatusResponse {
                    configured: true,
                    provider: val
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    api_url: val
                        .get("api_url")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    model_name: val
                        .get("model_name")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                };
            }
        }
    }

    let has_global_keys = {
        let router = state.router.read().await;
        !router.list_global_keys().await.is_empty()
    };

    if has_global_keys {
        return EngineStatusResponse {
            configured: true,
            provider: Some("router".to_string()),
            api_url: None,
            model_name: None,
        };
    }

    EngineStatusResponse {
        configured: false,
        provider: None,
        api_url: None,
        model_name: None,
    }
}

fn scope_status_response(
    global_state: EngineStatusResponse,
    caller: &CallerScope,
    has_tenant_keys: bool,
) -> EngineStatusResponse {
    match caller {
        CallerScope::Unauthenticated => EngineStatusResponse {
            configured: global_state.configured,
            provider: None,
            api_url: None,
            model_name: None,
        },
        CallerScope::Master => global_state,
        CallerScope::Tenant(_) => EngineStatusResponse {
            configured: global_state.configured || has_tenant_keys,
            provider: if global_state.configured {
                global_state.provider
            } else if has_tenant_keys {
                Some("router".to_string())
            } else {
                None
            },
            // Never expose internal admin `api_url` (e.g. private LAN Ollama IP) to tenants
            api_url: None,
            model_name: global_state.model_name,
        },
    }
}

#[utoipa::path(
    get,
    path = "/api/engine/status",
    tag = "engine",
    responses(
        (status = 200, description = "Engine configuration status", body = EngineStatusResponse)
    )
)]
pub async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EngineStatusResponse>, AegisHttpError> {
    let caller = resolve_optional_caller(&state, &headers).await;
    let global_state = read_global_engine_state(&state).await;

    let has_tenant_keys = match &caller {
        CallerScope::Tenant(tid) => {
            let router = state.router.read().await;
            !router.list_tenant_keys(tid).await.is_empty()
        }
        _ => false,
    };

    Ok(Json(scope_status_response(
        global_state,
        &caller,
        has_tenant_keys,
    )))
}

#[utoipa::path(
    post,
    path = "/api/engine/configure",
    tag = "engine",
    request_body = EngineConfig,
    responses(
        (status = 200, description = "Engine configured", body = ConfigureResponse),
        (status = 400, description = "Invalid engine URL"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden URL or unconfigured global engine")
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Tenant identifier"),
        ("x-citadel-key" = String, Header, description = "Session key (plaintext)")
    )
)]
pub async fn configure(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<EngineConfig>,
) -> Result<Json<ConfigureResponse>, AegisHttpError> {
    // CORE-371: Only Ring-0 Master Admin may mutate global HAL credentials and engine_config.json.
    if auth.is_master {
        let validated_url = validate_engine_api_url(&body.api_url, true)?;

        state
            .hal
            .update_cloud_credentials(
                validated_url.clone(),
                body.model_name.clone(),
                body.api_key.clone(),
            )
            .await;

        let config_to_save = json!({
            "configured": true,
            "provider": body.provider,
            "api_url": validated_url,
            "model_name": body.model_name,
            "configured_by": auth.tenant_id,
        });

        let config_json = serde_json::to_string_pretty(&config_to_save)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

        let config_path = state.config.data_dir.join("engine_config.json");
        fs::write(&config_path, config_json)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

        return Ok(Json(ConfigureResponse {
            success: true,
            message: "Global Cognitive Engine configured.".to_string(),
        }));
    }

    // Ordinary tenant (`!auth.is_master`):
    // 1. Empty `api_key` means linking to an already-configured global engine without overwriting it.
    if body.api_key.trim().is_empty() {
        let global = read_global_engine_state(&state).await;
        if !global.configured {
            return Err(AegisHttpError::Forbidden(
                "Global Cognitive Engine is not configured; provide a personal API key".to_string(),
            ));
        }
        return Ok(Json(ConfigureResponse {
            success: true,
            message: "Linked to global Cognitive Engine.".to_string(),
        }));
    }

    // 2. Non-empty `api_key`: isolate inside this tenant's KeyPool with strict SSRF validation.
    let validated_url = validate_engine_api_url(&body.api_url, false)?;
    let provider = infer_tenant_provider(&body.provider, &validated_url);
    let active_models = {
        let trimmed_model = body.model_name.trim();
        if trimmed_model.is_empty() {
            None
        } else {
            Some(vec![trimmed_model.to_string()])
        }
    };

    let entry = ank_core::router::key_pool::ApiKeyEntry {
        key_id: uuid::Uuid::new_v4().to_string(),
        provider,
        api_key: body.api_key.trim().to_string(),
        api_url: Some(validated_url),
        label: Some("Onboarding Key".to_string()),
        is_active: true,
        rate_limited_until: None,
        active_models,
        is_free_tier: false,
    };

    let router = state.router.read().await;
    router
        .add_tenant_key(&auth.tenant_id, entry)
        .await
        .map_err(AegisHttpError::Internal)?;
    router.invalidate_sticky(&auth.tenant_id).await;

    Ok(Json(ConfigureResponse {
        success: true,
        message: "Tenant Cognitive Engine key configured in isolated KeyPool.".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct HwProfileRequest {
    pub admin_tenant_id: String,
    pub session_key: String,
    pub profile: String,
}

pub async fn set_hw_profile(
    State(state): State<AppState>,
    Json(body): Json<HwProfileRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = crate::citadel::hash_passphrase(&body.session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(AegisHttpError::Internal)?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    if !["1", "2", "3"].contains(&body.profile.as_str()) {
        return Err(AegisHttpError::BadRequest(
            "Invalid profile. Use 1, 2 or 3.".into(),
        ));
    }

    std::env::set_var("HW_PROFILE", &body.profile);

    Ok(Json(json!({
        "success": true,
        "profile": body.profile
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_engine_api_url_blocks_ssrf_and_enforces_tenant_https() {
        // Public HTTPS allowed for both tenant and master
        assert!(validate_engine_api_url("https://openrouter.ai/api/v1", false).is_ok());
        assert!(validate_engine_api_url("https://api.openai.com/v1", true).is_ok());

        // Plain HTTP rejected for tenants, allowed on loopback/LAN for master admin (local Ollama)
        assert!(validate_engine_api_url("http://openrouter.ai/api/v1", false).is_err());
        assert!(validate_engine_api_url("http://127.0.0.1:11434/v1", true).is_ok());
        assert!(validate_engine_api_url("https://127.0.0.1:11434/v1", false).is_err());
        assert!(validate_engine_api_url("https://192.168.1.20:11434/v1", false).is_err());
        assert!(validate_engine_api_url("https://localhost:11434/v1", false).is_err());

        // Cloud metadata IPs blocked even for master admin
        assert!(validate_engine_api_url("http://169.254.169.254/latest/meta-data/", true).is_err());
        assert!(validate_engine_api_url("http://[fd00:ec2::254]/latest/meta-data/", true).is_err());
        assert!(validate_engine_api_url(
            "http://metadata.google.internal/computeMetadata/v1/",
            true
        )
        .is_err());

        // Embedded credentials rejected
        assert!(validate_engine_api_url("https://user:pass@openrouter.ai/api/v1", true).is_err());
    }

    #[test]
    fn test_scope_status_response_redacts_internal_urls() {
        let global = EngineStatusResponse {
            configured: true,
            provider: Some("ollama".to_string()),
            api_url: Some("http://192.168.10.50:11434/v1".to_string()),
            model_name: Some("qwen2.5-coder:14b".to_string()),
        };

        let unauth = scope_status_response(
            EngineStatusResponse {
                configured: global.configured,
                provider: global.provider.clone(),
                api_url: global.api_url.clone(),
                model_name: global.model_name.clone(),
            },
            &CallerScope::Unauthenticated,
            false,
        );
        assert_eq!(
            unauth,
            EngineStatusResponse {
                configured: true,
                provider: None,
                api_url: None,
                model_name: None,
            }
        );

        let tenant_scoped = scope_status_response(
            EngineStatusResponse {
                configured: global.configured,
                provider: global.provider.clone(),
                api_url: global.api_url.clone(),
                model_name: global.model_name.clone(),
            },
            &CallerScope::Tenant("alice".to_string()),
            false,
        );
        assert!(tenant_scoped.configured);
        assert_eq!(tenant_scoped.provider.as_deref(), Some("ollama"));
        assert!(tenant_scoped.api_url.is_none());

        let byok_tenant = scope_status_response(
            EngineStatusResponse {
                configured: false,
                provider: None,
                api_url: None,
                model_name: None,
            },
            &CallerScope::Tenant("bob".to_string()),
            true,
        );
        assert!(byok_tenant.configured);
        assert_eq!(byok_tenant.provider.as_deref(), Some("router"));
    }
}
