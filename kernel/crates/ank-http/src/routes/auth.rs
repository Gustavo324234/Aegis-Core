use crate::{
    citadel::hash_passphrase, error::AegisHttpError, rate_limiter::RateLimitOutcome,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use utoipa::ToSchema;

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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<LoginResponse>, AegisHttpError> {
    let ip = addr.ip();

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
        Ok(false) => Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        )),
        Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => {
            state.auth_rate_limiter.reset(ip, &body.tenant_id);
            Ok(Json(LoginResponse {
                message: "Password rotation required".to_string(),
                status: "password_must_change".to_string(),
                role: "tenant".to_string(),
            }))
        }
        Err(_) => Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/auth/setup",
    tag = "auth",
    request_body = AdminSetupRequest,
    responses(
        (status = 200, description = "Master admin initialized", body = SetupResponse),
        (status = 500, description = "Kernel error")
    )
)]
pub async fn setup(
    State(state): State<AppState>,
    Json(body): Json<AdminSetupRequest>,
) -> Result<Json<SetupResponse>, AegisHttpError> {
    let hash = hash_passphrase(&body.passphrase);
    let citadel = state.citadel.lock().await;
    citadel
        .enclave
        .initialize_master(&body.username, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

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
        (status = 400, description = "Invalid or expired setup token"),
        (status = 500, description = "Kernel error")
    )
)]
pub async fn setup_token(
    State(state): State<AppState>,
    Json(body): Json<SetupTokenRequest>,
) -> Result<Json<SetupTokenResponse>, AegisHttpError> {
    let hash = hash_passphrase(&body.password);

    {
        let citadel = state.citadel.lock().await;
        let valid = citadel
            .enclave
            .validate_setup_token_only(&body.setup_token)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

        if !valid {
            return Err(AegisHttpError::Kernel(
                "Invalid or expired setup token".to_string(),
            ));
        }
    }

    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .initialize_master(&body.username, &hash)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    }

    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .consume_setup_token(&body.setup_token)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    }

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
        (status = 401, description = "Invalid current password"),
        (status = 500, description = "Kernel error")
    )
)]
pub async fn change_password(
    State(state): State<AppState>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, AegisHttpError> {
    let current_hash = hash_passphrase(&body.current_password);
    let new_hash = hash_passphrase(&body.new_password);

    let citadel = state.citadel.lock().await;

    // Verificar la contraseña actual usando authenticate_tenant (ya implementado en enclave)
    // Ignoramos PASSWORD_MUST_CHANGE — el usuario está intentando cambiarlo ahora mismo
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
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(ChangePasswordResponse {
        success: true,
        message: "Password updated successfully".to_string(),
    }))
}
