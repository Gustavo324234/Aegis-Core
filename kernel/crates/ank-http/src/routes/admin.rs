use crate::{
    citadel::{hash_passphrase, CitadelError},
    error::AegisHttpError,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenant", post(create_tenant))
        .route("/tenant/create", post(create_tenant))
        .route("/tenants", get(list_tenants))
        .route("/tenant/:id", delete(delete_tenant_path))
        .route("/tenant/delete", post(delete_tenant_body))
        .route("/reset_password", post(reset_password))
}

/// Body para crear tenant — auth viene de headers Citadel
#[derive(Deserialize)]
pub struct TenantCreateRequest {
    pub username: String,
}

/// Body para reset de contraseña — auth viene de headers Citadel
#[derive(Deserialize)]
pub struct PasswordResetRequest {
    pub tenant_id: String,
    pub new_passphrase: String,
}

/// Body legacy para delete por body (mantener compatibilidad)
#[derive(Deserialize)]
pub struct TenantDeleteAction {
    pub admin_tenant_id: String,
    pub admin_session_key: String,
    pub target_tenant_id: String,
}

/// Extrae y valida credenciales Master Admin desde headers Citadel.
/// Retorna el tenant_id autenticado o error.
async fn require_master_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, AegisHttpError> {
    let tenant_id = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingTenant))?;

    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(AegisHttpError::Citadel(CitadelError::MissingKey))?;

    let hash = hash_passphrase(&raw_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&tenant_id, &hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    Ok(tenant_id)
}

pub async fn create_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TenantCreateRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    let (port, temp_pass) = citadel
        .enclave
        .create_tenant(&body.username)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(json!({
        "tenant_id": body.username,
        "temporary_passphrase": temp_pass,
        "network_port": port
    })))
}

pub async fn list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    let tenants = citadel
        .enclave
        .list_tenants()
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    let tenants_json: Vec<Value> = tenants
        .into_iter()
        .map(|t| {
            json!({
                "tenant_id": t.tenant_id,
                "username": t.username,
                "role": t.role,
                "created_at": t.created_at,
                "last_active": t.last_active,
                "port": t.port
            })
        })
        .collect();

    // Envolver en objeto { "tenants": [...] } para que el frontend pueda hacer data.tenants
    Ok(Json(json!({ "tenants": tenants_json })))
}

pub async fn delete_tenant_path(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    citadel
        .enclave
        .delete_tenant(&target_id)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(
        json!({ "success": true, "message": format!("Tenant {} deleted.", target_id) }),
    ))
}

pub async fn delete_tenant_body(
    State(state): State<AppState>,
    Json(body): Json<TenantDeleteAction>,
) -> Result<Json<Value>, AegisHttpError> {
    // Endpoint legacy — mantiene auth por body para compatibilidad con clientes viejos
    let admin_hash = hash_passphrase(&body.admin_session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    citadel
        .enclave
        .delete_tenant(&body.target_tenant_id)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(
        json!({ "success": true, "message": "Tenant deleted successfully" }),
    ))
}

pub async fn reset_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PasswordResetRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let new_hash = hash_passphrase(&body.new_passphrase);
    let citadel = state.citadel.lock().await;

    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(
        json!({ "success": true, "message": "Password reset successful" }),
    ))
}
