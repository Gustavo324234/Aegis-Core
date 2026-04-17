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
use utoipa::ToSchema;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenant", post(create_tenant))
        .route("/tenant/create", post(create_tenant))
        .route("/tenants", get(list_tenants))
        .route("/tenant/:id", delete(delete_tenant_path))
        .route("/tenant/delete", post(delete_tenant_body))
        .route("/reset_password", post(reset_password))
}

#[derive(Deserialize, ToSchema)]
pub struct TenantCreateRequest {
    #[schema(example = "new_user", description = "Username for the new tenant")]
    pub username: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct TenantResponse {
    pub tenant_id: String,
    pub temporary_passphrase: String,
    pub network_port: u16,
}

#[derive(serde::Serialize, ToSchema)]
pub struct TenantInfo {
    pub tenant_id: String,
    pub username: String,
    pub role: String,
    pub created_at: String,
    pub last_active: Option<String>,
    pub port: u16,
}

#[derive(serde::Serialize, ToSchema)]
pub struct TenantsListResponse {
    pub tenants: Vec<TenantInfo>,
}

#[derive(Deserialize, ToSchema)]
pub struct PasswordResetRequest {
    #[schema(example = "tenant_001", description = "Tenant identifier")]
    pub tenant_id: String,
    #[schema(format = "password", description = "New passphrase")]
    pub new_passphrase: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct PasswordResetResponse {
    pub success: bool,
    pub message: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct TenantDeleteResponse {
    pub success: bool,
    pub message: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Deserialize, ToSchema)]
pub struct TenantDeleteAction {
    #[schema(example = "admin", description = "Admin tenant identifier")]
    pub admin_tenant_id: String,
    #[schema(format = "password", description = "Admin session key")]
    pub admin_session_key: String,
    #[schema(example = "tenant_001", description = "Target tenant to delete")]
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

#[utoipa::path(
    post,
    path = "/api/admin/tenant",
    tag = "admin",
    request_body = TenantCreateRequest,
    responses(
        (status = 200, description = "Tenant created", body = TenantResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
pub async fn create_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TenantCreateRequest>,
) -> Result<Json<TenantResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    let (port, temp_pass) = citadel
        .enclave
        .create_tenant(&body.username)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(TenantResponse {
        tenant_id: body.username,
        temporary_passphrase: temp_pass,
        network_port: port,
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/tenants",
    tag = "admin",
    responses(
        (status = 200, description = "List of tenants", body = TenantsListResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
pub async fn list_tenants(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TenantsListResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    let tenants = citadel
        .enclave
        .list_tenants()
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    let tenants_info: Vec<TenantInfo> = tenants
        .into_iter()
        .map(|t| TenantInfo {
            tenant_id: t.tenant_id,
            username: t.username,
            role: t.role,
            created_at: t.created_at,
            last_active: t.last_active,
            port: t.port,
        })
        .collect();

    Ok(Json(TenantsListResponse {
        tenants: tenants_info,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/admin/tenant/{id}",
    tag = "admin",
    responses(
        (status = 200, description = "Tenant deleted", body = TenantDeleteResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Tenant ID to delete"),
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
pub async fn delete_tenant_path(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(target_id): Path<String>,
) -> Result<Json<TenantDeleteResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let citadel = state.citadel.lock().await;
    citadel
        .enclave
        .delete_tenant(&target_id)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(TenantDeleteResponse {
        success: true,
        message: format!("Tenant {} deleted.", target_id),
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/tenant/delete",
    tag = "admin",
    request_body = TenantDeleteAction,
    responses(
        (status = 200, description = "Tenant deleted", body = TenantDeleteResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    )
)]
pub async fn delete_tenant_body(
    State(state): State<AppState>,
    Json(body): Json<TenantDeleteAction>,
) -> Result<Json<TenantDeleteResponse>, AegisHttpError> {
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

    Ok(Json(TenantDeleteResponse {
        success: true,
        message: "Tenant deleted successfully".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/reset_password",
    tag = "admin",
    request_body = PasswordResetRequest,
    responses(
        (status = 200, description = "Password reset", body = PasswordResetResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    params(
        ("x-citadel-tenant" = String, Header, description = "Admin tenant ID"),
        ("x-citadel-key" = String, Header, description = "Admin session key (plaintext)")
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, AegisHttpError> {
    require_master_auth(&state, &headers).await?;

    let new_hash = hash_passphrase(&body.new_passphrase);
    let citadel = state.citadel.lock().await;

    citadel
        .enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(PasswordResetResponse {
        success: true,
        message: "Password reset successful".to_string(),
    }))
}
