use crate::{citadel::CitadelCredentials, error::AegisHttpError, state::AppState};
use ank_core::enclave::TenantDB;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tools", get(list_tools))
        .route("/servers", post(register_server))
        .route("/tools/:name", delete(unregister_tool))
        .route("/tools/:name/policy", get(get_tool_policy))
        .route("/tools/:name/policy", post(set_tool_policy))
}

#[derive(Serialize)]
pub struct McpToolDto {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Deserialize)]
pub struct RegisterServerDto {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
    pub sse_url: Option<String>,
    pub post_url: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterServerResponse {
    pub status: String,
    pub server: String,
    pub tools_registered: usize,
}

#[derive(Deserialize, Serialize)]
pub struct ToolPolicyDto {
    pub policy: String, // "allow" | "deny"
}

/// GET /api/mcp/tools — Lista todas las herramientas MCP activas
async fn list_tools(State(state): State<AppState>) -> Result<Json<Vec<McpToolDto>>, AegisHttpError> {
    let tools = state.hal.mcp_registry.list_tools().await;
    let dtos = tools
        .into_iter()
        .map(|t| McpToolDto {
            name: t.name,
            description: t.description,
            input_schema: t.input_schema,
        })
        .collect();
    Ok(Json(dtos))
}

/// POST /api/mcp/servers — Registra y conecta dinámicamente un servidor MCP
async fn register_server(
    State(state): State<AppState>,
    Json(payload): Json<RegisterServerDto>,
) -> Result<Json<RegisterServerResponse>, AegisHttpError> {
    let server_name = payload.name.clone();
    let mut cfg_map = serde_json::Map::new();

    if let Some(cmd) = payload.command {
        cfg_map.insert("command".to_string(), serde_json::Value::String(cmd));
    }
    if let Some(args) = payload.args {
        cfg_map.insert("args".to_string(), serde_json::json!(args));
    }
    if let Some(env) = payload.env {
        cfg_map.insert("env".to_string(), serde_json::json!(env));
    }
    if let Some(cwd) = payload.cwd {
        cfg_map.insert("cwd".to_string(), serde_json::Value::String(cwd));
    }
    if let Some(sse) = payload.sse_url {
        cfg_map.insert("sseUrl".to_string(), serde_json::Value::String(sse));
    }
    if let Some(post) = payload.post_url {
        cfg_map.insert("postUrl".to_string(), serde_json::Value::String(post));
    }

    let default_cwd = std::path::PathBuf::from(".");
    let count = state
        .hal
        .mcp_registry
        .connect_and_register_server(&server_name, &serde_json::Value::Object(cfg_map), &default_cwd)
        .await
        .map_err(|e| AegisHttpError::Kernel(format!("Failed to connect MCP server '{}': {}", server_name, e)))?;

    Ok(Json(RegisterServerResponse {
        status: "connected".to_string(),
        server: server_name,
        tools_registered: count,
    }))
}

/// DELETE /api/mcp/tools/:name — Desregistra una herramienta MCP
async fn unregister_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AegisHttpError> {
    if state.hal.mcp_registry.unregister_tool(&name).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AegisHttpError::BadRequest(format!("Tool '{}' not found", name)))
    }
}

/// GET /api/mcp/tools/:name/policy — Obtiene la política de ejecución del tenant para la herramienta
async fn get_tool_policy(
    creds: CitadelCredentials,
    Path(name): Path<String>,
) -> Result<Json<ToolPolicyDto>, AegisHttpError> {
    let db = TenantDB::open(&creds.tenant_id, &creds.session_key_hash)
        .map_err(|e| AegisHttpError::Kernel(format!("Database error: {}", e)))?;

    let policy_key = format!("mcp_tool_policy:{}", name);
    let policy = db
        .get_kv(&policy_key)
        .unwrap_or(None)
        .unwrap_or_else(|| "allow".to_string());

    Ok(Json(ToolPolicyDto { policy }))
}

/// POST /api/mcp/tools/:name/policy — Configura la política del tenant ("allow" o "deny")
async fn set_tool_policy(
    creds: CitadelCredentials,
    Path(name): Path<String>,
    Json(payload): Json<ToolPolicyDto>,
) -> Result<StatusCode, AegisHttpError> {
    let db = TenantDB::open(&creds.tenant_id, &creds.session_key_hash)
        .map_err(|e| AegisHttpError::Kernel(format!("Database error: {}", e)))?;

    let policy_key = format!("mcp_tool_policy:{}", name);
    db.set_kv(&policy_key, &payload.policy)
        .map_err(|e| AegisHttpError::Kernel(format!("Failed to save policy: {}", e)))?;

    Ok(StatusCode::OK)
}
