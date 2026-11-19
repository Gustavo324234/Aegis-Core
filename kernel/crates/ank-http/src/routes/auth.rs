use crate::{
    citadel::{hash_passphrase, CitadelError},
    error::AegisHttpError,
    rate_limiter::{AuthRateLimiter, RateLimitOutcome},
    routes::status::OptionalPeerAddr,
    state::AppState,
};
use ank_core::enclave::MasterEnclave;
use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use utoipa::ToSchema;

const MIN_PASSPHRASE_CHARS: usize = 6;
const BOOTSTRAP_RATE_BUCKET: &str = "__bootstrap_setup__";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/setup", post(setup))
        .route("/setup-token", post(setup_token))
        .route("/change_password", post(change_password))
        .route("/quick-personal-setup", post(quick_personal_setup))
}

#[derive(Deserialize, ToSchema)]
pub struct AuthRequest {
    #[schema(example = "admin")]
    pub tenant_id: String,
    #[schema(format = "password")]
    pub session_key: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AdminSetupRequest {
    #[schema(example = "admin")]
    pub username: String,
    #[schema(format = "password")]
    pub passphrase: String,
    /// CORE-371: Setup token is mandatory across all bootstrap endpoints.
    #[serde(default)]
    #[schema(example = "SETUP-XXXX-XXXX")]
    pub setup_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetupTokenRequest {
    #[schema(example = "admin")]
    pub username: String,
    #[schema(format = "password")]
    pub password: String,
    #[schema(example = "SETUP-XXXX-XXXX")]
    pub setup_token: String,
}

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    #[schema(example = "tenant_001")]
    pub tenant_id: String,
    #[schema(format = "password")]
    pub current_password: String,
    #[schema(format = "password")]
    pub new_password: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct LoginResponse {
    pub message: String,
    pub status: String,
    pub role: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SetupResponse {
    pub status: String,
    pub message: String,
    pub factory_reset_applied: bool,
}

#[derive(serde::Serialize, ToSchema)]
pub struct SetupTokenResponse {
    pub success: bool,
    pub factory_reset_applied: bool,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ChangePasswordResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize, ToSchema)]
pub struct QuickPersonalSetupRequest {
    #[schema(example = "alex")]
    pub username: String,
    #[schema(format = "password", example = "MiClaveSegura123")]
    pub passphrase: String,
    #[schema(example = "gemini")]
    pub provider: String,
    #[schema(format = "password", example = "AIzaSy...")]
    pub api_key: Option<String>,
    #[schema(example = "gemini-2.5-flash")]
    pub model_name: Option<String>,
    #[schema(example = "SETUP-XXXX-XXXX")]
    pub setup_token: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
pub struct QuickPersonalSetupResponse {
    pub status: String,
    pub tenant_id: String,
}

fn client_ip(peer_addr: Option<SocketAddr>, headers: &HeaderMap) -> IpAddr {
    let Some(addr) = peer_addr else {
        return IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    };
    let peer_ip = addr.ip();
    if !peer_ip.is_loopback() {
        return peer_ip;
    }

    if let Some(cf_ip) = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return cf_ip;
    }

    if let Some(real_ip) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return real_ip;
    }

    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        let mut last_valid: Option<IpAddr> = None;
        for part in xff.rsplit(',') {
            if let Ok(parsed) = part.trim().parse::<IpAddr>() {
                if !parsed.is_loopback() {
                    return parsed;
                }
                if last_valid.is_none() {
                    last_valid = Some(parsed);
                }
            }
        }
        if let Some(valid) = last_valid {
            return valid;
        }
    }

    peer_ip
}

fn validate_new_passphrase(raw: &str) -> Result<(), AegisHttpError> {
    if raw.trim().is_empty() || raw.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(AegisHttpError::BadRequest(format!(
            "Passphrase must be at least {} characters long",
            MIN_PASSPHRASE_CHARS
        )));
    }
    Ok(())
}

/// CORE-371: Atomic, rate-limited Master Admin bootstrap enforcing a valid `setup_token`.
async fn bootstrap_master_with_token(
    enclave: &MasterEnclave,
    rate_limiter: &AuthRateLimiter,
    ip: IpAddr,
    username: &str,
    raw_password: &str,
    setup_token: &str,
) -> Result<(), AegisHttpError> {
    match rate_limiter.check_and_record_failed(ip, BOOTSTRAP_RATE_BUCKET) {
        RateLimitOutcome::Blocked { retry_after_secs } => {
            return Err(AegisHttpError::RateLimitExceeded(retry_after_secs));
        }
        RateLimitOutcome::Allowed { .. } => {}
    }

    let trimmed_user = username.trim();
    if trimmed_user.is_empty() {
        return Err(AegisHttpError::BadRequest(
            "Username cannot be empty".to_string(),
        ));
    }
    validate_new_passphrase(raw_password)?;

    let trimmed_token = setup_token.trim();
    if trimmed_token.is_empty() {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    if enclave
        .admin_exists()
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?
    {
        return Err(AegisHttpError::Forbidden(
            "Master Admin is already initialized".to_string(),
        ));
    }

    let valid = enclave
        .validate_setup_token_only(trimmed_token)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !valid {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    let hash = hash_passphrase(raw_password);
    enclave
        .initialize_master(trimmed_user, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    enclave
        .consume_setup_token(trimmed_token)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    rate_limiter.reset(ip, BOOTSTRAP_RATE_BUCKET);
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Authentication successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 429, description = "Rate limit exceeded")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    headers: HeaderMap,
    Json(body): Json<AuthRequest>,
) -> Result<Json<LoginResponse>, AegisHttpError> {
    let ip = client_ip(peer_addr, &headers);

    match state
        .auth_rate_limiter
        .check_and_record_failed(ip, &body.tenant_id)
    {
        RateLimitOutcome::Blocked { retry_after_secs } => {
            return Err(AegisHttpError::RateLimitExceeded(retry_after_secs));
        }
        RateLimitOutcome::Allowed {
            remaining,
            reset_in_secs: _,
        } => {
            tracing::warn!(
                ip = %ip,
                tenant_id = %body.tenant_id,
                remaining_attempts = remaining,
                "Auth attempt recorded"
            );
        }
    }

    let hash = hash_passphrase(&body.session_key);
    let citadel = state.citadel.lock().await;

    let is_master = citadel
        .enclave
        .authenticate_master(&body.tenant_id, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if is_master {
        state.auth_rate_limiter.reset(ip, &body.tenant_id);
        return Ok(Json(LoginResponse {
            message: "Citadel Handshake Successful".to_string(),
            status: "authenticated".to_string(),
            role: "admin".to_string(),
        }));
    }

    match citadel
        .enclave
        .authenticate_tenant(&body.tenant_id, &hash)
        .await
    {
        Ok(true) => {
            state.auth_rate_limiter.reset(ip, &body.tenant_id);
            Ok(Json(LoginResponse {
                message: "Citadel Handshake Successful".to_string(),
                status: "authenticated".to_string(),
                role: "tenant".to_string(),
            }))
        }
        Ok(false) => Err(AegisHttpError::Citadel(CitadelError::Unauthorized)),
        Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => {
            state.auth_rate_limiter.reset(ip, &body.tenant_id);
            Ok(Json(LoginResponse {
                message: "Password rotation required".to_string(),
                status: "password_must_change".to_string(),
                role: "tenant".to_string(),
            }))
        }
        Err(_) => Err(AegisHttpError::Citadel(CitadelError::Unauthorized)),
    }
}

#[utoipa::path(
    post,
    path = "/api/auth/setup",
    tag = "auth",
    request_body = AdminSetupRequest,
    responses(
        (status = 200, description = "Master admin initialized", body = SetupResponse),
        (status = 401, description = "Missing, invalid, or expired setup token"),
        (status = 403, description = "Already initialized"),
        (status = 429, description = "Rate limit exceeded")
    )
)]
pub async fn setup(
    State(state): State<AppState>,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    headers: HeaderMap,
    Json(body): Json<AdminSetupRequest>,
) -> Result<Json<SetupResponse>, AegisHttpError> {
    let ip = client_ip(peer_addr, &headers);
    let citadel = state.citadel.lock().await;
    bootstrap_master_with_token(
        &citadel.enclave,
        &state.auth_rate_limiter,
        ip,
        &body.username,
        &body.passphrase,
        &body.setup_token,
    )
    .await?;

    Ok(Json(SetupResponse {
        status: "success".to_string(),
        message: "Master Admin initialized".to_string(),
        factory_reset_applied: true,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/setup-token",
    tag = "auth",
    request_body = SetupTokenRequest,
    responses(
        (status = 200, description = "Setup completed", body = SetupTokenResponse),
        (status = 401, description = "Invalid or expired setup token"),
        (status = 403, description = "Already initialized"),
        (status = 429, description = "Rate limit exceeded")
    )
)]
pub async fn setup_token(
    State(state): State<AppState>,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    headers: HeaderMap,
    Json(body): Json<SetupTokenRequest>,
) -> Result<Json<SetupTokenResponse>, AegisHttpError> {
    let ip = client_ip(peer_addr, &headers);
    let citadel = state.citadel.lock().await;
    bootstrap_master_with_token(
        &citadel.enclave,
        &state.auth_rate_limiter,
        ip,
        &body.username,
        &body.password,
        &body.setup_token,
    )
    .await?;

    Ok(Json(SetupTokenResponse {
        success: true,
        factory_reset_applied: true,
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/change_password",
    tag = "auth",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed", body = ChangePasswordResponse),
        (status = 400, description = "Weak new password"),
        (status = 401, description = "Invalid current password"),
        (status = 429, description = "Rate limit exceeded")
    )
)]
pub async fn change_password(
    State(state): State<AppState>,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, AegisHttpError> {
    let ip = client_ip(peer_addr, &headers);

    match state
        .auth_rate_limiter
        .check_and_record_failed(ip, &body.tenant_id)
    {
        RateLimitOutcome::Blocked { retry_after_secs } => {
            return Err(AegisHttpError::RateLimitExceeded(retry_after_secs));
        }
        RateLimitOutcome::Allowed { .. } => {}
    }

    validate_new_passphrase(&body.new_password)?;
    if body.new_password == body.current_password {
        return Err(AegisHttpError::BadRequest(
            "New password must differ from current password".to_string(),
        ));
    }

    let current_hash = hash_passphrase(&body.current_password);
    let new_hash = hash_passphrase(&body.new_password);

    let citadel = state.citadel.lock().await;

    let is_master = citadel
        .enclave
        .authenticate_master(&body.tenant_id, &current_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if is_master {
        citadel
            .enclave
            .reset_master_password(&body.tenant_id, &new_hash)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
        state.auth_rate_limiter.reset(ip, &body.tenant_id);
        return Ok(Json(ChangePasswordResponse {
            success: true,
            message: "Password updated successfully".to_string(),
        }));
    }

    // Verificar la contraseña actual usando authenticate_tenant
    // Ignoramos PASSWORD_MUST_CHANGE — el usuario está intentando cambiarla ahora mismo
    let current_valid = match citadel
        .enclave
        .authenticate_tenant(&body.tenant_id, &current_hash)
        .await
    {
        Ok(valid) => valid,
        Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => true,
        Err(_) => false,
    };

    if !current_valid {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    state.auth_rate_limiter.reset(ip, &body.tenant_id);

    Ok(Json(ChangePasswordResponse {
        success: true,
        message: "Password updated successfully".to_string(),
    }))
}

#[allow(clippy::too_many_arguments)]
async fn execute_quick_personal_bootstrap(
    enclave: &MasterEnclave,
    rate_limiter: &AuthRateLimiter,
    peer_addr: Option<SocketAddr>,
    headers: &HeaderMap,
    raw_username: &str,
    raw_passphrase: &str,
    setup_token: Option<&str>,
) -> Result<(String, IpAddr), AegisHttpError> {
    let ip = client_ip(peer_addr, headers);

    match rate_limiter.check_and_record_failed(ip, BOOTSTRAP_RATE_BUCKET) {
        RateLimitOutcome::Blocked { retry_after_secs } => {
            return Err(AegisHttpError::RateLimitExceeded(retry_after_secs));
        }
        RateLimitOutcome::Allowed { .. } => {}
    }

    let username = raw_username.trim().to_lowercase();
    if username.is_empty() {
        return Err(AegisHttpError::BadRequest(
            "Username cannot be empty".to_string(),
        ));
    }

    if raw_passphrase.chars().count() < 8 {
        return Err(AegisHttpError::BadRequest(
            "Passphrase must be at least 8 characters long".to_string(),
        ));
    }

    // CORE-398: Siempre exigir setup_token válido (incluso en loopback y con proxy).
    let tok = setup_token
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(AegisHttpError::Citadel(CitadelError::Unauthorized))?;

    if enclave
        .admin_exists()
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?
    {
        return Err(AegisHttpError::Forbidden(
            "Master Admin is already initialized".to_string(),
        ));
    }

    let valid = enclave
        .validate_setup_token_only(tok)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    if !valid {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    let hash = hash_passphrase(raw_passphrase);

    enclave
        .initialize_master("admin", &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    enclave
        .create_personal_tenant(&username, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    enclave
        .consume_setup_token(tok)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    rate_limiter.reset(ip, BOOTSTRAP_RATE_BUCKET);
    Ok((username, ip))
}

#[utoipa::path(
    post,
    path = "/api/auth/quick-personal-setup",
    tag = "auth",
    request_body = QuickPersonalSetupRequest,
    responses(
        (status = 200, description = "Quick personal setup successful", body = QuickPersonalSetupResponse),
        (status = 400, description = "Invalid request or weak passphrase"),
        (status = 401, description = "Unauthorized: setup token required"),
        (status = 403, description = "Admin already exists"),
        (status = 429, description = "Rate limit exceeded")
    )
)]
pub async fn quick_personal_setup(
    State(state): State<AppState>,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    headers: HeaderMap,
    Json(body): Json<QuickPersonalSetupRequest>,
) -> Result<Json<QuickPersonalSetupResponse>, AegisHttpError> {
    let (username, _ip) = {
        let citadel = state.citadel.lock().await;
        execute_quick_personal_bootstrap(
            &citadel.enclave,
            &state.auth_rate_limiter,
            peer_addr,
            &headers,
            &body.username,
            &body.passphrase,
            body.setup_token.as_deref(),
        )
        .await?
    };

    let canonical_provider = ank_core::router::normalize_provider_id(&body.provider);
    let has_key = body
        .api_key
        .as_deref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);

    if has_key || canonical_provider == "ollama" {
        let (api_url, default_model): (String, String) = match canonical_provider.as_str() {
            "ollama" => (
                "http://localhost:11434/v1/chat/completions".to_string(),
                "llama3.2".to_string(),
            ),
            "gemini" => (
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                    .to_string(),
                "gemini-2.5-flash".to_string(),
            ),
            "openrouter" => (
                "https://openrouter.ai/api/v1/chat/completions".to_string(),
                "openrouter/free".to_string(),
            ),
            "groq" => (
                "https://api.groq.com/openai/v1/chat/completions".to_string(),
                "groq/llama-3.3-70b-versatile".to_string(),
            ),
            "anthropic" => (
                "https://openrouter.ai/api/v1/chat/completions".to_string(),
                "anthropic/claude-sonnet-4-6".to_string(),
            ),
            _ => (
                "https://api.openai.com/v1/chat/completions".to_string(),
                "openai/gpt-4o-mini".to_string(),
            ),
        };

        let raw_model = body.model_name.unwrap_or(default_model);
        let model = match raw_model.trim() {
            "gemini-1.5-flash" | "gemini-1.5-pro" => "gemini-2.5-flash".to_string(),
            other => other.to_string(),
        };
        let key_str = body
            .api_key
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| "ollama".to_string());

        let bare_hal_model = model
            .strip_prefix("openai/")
            .or_else(|| model.strip_prefix("groq/"))
            .unwrap_or(&model)
            .to_string();

        state
            .hal
            .update_cloud_credentials(api_url.clone(), bare_hal_model, key_str.clone())
            .await;

        let config_to_save = serde_json::json!({
            "configured": true,
            "provider": canonical_provider,
            "api_url": api_url,
            "model_name": model,
            "configured_by": username,
        });

        if let Ok(config_json) = serde_json::to_string_pretty(&config_to_save) {
            let config_path = state.config.data_dir.join("engine_config.json");
            let _ = tokio::fs::write(&config_path, config_json).await;
        }

        let entry = ank_core::router::key_pool::ApiKeyEntry {
            key_id: uuid::Uuid::new_v4().to_string(),
            provider: canonical_provider,
            api_key: key_str,
            api_url: Some(api_url),
            label: Some("Quick Setup Key".to_string()),
            is_active: true,
            rate_limited_until: None,
            active_models: Some(vec![model]),
            is_free_tier: false,
        };

        let router = state.router.read().await;
        let _ = router.add_tenant_key(&username, entry.clone()).await;
        let _ = router.add_global_key(entry).await;
        router.invalidate_all_sticky().await;
    }

    Ok(Json(QuickPersonalSetupResponse {
        status: "READY".to_string(),
        tenant_id: username,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bootstrap_rejects_missing_or_invalid_setup_token() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;
        enclave.store_setup_token("SETUP-VALID-TOKEN", 15).await?;
        let limiter = AuthRateLimiter::new(crate::rate_limiter::RateLimitConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));

        // Empty setup_token (legacy /api/auth/setup bypass attempt) must fail with 401
        let empty_err =
            bootstrap_master_with_token(&enclave, &limiter, ip, "admin", "strongpass123", "").await;
        assert!(matches!(
            empty_err,
            Err(AegisHttpError::Citadel(CitadelError::Unauthorized))
        ));
        assert!(!enclave.admin_exists().await?);

        // Wrong setup_token must fail with 401
        let wrong_err = bootstrap_master_with_token(
            &enclave,
            &limiter,
            ip,
            "admin",
            "strongpass123",
            "SETUP-WRONG",
        )
        .await;
        assert!(matches!(
            wrong_err,
            Err(AegisHttpError::Citadel(CitadelError::Unauthorized))
        ));
        assert!(!enclave.admin_exists().await?);

        // Valid setup_token initializes Master Admin and consumes the token
        bootstrap_master_with_token(
            &enclave,
            &limiter,
            ip,
            "admin",
            "strongpass123",
            "SETUP-VALID-TOKEN",
        )
        .await?;
        assert!(enclave.admin_exists().await?);
        assert!(
            enclave
                .authenticate_master("admin", &hash_passphrase("strongpass123"))
                .await?
        );

        // Second bootstrap attempt must fail with 403 Forbidden
        let replay_err = bootstrap_master_with_token(
            &enclave,
            &limiter,
            ip,
            "attacker",
            "strongpass123",
            "SETUP-VALID-TOKEN",
        )
        .await;
        assert!(matches!(replay_err, Err(AegisHttpError::Forbidden(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_bootstrap_rate_limits_token_bruteforce() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;
        enclave.store_setup_token("SETUP-REAL-SECRET", 15).await?;
        let limiter = AuthRateLimiter::new(crate::rate_limiter::RateLimitConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42));

        for _ in 0..5 {
            let _ = bootstrap_master_with_token(
                &enclave,
                &limiter,
                ip,
                "admin",
                "strongpass123",
                "SETUP-GUESS",
            )
            .await;
        }

        // 6th attempt must be blocked with 429 even if token were valid
        let blocked = bootstrap_master_with_token(
            &enclave,
            &limiter,
            ip,
            "admin",
            "strongpass123",
            "SETUP-REAL-SECRET",
        )
        .await;
        assert!(matches!(blocked, Err(AegisHttpError::RateLimitExceeded(_))));
        assert!(!enclave.admin_exists().await?);
        Ok(())
    }

    #[tokio::test]
    async fn test_quick_personal_setup_flow() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;
        assert!(!enclave.admin_exists().await?);

        let hash = hash_passphrase("mypassword123");
        enclave.initialize_master("admin", &hash).await?;
        enclave.create_personal_tenant("alex", &hash).await?;

        assert!(enclave.admin_exists().await?);
        assert!(enclave.authenticate_master("admin", &hash).await?);
        // Tenant is authenticated and does NOT require password change
        assert!(enclave.authenticate_tenant("alex", &hash).await?);

        Ok(())
    }

    #[test]
    fn test_client_ip_resolves_proxied_and_blocks_spoofing() -> anyhow::Result<()> {
        use axum::http::HeaderValue;

        let empty_headers = HeaderMap::new();
        assert_eq!(
            client_ip(None, &empty_headers),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );

        let loopback_peer = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 40000));
        let public_peer = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 10)),
            40000,
        ));

        let mut cf_headers = HeaderMap::new();
        cf_headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.55"));
        assert_eq!(
            client_ip(loopback_peer, &cf_headers),
            "203.0.113.55".parse::<IpAddr>()?
        );

        let mut xff_headers = HeaderMap::new();
        xff_headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("10.0.0.1, 203.0.113.77"),
        );
        assert_eq!(
            client_ip(loopback_peer, &xff_headers),
            "203.0.113.77".parse::<IpAddr>()?
        );

        // Non-loopback peers cannot spoof client IP via proxy headers
        assert_eq!(
            client_ip(public_peer, &xff_headers),
            "198.51.100.10".parse::<IpAddr>()?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_quick_personal_setup_requires_setup_token_even_on_loopback() -> anyhow::Result<()>
    {
        use axum::http::HeaderValue;

        let enclave = MasterEnclave::new_in_memory().await?;
        enclave.store_setup_token("SETUP-QUICK-VALID", 15).await?;
        let limiter = AuthRateLimiter::new(crate::rate_limiter::RateLimitConfig::default());
        let loopback = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 50000));
        let empty = HeaderMap::new();

        // Direct loopback without setup_token must fail with 401
        let no_tok = execute_quick_personal_bootstrap(
            &enclave,
            &limiter,
            loopback,
            &empty,
            "alex",
            "strongpass123",
            None,
        )
        .await;
        assert!(matches!(
            no_tok,
            Err(AegisHttpError::Citadel(CitadelError::Unauthorized))
        ));
        assert!(!enclave.admin_exists().await?);

        // Loopback with proxy headers without setup_token must also fail with 401
        // and account rate-limit against the forwarded IP, not 127.0.0.1.
        let mut proxy_hdrs = HeaderMap::new();
        proxy_hdrs.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.88"));
        for _ in 0..5 {
            let err = execute_quick_personal_bootstrap(
                &enclave,
                &limiter,
                loopback,
                &proxy_hdrs,
                "alex",
                "strongpass123",
                Some(""),
            )
            .await;
            assert!(matches!(
                err,
                Err(AegisHttpError::Citadel(CitadelError::Unauthorized))
            ));
        }
        let proxied_blocked = execute_quick_personal_bootstrap(
            &enclave,
            &limiter,
            loopback,
            &proxy_hdrs,
            "alex",
            "strongpass123",
            Some("SETUP-QUICK-VALID"),
        )
        .await;
        assert!(matches!(
            proxied_blocked,
            Err(AegisHttpError::RateLimitExceeded(_))
        ));

        // Direct loopback with valid setup_token succeeds (different rate bucket) and consumes token
        let (created_user, used_ip) = execute_quick_personal_bootstrap(
            &enclave,
            &limiter,
            loopback,
            &empty,
            "Alex",
            "strongpass123",
            Some("SETUP-QUICK-VALID"),
        )
        .await?;
        assert_eq!(created_user, "alex");
        assert_eq!(used_ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(enclave.admin_exists().await?);
        let hash = hash_passphrase("strongpass123");
        assert!(enclave.authenticate_master("admin", &hash).await?);
        assert!(enclave.authenticate_tenant("alex", &hash).await?);

        // Reusing consumed token fails with 403 Admin already initialized
        let replay = execute_quick_personal_bootstrap(
            &enclave,
            &limiter,
            loopback,
            &empty,
            "bob",
            "strongpass123",
            Some("SETUP-QUICK-VALID"),
        )
        .await;
        assert!(matches!(replay, Err(AegisHttpError::Forbidden(_))));
        Ok(())
    }
}
