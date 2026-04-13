use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tokio::fs;

pub fn router() -> Router<AppState> {
    Router::new().route("/upload", post(upload))
}

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

    // Sanitizar filename
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

    // Resolve path using auth.tenant_id from Citadel headers
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

    // Path traversal check
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
