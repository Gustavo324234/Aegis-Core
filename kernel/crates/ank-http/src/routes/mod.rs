pub mod admin;
pub mod auth;
pub mod engine;
pub mod openapi;
pub mod providers;
pub mod router_api;
pub mod siren_api;
pub mod status;
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
        .nest("/api/router", router_api::router())
        .nest("/api/workspace", workspace::router())
        .nest("/api/providers", providers::router())
        .nest("/api/status", status::router())
        .nest("/api/system", status::system_router())
        .nest("/api/siren", siren_api::router())
        // WebSocket Routes
        .nest("/ws/chat", ws::chat::router())
        .nest("/ws/siren", ws::siren::router())
        // Static Files (Catch-all)
        .fallback(static_files::spa_handler)
        .with_state(state)
}
