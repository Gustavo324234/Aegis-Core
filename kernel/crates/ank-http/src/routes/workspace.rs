use crate::{citadel::hash_passphrase, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Multipart, State},
    routing::post,
    Json, Router,
};
use regex::Regex;
use serde_json::{json, Value};
use tokio::fs;

pub fn router() -> Router<AppState> {
    Router::new().route("/upload", post(upload))
}

async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, AegisHttpError> {
    let mut tenant_id = None;
    let mut session_key = None;
    let mut file_data = None;
    let mut original_name = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "tenant_id" {
            tenant_id = Some(
                field
                    .text()
                    .await
                    .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?,
            );
        } else if name == "session_key" {
            session_key = Some(
                field
                    .text()
                    .await
                    .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?,
            );
        } else if name == "file" {
            original_name = field.file_name().map(|s| s.to_string());
            file_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?,
            );
        }
    }

    let tenant_id =
        tenant_id.ok_or_else(|| AegisHttpError::BadRequest("Missing tenant_id".into()))?;
    let session_key =
        session_key.ok_or_else(|| AegisHttpError::BadRequest("Missing session_key".into()))?;
    let file_data = file_data.ok_or_else(|| AegisHttpError::BadRequest("Missing file".into()))?;
    let original_name =
        original_name.ok_or_else(|| AegisHttpError::BadRequest("Missing filename".into()))?;

    // Validar tenant_id
    let tenant_re = Regex::new(r"^[a-zA-Z0-9_-]+$")
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
    if !tenant_re.is_match(&tenant_id) {
        return Err(AegisHttpError::BadRequest(
            "Invalid tenant_id format".into(),
        ));
    }

    // Auth
    let hash = hash_passphrase(&session_key);
    {
        let citadel = state.citadel.lock().await;
        let is_auth = citadel
            .enclave
            .authenticate_tenant(&tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;

        if !is_auth {
            return Err(AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized));
        }
    }

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

    // Resolve path
    let base = state
        .config
        .data_dir
        .join("users")
        .join(&tenant_id)
        .join("workspace");

    // Create base dir if not exists
    fs::create_dir_all(&base)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    let file_path = base.join(&safe_name);

    // Path traversal check
    if !file_path.starts_with(&base) {
        return Err(AegisHttpError::BadRequest("Path traversal detected".into()));
    }

    // Write file
    fs::write(&file_path, file_data)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    Ok(Json(json!({
        "status": "success",
        "filename": safe_name,
        "message": "File injected successfully"
    })))
}
