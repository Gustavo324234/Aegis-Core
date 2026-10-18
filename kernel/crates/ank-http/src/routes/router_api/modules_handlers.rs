use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Serialize)]
pub struct ModuleUiResponse {
    pub module_id: String,
    pub display_name: String,
    pub version: String,
    pub active: bool,
    pub exposed_tools: Vec<ank_core::router::modules::ExposedTool>,
    pub ui_views: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ModulesListResponse {
    pub modules: Vec<ModuleUiResponse>,
}

#[derive(Deserialize)]
pub struct EnableModuleBody {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ExecuteToolBody {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

pub async fn list_modules(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<ModulesListResponse>, AegisHttpError> {
    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    // Open TenantDB to query module status
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let mut response_modules = Vec::new();

    for manifest in modules_registry.values() {
        let active = db
            .get_kv(&format!("module_active:{}", manifest.module_id))
            .unwrap_or(None)
            .map(|v| v == "true")
            .unwrap_or(false);

        response_modules.push(ModuleUiResponse {
            module_id: manifest.module_id.clone(),
            display_name: manifest.display_name.clone(),
            version: manifest.version.clone(),
            active,
            exposed_tools: manifest.exposed_tools.clone(),
            ui_views: manifest.ui_views.clone(),
        });
    }

    Ok(Json(ModulesListResponse {
        modules: response_modules,
    }))
}

pub async fn enable_module(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(module_id): Path<String>,
    Json(body): Json<EnableModuleBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    // Open TenantDB to toggle status
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    if !modules_registry.contains_key(&module_id) {
        return Err(AegisHttpError::BadRequest(format!(
            "Module '{}' not found in system",
            module_id
        )));
    }

    db.set_kv(
        &format!("module_active:{}", module_id),
        if body.enabled { "true" } else { "false" },
    )
    .map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to set active status in DB: {}", e))
    })?;

    info!(
        "HTTP API: Module '{}' active status set to {} for tenant '{}'",
        module_id, body.enabled, auth.tenant_id
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "module_id": module_id,
        "active": body.enabled
    })))
}

pub async fn execute_module_tool(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(module_id): Path<String>,
    Json(body): Json<ExecuteToolBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    // Verify that the module is active for this tenant
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let is_active = db
        .get_kv(&format!("module_active:{}", module_id))
        .unwrap_or(None)
        .map(|v| v == "true")
        .unwrap_or(false);

    if !is_active {
        return Err(AegisHttpError::BadRequest(format!(
            "Module '{}' is not enabled for this tenant",
            module_id
        )));
    }

    let router = state.router.read().await;
    let modules_registry = router.modules.read().await;

    let manifest = modules_registry.get(&module_id).ok_or_else(|| {
        AegisHttpError::BadRequest(format!("Module '{}' not found in system", module_id))
    })?;

    // Verify the tool is exposed by this module
    if !manifest
        .exposed_tools
        .iter()
        .any(|t| t.name == body.tool_name)
    {
        return Err(AegisHttpError::BadRequest(format!(
            "Tool '{}' is not exposed by module '{}'",
            body.tool_name, module_id
        )));
    }

    let endpoint = manifest.ipc_transport.endpoint.clone();

    // Convert arguments to string JSON
    let args_str = serde_json::to_string(&body.arguments)
        .map_err(|e| AegisHttpError::BadRequest(format!("Invalid arguments format: {}", e)))?;

    // Call the external module tool via pooled HTTP/2 gRPC channel (CORE-353)
    let response = ank_core::router::modules::call_module_tool_pooled(
        &endpoint,
        &body.tool_name,
        &args_str,
        &auth.tenant_id,
    )
    .await
    .map_err(|e| {
        if e.starts_with("Invalid endpoint format:") {
            AegisHttpError::BadRequest(e)
        } else {
            AegisHttpError::Internal(anyhow::anyhow!(e))
        }
    })?;

    if response.success {
        // Formulate result as a parsed JSON value
        let parsed_result: serde_json::Value = serde_json::from_str(&response.result_json)
            .unwrap_or(serde_json::Value::String(response.result_json));
        Ok(Json(serde_json::json!({
            "success": true,
            "result": parsed_result
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "error": response.result_json
        })))
    }
}
