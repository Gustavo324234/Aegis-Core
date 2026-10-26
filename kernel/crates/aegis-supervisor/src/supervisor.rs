use crate::health;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tracing::{error, info, warn};

const PID_FILE_NAME: &str = "ank-server.pid";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorConfig {
    pub ank_bin: PathBuf,
    pub data_dir: PathBuf,
    pub root_key: Option<String>,
    pub port: u16,
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

    fn pid_file_path(&self) -> PathBuf {
        self.config.data_dir.join(PID_FILE_NAME)
    }

    fn write_pid_file(&self, pid: u32) {
        let _ = std::fs::create_dir_all(&self.config.data_dir);
        let _ = std::fs::write(self.pid_file_path(), pid.to_string());
    }

    fn remove_pid_file(&self) {
        let _ = std::fs::remove_file(self.pid_file_path());
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
        cmd.kill_on_drop(true)
            .env("AEGIS_DATA_DIR", &self.config.data_dir)
            .env("AEGIS_HTTP_PORT", self.config.port.to_string());

        match (&self.config.root_key, self.config.dev_mode) {
            (Some(rk), _) if !rk.trim().is_empty() => {
                cmd.env("AEGIS_ROOT_KEY", rk);
            }
            (_, true) => {
                warn!(
                    "AEGIS_ROOT_KEY not set in dev mode — injecting ephemeral 'development_key'."
                );
                cmd.env("AEGIS_ROOT_KEY", "development_key");
            }
            _ => {
                // Production mode without parent env var: let ank-server resolve AEGIS_ROOT_KEY
                // from C:\ProgramData\Aegis\aegis.env or /etc/aegis/aegis.env instead of shadowing it.
            }
        }

        if self.config.dev_mode {
            cmd.env("RUST_LOG", "info");
            cmd.env("AEGIS_MTLS_STRICT", "false");
            cmd.env("DEV_MODE", "true");
        }

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn ANK at {:?}", self.config.ank_bin))?;

        if let Some(pid) = child.id() {
            self.write_pid_file(pid);
        }

        self.ank_process = Some(child);
        info!("ANK Kernel started.");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Aegis processes...");

        if let Some(mut child) = self.ank_process.take() {
            info!("Killing ANK process...");
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.remove_pid_file();

        Ok(())
    }

    /// Detiene una instancia externa de `ank-server` registrada en `ank-server.pid`.
    pub async fn stop_external_instance(&self) -> Result<bool> {
        let pid_path = self.pid_file_path();
        if !pid_path.exists() {
            return Ok(false);
        }

        let raw_pid = std::fs::read_to_string(&pid_path)
            .with_context(|| format!("Failed to read PID file {:?}", pid_path))?;
        let pid: u32 = raw_pid
            .trim()
            .parse()
            .with_context(|| format!("Invalid PID '{}' in {:?}", raw_pid.trim(), pid_path))?;

        info!("Terminating managed ank-server process (PID {})...", pid);

        #[cfg(windows)]
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .await;

        #[cfg(not(windows))]
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;

        self.remove_pid_file();
        Ok(status.map(|s| s.success()).unwrap_or(false))
    }

    pub async fn health_check(&self) -> bool {
        health::check_health(self.config.port).await
    }

    /// Supervisa el proceso hijo en consola y captura `Ctrl+C` para apagado limpio.
    pub async fn watch(&mut self) {
        let (tx, rx) = watch::channel(false);
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = tx.send(true);
            }
        });
        self.watch_until_shutdown(rx).await;
    }

    /// Bucle principal de supervisión con reaping de procesos hijos (`try_wait`),
    /// verificación HTTP acotada y apagado limpio ante señal externa (SCM o Ctrl+C).
    pub async fn watch_until_shutdown(&mut self, mut shutdown_rx: watch::Receiver<bool>) {
        // Margen inicial de arranque antes del primer health check
        let _ = health::check_health_with_retries(self.config.port, 10, Duration::from_millis(500))
            .await;

        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        info!("Shutdown signal received. Stopping supervised ANK process...");
                        let _ = self.stop().await;
                        break;
                    }
                }
            }

            let mut child_exited = false;
            if let Some(ref mut child) = self.ank_process {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        warn!("ANK process exited unexpectedly with status: {}", status);
                        child_exited = true;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!("Failed to query ANK child status: {}", e);
                        child_exited = true;
                    }
                }
            } else {
                child_exited = true;
            }

            if child_exited || !self.health_check().await {
                warn!("ANK health/liveness check FAILED. Restarting...");
                let _ = self.stop().await;
                if let Err(e) = self.start().await {
                    error!("Failed to restart after ANK failure: {}", e);
                    tokio::select! {
                        _ = tokio::time::sleep(backoff) => {}
                        changed = shutdown_rx.changed() => {
                            if changed.is_err() || *shutdown_rx.borrow() {
                                break;
                            }
                        }
                    }
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                } else {
                    let _ = health::check_health_with_retries(
                        self.config.port,
                        10,
                        Duration::from_millis(500),
                    )
                    .await;
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
        if let Ok(pid_str) = std::fs::read_to_string(self.pid_file_path()) {
            println!("  - Managed PID: {}", pid_str.trim());
        }
        Ok(())
    }

    /// Muestra las últimas `max_lines` líneas del archivo de log más reciente en `{data_dir}/logs`.
    pub fn tail_logs(&self, max_lines: usize) -> Result<()> {
        let logs_dir = self.config.data_dir.join("logs");
        if !logs_dir.exists() {
            println!("No logs directory found at {}", logs_dir.display());
            return Ok(());
        }

        let mut newest_file: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in std::fs::read_dir(&logs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            match &newest_file {
                Some((best_ts, _)) if modified <= *best_ts => {}
                _ => newest_file = Some((modified, path)),
            }
        }

        let Some((_, log_path)) = newest_file else {
            println!("No log files found in {}", logs_dir.display());
            return Ok(());
        };

        let content = std::fs::read_to_string(&log_path)
            .with_context(|| format!("Failed to read log file {:?}", log_path))?;
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(max_lines.max(1));

        println!(
            "=== {} (last {} lines) ===",
            log_path.display(),
            all_lines.len() - start
        );
        for line in &all_lines[start..] {
            println!("{}", line);
        }
        Ok(())
    }
}
