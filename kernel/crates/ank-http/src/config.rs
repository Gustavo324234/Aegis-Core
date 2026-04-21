use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub port: u16,
    pub static_dir: String,
    pub dev_mode: bool,
    pub ui_dist_path: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            static_dir: "./dist".to_string(),
            dev_mode: false,
            ui_dist_path: None,
            data_dir: PathBuf::from("."),
            tls_cert: None,
            tls_key: None,
        }
    }
}

impl HttpConfig {
    pub fn from_env() -> Self {
        let port = std::env::var("ANK_HTTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3000);

        let static_dir =
            std::env::var("ANK_HTTP_STATIC_DIR").unwrap_or_else(|_| "./dist".to_string());

        let dev_mode = std::env::var("DEV_MODE")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(false);

        let ui_dist_path = std::env::var("UI_DIST_PATH").ok().map(PathBuf::from);

        let data_dir = std::env::var("AEGIS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));

        let tls_cert = std::env::var("AEGIS_TLS_CERT").ok().map(PathBuf::from);
        let tls_key = std::env::var("AEGIS_TLS_KEY").ok().map(PathBuf::from);

        Self {
            port,
            static_dir,
            dev_mode,
            ui_dist_path,
            data_dir,
            tls_cert,
            tls_key,
        }
    }

    pub fn tls_enabled(&self) -> bool {
        match (&self.tls_cert, &self.tls_key) {
            (Some(c), Some(k)) => c.exists() && k.exists(),
            _ => false,
        }
    }

    pub fn tls_paths(&self) -> Option<(&PathBuf, &PathBuf)> {
        self.tls_cert.as_ref().and_then(|c| {
            self.tls_key.as_ref().map(|k| (c, k))
        })
    }
}
