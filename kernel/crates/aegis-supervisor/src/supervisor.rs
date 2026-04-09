use crate::health;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    pub ank_bin: PathBuf,   // path al binario ank-server
    pub data_dir: PathBuf,
    pub root_key: String,
    pub port: u16,          // puerto unificado (gRPC + HTTP suelen compartir o el supervisor solo checkea uno)
    pub dev_mode: bool,
}

pub struct AegisSupervisor {
    ank_process: Option<Child>,
    config: SupervisorConfig,
}

impl AegisSupervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            ank_process: None,
            config,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Aegis Kernel (ank-server)...");
        self.start_ank().await?;
        Ok(())
    }

    async fn start_ank(&mut self) -> Result<()> {
        if self.ank_process.is_some() {
            return Ok(());
        }

        info!(
            "Launching ANK Kernel ({})...",
            self.config.ank_bin.display()
        );
        let mut cmd = Command::new(&self.config.ank_bin);
        cmd.env("AEGIS_ROOT_KEY", &self.config.root_key)
            .env("AEGIS_DATA_DIR", &self.config.data_dir)
            .env("AEGIS_HTTP_PORT", self.config.port.to_string());

        if self.config.dev_mode {
            cmd.env("RUST_LOG", "info");
            cmd.env("AEGIS_MTLS_STRICT", "false");
            cmd.env("DEV_MODE", "true");
        }

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn ANK at {:?}", self.config.ank_bin))?;

        self.ank_process = Some(child);
        info!("ANK Kernel started.");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Aegis processes...");

        if let Some(mut child) = self.ank_process.take() {
            info!("Killing ANK process...");
            let _ = child.kill().await;
        }

        Ok(())
    }

    pub async fn health_check(&self) -> bool {
        health::check_health(self.config.port).await
    }

    pub async fn watch(&mut self) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            if !self.health_check().await {
                warn!("ANK health check FAILED. Restarting...");
                let _ = self.stop().await;
                if let Err(e) = self.start().await {
                    error!("Failed to restart after ANK failure: {}", e);
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                } else {
                    backoff = Duration::from_secs(1);
                }
            } else {
                backoff = Duration::from_secs(1);
            }
        }
    }

    pub async fn status(&self) -> Result<()> {
        let ok = self.health_check().await;
        println!("Aegis System Status:");
        println!("  - ANK Kernel: {}", if ok { "UP" } else { "DOWN" });
        Ok(())
    }
}
