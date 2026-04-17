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
    #[error("Rate limit exceeded. Retry after {0} seconds")]
    RateLimitExceeded(u64),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AegisHttpError {
    fn into_response(self) -> Response {
        match self {
            AegisHttpError::Citadel(e) => e.into_response(),
            AegisHttpError::BadRequest(m) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": m }))).into_response()
            }
            AegisHttpError::Kernel(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": m })),
            )
                .into_response(),
            AegisHttpError::RateLimitExceeded(retry_after) => {
                let mut response = Json(json!({ "error": format!("Rate limit exceeded. Retry after {} seconds", retry_after) })).into_response();
                response.headers_mut().insert(
                    axum::http::header::RETRY_AFTER,
                    axum::http::HeaderValue::from(retry_after),
                );
                (StatusCode::TOO_MANY_REQUESTS, response).into_response()
            }
            AegisHttpError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response(),
        }
    }
}
