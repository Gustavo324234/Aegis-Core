pub mod admin;
pub mod auth;
pub mod engine;
pub mod providers;
pub mod router_api;
pub mod siren_api;
pub mod status;
pub mod workspace;

use crate::static_files;
use crate::ws;
use crate::AppState;
use axum::Router;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(status::health_check))
        // API Routes
        .nest("/api/auth", auth::router())
        .nest("/api/admin", admin::router())
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
