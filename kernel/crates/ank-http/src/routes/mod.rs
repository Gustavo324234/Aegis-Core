pub mod admin;
pub mod agents;
pub mod auth;
pub mod chat_history;
pub mod engine;
pub mod fs;
pub mod music_api;
pub mod oauth_api;
pub mod openapi;
pub mod persona_api;
pub mod providers;
pub mod prs;
pub mod router_api;
pub mod siren_api;
pub mod status;
pub mod stt_download;
pub mod system_config_api;
pub mod workspace;

use crate::static_files;
use crate::ws;
use crate::AppState;
use axum::{routing::get, Router};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub fn build_router(state: AppState) -> Router {
    let swagger_ui =
        SwaggerUi::new("/api/docs/:_").url("/api-docs/openapi.json", openapi::ApiDoc::openapi());

    Router::new()
        .route("/health", get(status::health_check))
        .merge(swagger_ui)
        // API Routes
        .nest("/api/auth", auth::router())
        .nest("/api/admin", admin::router())
        .nest("/api/admin/system-config", system_config_api::router())
        .nest("/api/engine", engine::router())
        .nest("/api/oauth", oauth_api::router())
        .nest("/api/router", router_api::router())
        .nest("/api/chat", chat_history::router())
        .nest("/api/workspace", workspace::router())
        .nest("/api/fs", fs::router())
        .nest("/api/prs", prs::router())
        .nest("/api/git", prs::git_router())
        .nest("/api/providers", providers::router())
        .nest("/api/status", status::router())
        .nest("/api/system", status::system_router())
        .nest("/api/siren", siren_api::router())
        .nest("/api/siren/stt", stt_download::router())
        .nest("/api/persona", persona_api::router())
        .nest("/api/agents", agents::router())
        .nest("/api/music", music_api::router())
        // WebSocket Routes
        .nest("/ws/chat", ws::chat::router())
        .nest("/ws/siren", ws::siren::router())
        .nest("/ws/agents", ws::agents::router())
        // Static Files (Catch-all)
        .fallback(static_files::spa_handler)
        .with_state(state)
}
