use axum::{routing::get, Router};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(music_health))
}

async fn music_health() -> &'static str {
    "Music API not implemented"
}
