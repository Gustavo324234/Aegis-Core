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
    #[schema(example = "new_user")]
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
    #[schema(example = "tenant_001")]
    pub tenant_id: String,
    #[schema(format = "password")]
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
    #[schema(example = "admin")]
    pub admin_tenant_id: String,
    #[schema(format = "password")]
    pub admin_session_key: String,
    #[schema(example = "tenant_001")]
    pub target_tenant_id: String,
}

/// Extrae y valida credenciales Master Admin desde headers Citadel (`cst_*` o key legacy).
/// Retorna el tenant_id autenticado o error sin retener el lock de `citadel`.
async fn require_master_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<String, AegisHttpError> {
    crate::citadel::authenticate_master_headers(state, headers)
        .await
        .map_err(AegisHttpError::Citadel)
}

fn validate_tenant_username(raw: &str) -> Result<String, AegisHttpError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(AegisHttpError::BadRequest(
            "Username must be 1..=64 characters".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AegisHttpError::BadRequest(
            "Username may only contain ASCII letters, digits, '_' and '-'".to_string(),
        ));
    }
    if matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "admin" | "root" | "default" | "system"
    ) {
        return Err(AegisHttpError::BadRequest(
            "Username is reserved".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

async fn cleanup_deleted_tenant_resources(state: &AppState, target_id: &str) {
    state.session_store.revoke_tenant(target_id);
    ank_core::enclave::TenantDB::evict_tenant(target_id);
    {
        let router = state.router.read().await;
        if let Err(e) = router.purge_tenant_keys(target_id).await {
            tracing::warn!(
                tenant_id = %target_id,
                error = %e,
                "Failed to purge tenant keys from KeyPool"
            );
        }
        router.invalidate_sticky(target_id).await;
    }

    let _ = state.whatsapp.disconnect(target_id, true).await;

    let valid_dir_name = !target_id.is_empty()
        && target_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid_dir_name {
        let user_dir = state.config.data_dir.join("users").join(target_id);
        if user_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&user_dir) {
                tracing::warn!(
                    tenant_id = %target_id,
                    error = %e,
                    "Failed to remove deleted tenant directory"
                );
            }
        }
    }

    crate::ws::chat::invalidate_onboarding_cache(target_id);
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
    let username = validate_tenant_username(&body.username)?;

    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    let (port, temp_pass) = enclave
        .create_tenant(&username)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    Ok(Json(TenantResponse {
        tenant_id: username,
        temporary_passphrase: temp_pass,
        network_port: port as u16,
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

    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    let tenants = enclave
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
            last_active: Some(t.last_active),
            port: t.port as u16,
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
    let admin_id = require_master_auth(&state, &headers).await?;
    if target_id == admin_id {
        return Err(AegisHttpError::BadRequest(
            "Cannot delete active master admin account".into(),
        ));
    }

    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    enclave
        .delete_tenant(&target_id)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    cleanup_deleted_tenant_resources(&state, &target_id).await;

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
    if body.target_tenant_id == body.admin_tenant_id {
        return Err(AegisHttpError::BadRequest(
            "Cannot delete active master admin account".into(),
        ));
    }

    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    let is_auth = if body
        .admin_session_key
        .trim()
        .starts_with(crate::citadel::CITADEL_TOKEN_PREFIX)
    {
        state
            .session_store
            .resolve(&body.admin_session_key, Some(&body.admin_tenant_id))
            .map(|a| a.is_master)
            .unwrap_or(false)
    } else {
        let admin_hash = hash_passphrase(&body.admin_session_key);
        enclave
            .authenticate_master(&body.admin_tenant_id, &admin_hash)
            .await
            .map_err(|e| AegisHttpError::Kernel(e.to_string()))?
    };

    if !is_auth {
        return Err(AegisHttpError::Citadel(CitadelError::Unauthorized));
    }

    enclave
        .delete_tenant(&body.target_tenant_id)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    cleanup_deleted_tenant_resources(&state, &body.target_tenant_id).await;

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

    if body.new_passphrase.trim().is_empty() || body.new_passphrase.chars().count() < 6 {
        return Err(AegisHttpError::BadRequest(
            "New passphrase must be at least 6 characters long".to_string(),
        ));
    }

    let new_hash = hash_passphrase(&body.new_passphrase);
    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };

    enclave
        .reset_tenant_password(&body.tenant_id, &new_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    state.session_store.revoke_tenant(&body.tenant_id);
    let _ = ank_core::enclave::TenantDB::quarantine_for_password_reset(&body.tenant_id);

    Ok(Json(PasswordResetResponse {
        success: true,
        message: "Password reset successful".to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_tenant_username_rejects_traversal_and_reserved() {
        assert!(validate_tenant_username("../evil").is_err());
        assert!(validate_tenant_username("alice/bob").is_err());
        assert!(validate_tenant_username("alice bob").is_err());
        assert!(validate_tenant_username("admin").is_err());
        assert!(validate_tenant_username("root").is_err());
        assert!(validate_tenant_username("default").is_err());
        assert!(validate_tenant_username("").is_err());
        assert_eq!(
            validate_tenant_username("  alice_01-dev  ").ok().as_deref(),
            Some("alice_01-dev")
        );
    }
}
