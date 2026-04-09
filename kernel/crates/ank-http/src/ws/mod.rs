pub mod chat;
pub mod siren;

use axum::Router;
use crate::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/ws/chat", chat::router())
        .nest("/ws/siren", siren::router())
        .with_state(state)
}
