use crate::AppState;
use axum::{routing::get, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(music_health))
}

async fn music_health() -> &'static str {
    "Music API not implemented"
}
