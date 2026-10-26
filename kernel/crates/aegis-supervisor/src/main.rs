mod health;
mod supervisor;
mod windows_service;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::path::{Path, PathBuf};
use supervisor::{AegisSupervisor, SupervisorConfig};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "aegis")]
#[command(about = "Aegis OS Unified Process Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Internal: tells the supervisor to run as a background service
    #[arg(long)]
    service: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start ANK Kernel
    Start,
    /// Stop ANK Kernel
    Stop,
    /// Restart service
    Restart,
    /// Show status
    Status,
    /// Print recent kernel logs
    Logs {
        /// Number of trailing lines to print
        #[arg(short = 'n', long, default_value_t = 80)]
        lines: usize,
    },
    /// Start in development mode
    Dev,
    /// Install as a background service
    #[command(name = "install-service")]
    InstallService,
    /// Uninstall from background services
    #[command(name = "uninstall-service")]
    UninstallService,
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed")?;

    let cli = Cli::parse();

    // Only probe Windows SCM when invoked with `--service` or without an explicit CLI subcommand
    // (legacy service registrations without `--service`), avoiding SCM delays on `status`/`logs`.
    #[cfg(windows)]
    if (cli.service || cli.command.is_none()) && windows_service::run_as_service().is_ok() {
        return Ok(());
    }

    let mut config = load_config()?;

    if cli.service {
        info!("Aegis Supervisor: Running as background service...");
        config.dev_mode = false;
        let mut supervisor = AegisSupervisor::new(config);
        supervisor.start().await?;
        supervisor.watch().await;
        return Ok(());
    }

    match cli
        .command
        .context("No command provided. Use 'aegis --help' for usage.")?
    {
        Commands::Start => {
            config.dev_mode = false;
            let mut supervisor = AegisSupervisor::new(config);
            supervisor.start().await?;
            supervisor.watch().await;
        }
        Commands::Stop => {
            let supervisor = AegisSupervisor::new(config);
            if supervisor.stop_external_instance().await? {
                println!("ANK Kernel stopped.");
            } else {
                println!("No active managed ANK PID file found.");
            }
        }
        Commands::Restart => {
            info!("Restarting Aegis service...");
            config.dev_mode = false;
            let mut supervisor = AegisSupervisor::new(config);
            let _ = supervisor.stop_external_instance().await;
            supervisor.start().await?;
            supervisor.watch().await;
        }
        Commands::Status => {
            let supervisor = AegisSupervisor::new(config);
            supervisor.status().await?;
        }
        Commands::Logs { lines } => {
            let supervisor = AegisSupervisor::new(config);
            supervisor.tail_logs(lines)?;
        }
        Commands::Dev => {
            info!("Starting Aegis in DEV mode...");
            config.dev_mode = true;
            let mut supervisor = AegisSupervisor::new(config);
            supervisor.start().await?;
            supervisor.watch().await;
        }
        Commands::InstallService => {
            info!("Attempting to register Aegis as a system service...");
            #[cfg(windows)]
            windows_service::install_service()?;
            #[cfg(not(windows))]
            windows_service::install_service()?;
        }
        Commands::UninstallService => {
            info!("Removing Aegis service registration...");
            #[cfg(windows)]
            windows_service::uninstall_service()?;
            #[cfg(not(windows))]
            windows_service::uninstall_service()?;
        }
    }

    Ok(())
}

pub(crate) fn load_config() -> Result<SupervisorConfig> {
    let config_path = get_config_path();
    let env_root_key = env::var("AEGIS_ROOT_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    load_config_from_path(&config_path, env_root_key)
}

fn load_config_from_path(
    config_path: &Path,
    env_root_key: Option<String>,
) -> Result<SupervisorConfig> {
    let mut config = SupervisorConfig {
        ank_bin: PathBuf::from("ank-server"),
        data_dir: dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aegis"),
        root_key: env_root_key,
        port: 8000,
        dev_mode: false,
    };

    if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;
        let toml_val: toml::Value = toml::from_str(&content)?;

        if let Some(runtime) = toml_val.get("runtime") {
            if let Some(bin) = runtime.get("ank_bin").and_then(|v| v.as_str()) {
                config.ank_bin = PathBuf::from(bin);
            }
            if let Some(dir) = runtime.get("data_dir").and_then(|v| v.as_str()) {
                config.data_dir = PathBuf::from(dir);
            }
            if let Some(port) = runtime.get("port").and_then(|v| v.as_integer()) {
                if let Ok(valid_port) = u16::try_from(port) {
                    config.port = valid_port;
                }
            }
            if config.root_key.is_none() {
                if let Some(rk) = runtime
                    .get("root_key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                {
                    config.root_key = Some(rk.to_string());
                }
            }
        }
    }

    Ok(config)
}

fn get_config_path() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("aegis");
    p.push("config.toml");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config_does_not_inject_hardcoded_development_key() -> Result<()> {
        let missing = PathBuf::from("non_existent_supervisor_config_test.toml");
        let cfg = load_config_from_path(&missing, None)?;
        assert!(cfg.root_key.is_none());
        assert!(!cfg.dev_mode);
        Ok(())
    }
}
