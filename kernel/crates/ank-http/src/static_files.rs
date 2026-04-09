use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::path::PathBuf;
use tower::ServiceExt;
use tower_http::services::ServeDir;

#[cfg(feature = "embed-ui")]
use include_dir::{include_dir, Dir};
#[cfg(feature = "embed-ui")]
use mime_guess;

#[cfg(feature = "embed-ui")]
static UI_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../../shell/ui/dist");

pub async fn spa_handler(State(state): State<AppState>, uri: Uri, req: Request<Body>) -> Response {
    if state.config.dev_mode {
        return StatusCode::NOT_FOUND.into_response();
    }

    #[cfg(feature = "embed-ui")]
    {
        let path = uri.path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };

        if let Some(file) = UI_DIR.get_file(path) {
            let mime_type = mime_guess::from_path(path).first_or_octet_stream();
            return Response::builder()
                .header("Content-Type", mime_type.as_ref())
                .body(Body::from(file.contents()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }

        // SPA Fallback for embedded
        if let Some(index) = UI_DIR.get_file("index.html") {
            return Response::builder()
                .header("Content-Type", "text/html")
                .body(Body::from(index.contents()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }

    // Fallback to disk if not embedded or file not found in embed
    let dist_path = state
        .config
        .ui_dist_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("./shell/ui/dist"));

    // Check if the file exists
    let path = uri.path().trim_start_matches('/');
    let target_file = dist_path.join(path);

    if target_file.is_file() {
        let service = ServeDir::new(&dist_path);
        match service.oneshot(req).await {
            Ok(res) => res.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    } else {
        // SPA Fallback: serve index.html for any non-file request (client-side routing)
        let index_path = dist_path.join("index.html");
        if index_path.exists() {
            let index_req = match Request::builder().uri("/index.html").body(Body::empty()) {
                Ok(r) => r,
                Err(e) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            };
            match ServeDir::new(&dist_path).oneshot(index_req).await {
                Ok(res) => res.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            }
        } else {
            StatusCode::NOT_FOUND.into_response()
        }
    }
}
