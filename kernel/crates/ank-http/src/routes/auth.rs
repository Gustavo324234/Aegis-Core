use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

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
    let citadel = state.citadel.lock().await;

    // Master Admin check primero
    let is_master = citadel
        .enclave
        .authenticate_master(&body.tenant_id, &hash)
        .await
        .unwrap_or(false);

    if is_master {
        return Ok(Json(json!({
            "message": "Citadel Handshake Successful",
            "status": "authenticated",
            "role": "admin"
        })));
    }

    // Tenant check — PASSWORD_MUST_CHANGE se retorna como 200 con status especial,
    // no como error 403, para que el frontend pueda distinguirlo del 401.
    match citadel
        .enclave
        .authenticate_tenant(&body.tenant_id, &hash)
        .await
    {
        Ok(true) => Ok(Json(json!({
            "message": "Citadel Handshake Successful",
            "status": "authenticated",
            "role": "tenant"
        }))),
        Ok(false) => Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        )),
        Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => {
            // Credenciales correctas pero requiere cambio de contraseña.
            // Retornar 200 con status distinguible para que el frontend redirija
            // al flujo de cambio de contraseña en lugar de mostrar "credenciales incorrectas".
            Ok(Json(json!({
                "message": "Password rotation required",
                "status": "password_must_change",
                "role": "tenant"
            })))
        }
        Err(_) => Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        )),
    }
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

    // Paso 1: validar token sin consumir
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

    // Paso 2: crear admin
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .initialize_master(&body.username, &hash)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    }

    // Paso 3: consumir token solo tras éxito
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .consume_setup_token(&body.setup_token)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    }

    Ok(Json(json!({
        "success": true,
        "factory_reset_applied": true
    })))
}
