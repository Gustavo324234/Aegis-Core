use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::{
    enclave::TenantDB,
    workspace::config::{WorkspaceConfig, WorkspaceSettingsDto},
};
use axum::{
    extract::{Multipart, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::fs;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload))
        .route("/config", get(get_config).post(set_config))
}

// ── GET /api/workspace/config ─────────────────────────────────────────────────

async fn get_config(
    _state: State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<WorkspaceSettingsDto>, AegisHttpError> {
    let dto = tokio::task::spawn_blocking(move || -> anyhow::Result<WorkspaceSettingsDto> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        let settings = WorkspaceConfig::load_all(&db)?;
        Ok(WorkspaceSettingsDto::from(settings))
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(dto))
}

// ── POST /api/workspace/config ────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetConfigBody {
    key: String,
    value: String,
}

async fn set_config(
    _state: State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<SetConfigBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let allowed_keys = [
        "github_token",
        "project_root",
        "github_repo",
        "terminal_allowlist",
        "pr_merge_mode",
        "pr_auto_fix_ci",
    ];
    if !allowed_keys.contains(&body.key.as_str()) {
        return Err(AegisHttpError::BadRequest(format!(
            "Unknown config key: {}",
            body.key
        )));
    }

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        WorkspaceConfig::set(&db, &body.key, &body.value)?;
        Ok(())
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(json!({ "status": "ok" })))
}

// ── POST /api/workspace/upload ────────────────────────────────────────────────

async fn upload(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    mut multipart: Multipart,
) -> Result<Json<Value>, AegisHttpError> {
    let mut file_data = None;
    let mut original_name = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            original_name = field.file_name().map(|s| s.to_string());
            file_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?,
            );
        }
    }

    let file_data = file_data.ok_or_else(|| AegisHttpError::BadRequest("Missing file".into()))?;
    let original_name =
        original_name.ok_or_else(|| AegisHttpError::BadRequest("Missing filename".into()))?;

    let safe_name = original_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    if safe_name.is_empty() || safe_name.starts_with('.') {
        return Err(AegisHttpError::BadRequest("Invalid filename format".into()));
    }

    let base = state
        .config
        .data_dir
        .join("users")
        .join(&auth.tenant_id)
        .join("workspace");

    fs::create_dir_all(&base)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    let file_path = base.join(&safe_name);

    if !file_path.starts_with(&base) {
        return Err(AegisHttpError::BadRequest("Path traversal detected".into()));
    }

    fs::write(&file_path, file_data)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    Ok(Json(json!({
        "status": "success",
        "filename": safe_name,
        "message": "File injected successfully"
    })))
}
