use crate::AppState;
use axum::{routing::get, Router};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new().route("/config", get(get_music_config))
}

/// CORE-140: Music is always configured now.
/// With OAuth (Spotify/Google), a static YOUTUBE_API_KEY is no longer required.
/// The syscall executor checks per-tenant OAuth tokens at runtime.
async fn get_music_config() -> axum::Json<serde_json::Value> {
    axum::Json(json!({
        "configured": true,
        "provider": "auto"
    }))
}
