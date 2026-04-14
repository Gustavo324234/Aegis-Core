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

    // Master Admin check first — determines role = "admin"
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

    // Regular tenant check — role = "tenant"
    let is_auth = citadel
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

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    Ok(Json(json!({
        "message": "Citadel Handshake Successful",
        "status": "authenticated",
        "role": "tenant"
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

    // SRE-FIX (CORE-090 follow-up): Operaciones en pasos separados con locks independientes.
    //
    // BUG ANTERIOR:
    //   1. validate_and_consume_setup_token()  ← token quemado
    //   2. initialize_master()                 ← si falla: token quemado, admin nunca creado
    //      → sistema queda sin token ni admin, requiere reinstalación
    //
    // FIX:
    //   1. Validar token (sin consumir)
    //   2. initialize_master()
    //   3. Consumir token SOLO si initialize_master fue exitoso
    //   4. Si initialize_master falla: token sigue válido, usuario puede reintentar

    // Paso 1: Validar que el token existe y no está expirado (sin consumirlo todavía)
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
    } // lock liberado

    // Paso 2: Crear el Master Admin
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .initialize_master(&body.username, &hash)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    } // lock liberado

    // Paso 3: Consumir el token SOLO después de que initialize_master fue exitoso
    {
        let citadel = state.citadel.lock().await;
        citadel
            .enclave
            .consume_setup_token(&body.setup_token)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
    } // lock liberado

    Ok(Json(json!({
        "success": true,
        "factory_reset_applied": true
    })))
}
