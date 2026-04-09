pub mod chat;
pub mod siren;

use crate::AppState;
use axum::Router;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/ws/chat", chat::router())
        .nest("/ws/siren", siren::router())
        .with_state(state)
}
