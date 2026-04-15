use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/setup", post(setup))
        .route("/setup-token", post(setup_token))
        .route("/change_password", post(change_password))
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

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub tenant_id: String,
    pub current_password: String,
    pub new_password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.session_key);
    let citadel = state.citadel.lock().await;

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

    Ok(Json(json!({
        "success": true,
        "factory_reset_applied": true
    })))
}

/// Permite a un tenant cambiar su propia contraseña.
/// Verifica la contraseña actual (incluso si password_must_change=1) antes de aplicar la nueva.
/// No requiere privilegios de admin.
pub async fn change_password(
    State(state): State<AppState>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let current_hash = hash_passphrase(&body.current_password);
    let new_hash = hash_passphrase(&body.new_password);

    let citadel = state.citadel.lock().await;

    // Verificar contraseña actual — ignorar PASSWORD_MUST_CHANGE, solo verificar credenciales
    let row: Result<(String, i32), _> = {
        let conn = citadel.enclave.get_connection();
        let conn = conn.blocking_lock();
        let mut stmt = conn
            .prepare(
                "SELECT password_hash, password_must_change FROM tenants WHERE tenant_id = ?1 LIMIT 1",
            )
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;
        stmt.query_row([&body.tenant_id], |row| Ok((row.get(0)?, row.get(1)?)))
    };

    match row {
        Ok((real_hash, _)) => {
            use argon2::{
                password_hash::{PasswordHash, PasswordVerifier},
                Argon2,
            };
            let parsed = PasswordHash::new(&real_hash)
                .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;
            let valid = Argon2::default()
                .verify_password(current_hash.as_bytes(), &parsed)
                .is_ok();
            if !valid {
                return Err(AegisHttpError::Citadel(
                    crate::citadel::CitadelError::Unauthorized,
                ));
            }
        }
        Err(_) => {
            return Err(AegisHttpError::Citadel(
                crate::citadel::CitadelError::Unauthorized,
            ));
        }
    }

    // Aplicar nueva contraseña y limpiar password_must_change
    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "Password updated successfully"
    })))
}
