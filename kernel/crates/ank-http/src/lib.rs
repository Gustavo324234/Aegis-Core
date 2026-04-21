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
use axum_server::tls_rustls::RustlsConfig;
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
        let tls_paths = self
            .state
            .config
            .tls_paths()
            .map(|(c, k)| (c.clone(), k.clone()));
        let app = routes::build_router(self.state);
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;

        if let Some((tls_cert, tls_key)) = tls_paths {
            let tls_config = RustlsConfig::from_pem_file(&tls_cert, &tls_key)
                .await
                .map_err(|e| anyhow::anyhow!("TLS config failed: {}", e))?;

            tracing::info!("Aegis HTTPS server (TLS) listening on https://{}", addr);
            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        } else {
            tracing::warn!("Aegis HTTP server (INSECURE) listening on http://{}", addr);
            tracing::warn!("Siren (microphone) will not work from other devices over HTTP.");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await?;
        }

        Ok(())
    }
}
