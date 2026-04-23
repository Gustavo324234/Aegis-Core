pub mod citadel;
pub mod config;
pub mod error;
pub mod rate_limiter;
pub mod routes;
pub mod state;
mod static_files;
pub mod ws;

pub use config::HttpConfig;
pub use state::AppState;

use anyhow::Result;
use std::net::SocketAddr;

pub struct AegisHttpServer {
    pub state: AppState,
}

impl AegisHttpServer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn serve(self) -> Result<()> {
        let port = self.state.config.port;
        let app = routes::build_router(self.state);
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;

        tracing::info!("Aegis HTTP server listening on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}
