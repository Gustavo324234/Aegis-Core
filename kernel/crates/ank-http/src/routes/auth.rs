use crate::{
    citadel::{hash_passphrase, CitadelError},
    error::AegisHttpError,
    rate_limiter::{AuthRateLimiter, RateLimitOutcome},
    state::AppState,
};
use ank_core::enclave::MasterEnclave;
use axum::{
    extract::{ConnectInfo, State},
    routing::post,
    Json, Router,
};
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

fn client_ip(maybe_addr: Option<ConnectInfo<SocketAddr>>) -> IpAddr {
    maybe_addr
        .map(|ConnectInfo(addr)| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
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
    maybe_addr: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<LoginResponse>, AegisHttpError> {
    let ip = client_ip(maybe_addr);

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
    maybe_addr: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<AdminSetupRequest>,
) -> Result<Json<SetupResponse>, AegisHttpError> {
    let ip = client_ip(maybe_addr);
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
    maybe_addr: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<SetupTokenRequest>,
) -> Result<Json<SetupTokenResponse>, AegisHttpError> {
    let ip = client_ip(maybe_addr);
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
    maybe_addr: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, AegisHttpError> {
    let ip = client_ip(maybe_addr);

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
}
