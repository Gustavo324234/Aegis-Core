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

    response_modules.sort_by_key(|m| {
        let rank = match m.module_id.as_str() {
            "aegis.domain.creator" => 0,
            "aegis.domain.video" => 1,
            "aegis.domain.dev" => 2,
            "aegis.domain.business" => 3,
            _ => 4,
        };
        (rank, m.module_id.clone())
    });

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

    {
        let router = state.router.read().await;
        let modules_registry = router.modules.read().await;
        if !modules_registry.contains_key(&module_id) {
            return Err(AegisHttpError::BadRequest(format!(
                "Module '{}' not found in system",
                module_id
            )));
        }
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

    let _ = state
        .workspace_events
        .send(ank_core::pr_manager::WorkspaceWsEvent {
            tenant_id: auth.tenant_id.clone(),
            payload: serde_json::json!({
                "event": "modules_updated",
                "data": { "module_id": module_id, "active": body.enabled }
            }),
        });

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
    let is_active = {
        let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|e| {
                AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e))
            })?;
        db.get_kv(&format!("module_active:{}", module_id))
            .unwrap_or(None)
            .map(|v| v == "true")
            .unwrap_or(false)
    };

    if !is_active {
        return Err(AegisHttpError::BadRequest(format!(
            "Module '{}' is not enabled for this tenant",
            module_id
        )));
    }

    let (manifest, key_pool) = {
        let router = state.router.read().await;
        let modules_registry = router.modules.read().await;
        let m = modules_registry.get(&module_id).cloned().ok_or_else(|| {
            AegisHttpError::BadRequest(format!("Module '{}' not found in system", module_id))
        })?;
        (m, router.key_pool_arc())
    };

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

    // Convert arguments to string JSON
    let args_str = serde_json::to_string(&body.arguments)
        .map_err(|e| AegisHttpError::BadRequest(format!("Invalid arguments format: {}", e)))?;

    let ws_tx = std::sync::Arc::clone(&state.workspace_events);
    let tid_cb = auth.tenant_id.clone();
    let video_ctx = ank_core::video_studio::VideoExecContext {
        data_dir: state.config.data_dir.clone(),
        key_pool: Some(key_pool),
        router: Some(std::sync::Arc::clone(&state.router)),
        progress_cb: Some(std::sync::Arc::new(move |evt| {
            let _ = ws_tx.send(ank_core::pr_manager::WorkspaceWsEvent {
                tenant_id: tid_cb.clone(),
                payload: serde_json::json!({
                    "event": "video_studio_progress",
                    "data": evt
                }),
            });
        })),
    };

    let result_json = ank_core::router::modules::execute_module_tool_with_context(
        &manifest,
        &body.tool_name,
        &args_str,
        &auth.tenant_id,
        &auth.session_key_hash,
        Some(&video_ctx),
    )
    .await
    .map_err(|e| {
        if e.starts_with("Invalid endpoint format:") || e.starts_with("Invalid tool arguments") {
            AegisHttpError::BadRequest(e)
        } else {
            AegisHttpError::Internal(anyhow::anyhow!(e))
        }
    })?;

    if !body.tool_name.ends_with("_get_overview") {
        let _ = state
            .workspace_events
            .send(ank_core::pr_manager::WorkspaceWsEvent {
                tenant_id: auth.tenant_id.clone(),
                payload: serde_json::json!({
                    "event": "modules_updated",
                    "data": { "module_id": module_id, "tool_name": body.tool_name }
                }),
            });
    }

    let parsed_result: serde_json::Value =
        serde_json::from_str(&result_json).unwrap_or(serde_json::Value::String(result_json));
    Ok(Json(serde_json::json!({
        "success": true,
        "result": parsed_result
    })))
}
