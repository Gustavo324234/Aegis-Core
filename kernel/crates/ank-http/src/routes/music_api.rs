use crate::AppState;
use axum::{routing::get, Router};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new().route("/config", get(get_music_config))
}

async fn get_music_config() -> axum::Json<serde_json::Value> {
    let configured = std::env::var("YOUTUBE_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if configured {
        axum::Json(json!({
            "configured": true,
            "provider": "youtube"
        }))
    } else {
        axum::Json(json!({
            "configured": false
        }))
    }
}
