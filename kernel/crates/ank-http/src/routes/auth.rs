use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::{
    citadel::hash_passphrase,
    error::AegisHttpError,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/setup", post(setup))
        .route("/setup-token", post(setup_token))
}

#[derive(Deserialize)]
pub struct AuthRequest {
    pub tenant_id: String,
    pub session_key: String,
}

#[derive(Deserialize)]
pub struct AdminSetupRequest {
    pub username: String,
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct SetupTokenRequest {
    pub username: String,
    pub password: String,
    pub setup_token: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.session_key);
    state
        .citadel
        .lock()
        .await
        .enclave
        .authenticate_tenant(&body.tenant_id, &hash)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("PASSWORD_MUST_CHANGE") {
               AegisHttpError::Citadel(crate::citadel::CitadelError::PasswordMustChange)
            } else {
               AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized)
            }
        })?;

    Ok(Json(json!({
        "message": "Citadel Handshake Successful",
        "status": "authenticated"
    })))
}

pub async fn setup(
    State(state): State<AppState>,
    Json(body): Json<AdminSetupRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.passphrase);
    let citadel = state.citadel.lock().await;
    citadel
        .enclave
        .initialize_master(&body.username, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    
    Ok(Json(json!({
        "status": "success",
        "message": "Master Admin initialized",
        "factory_reset_applied": true
    })))
}

pub async fn setup_token(
    State(state): State<AppState>,
    Json(body): Json<SetupTokenRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.password);
    let citadel = state.citadel.lock().await;
    
    // Validar token
    let valid = citadel.enclave.validate_and_consume_setup_token(&body.setup_token)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
        
    if !valid {
        return Err(AegisHttpError::Kernel("Invalid or expired setup token".to_string()));
    }
    
    // Inicializar master
    citadel
        .enclave
        .initialize_master(&body.username, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "factory_reset_applied": true
    })))
}

