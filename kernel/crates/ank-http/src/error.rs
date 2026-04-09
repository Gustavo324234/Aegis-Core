use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AegisHttpError {
    #[error("Citadel: {0}")]
    Citadel(#[from] crate::citadel::CitadelError),
    #[error("Kernel error: {0}")]
    Kernel(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AegisHttpError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AegisHttpError::Citadel(e) => return e.into_response(),
            AegisHttpError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AegisHttpError::Kernel(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            AegisHttpError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

