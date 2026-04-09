use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenant", post(create_tenant))
        .route("/tenant/create", post(create_tenant)) // Alias
        .route("/tenants", get(list_tenants))
        .route("/tenant/:id", delete(delete_tenant_path))
        .route("/tenant/delete", post(delete_tenant_body))
        .route("/reset_password", post(reset_password))
}

#[derive(Deserialize)]
pub struct TenantCreateRequest {
    pub admin_tenant_id: String,
    pub admin_session_key: String,
    pub username: String,
}

#[derive(Deserialize)]
pub struct TenantDeleteAction {
    pub admin_tenant_id: String,
    pub admin_session_key: String,
    pub target_tenant_id: String,
}

#[derive(Deserialize)]
pub struct PasswordResetRequest {
    pub tenant_id: String,
    pub admin_tenant_id: String,
    pub admin_session_key: String,
    pub new_passphrase: String,
}

#[derive(Deserialize)]
pub struct AdminAuthQuery {
    pub admin_tenant_id: String,
    pub admin_session_key: String,
}

pub async fn create_tenant(
    State(state): State<AppState>,
    Json(body): Json<TenantCreateRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = hash_passphrase(&body.admin_session_key);
    let citadel = state.citadel.lock().await;

    // TODO(ANK-SEC-044): Validador centralizado de admin
    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    let (port, temp_pass) = citadel
        .enclave
        .create_tenant(&body.username) // Enclave usa username como tenant_id
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
    Query(query): Query<AdminAuthQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = hash_passphrase(&query.admin_session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&query.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

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

    Ok(Json(json!(tenants_json)))
}

pub async fn delete_tenant_path(
    State(state): State<AppState>,
    Path(target_id): Path<String>,
    Query(query): Query<AdminAuthQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = hash_passphrase(&query.admin_session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&query.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

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
    let admin_hash = hash_passphrase(&body.admin_session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
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
    Json(body): Json<PasswordResetRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let new_hash = hash_passphrase(&body.new_passphrase);
    let admin_hash = hash_passphrase(&body.admin_session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(
        json!({ "success": true, "message": "Password reset successful" }),
    ))
}
