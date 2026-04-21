use axum::{routing::get, Router};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(oauth_health))
}

async fn oauth_health() -> &'static str {
    "OAuth API not implemented"
}
