mod health;
mod supervisor;
#[cfg(windows)]
mod windows_service;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;
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
    /// Tail logs
    Logs,
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

    #[cfg(windows)]
    {
        // Try to run as a Windows Service if invoked by SCM
        if windows_service::run_as_service().is_err() {
            // Not a service context, continue as CLI
        } else {
            return Ok(());
        }
    }

    let cli = Cli::parse();
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
            println!("Stop command received. Terminating Aegis processes...");
            // Simplified: we could send signals if we had a daemon,
            // but for now we follow the legacy's path or just a simple message.
        }
        Commands::Restart => {
            info!("Restarting Aegis service...");
        }
        Commands::Status => {
            let supervisor = AegisSupervisor::new(config);
            supervisor.status().await?;
        }
        Commands::Logs => {
            info!("Tailing logs...");
            println!("Feature in development: Log file tailing");
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
        }
        Commands::UninstallService => {
            info!("Removing Aegis service registration...");
            #[cfg(windows)]
            windows_service::uninstall_service()?;
        }
    }

    Ok(())
}

fn load_config() -> Result<SupervisorConfig> {
    let root_key = env::var("AEGIS_ROOT_KEY").unwrap_or_else(|_| "development_key".to_string());
    let config_path = get_config_path();

    let mut config = SupervisorConfig {
        ank_bin: PathBuf::from("ank-server"),
        data_dir: dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("aegis"),
        root_key,
        port: 8000,
        dev_mode: false,
    };

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let toml_val: toml::Value = toml::from_str(&content)?;

        if let Some(runtime) = toml_val.get("runtime") {
            if let Some(bin) = runtime.get("ank_bin").and_then(|v| v.as_str()) {
                config.ank_bin = PathBuf::from(bin);
            }
            if let Some(dir) = runtime.get("data_dir").and_then(|v| v.as_str()) {
                config.data_dir = PathBuf::from(dir);
            }
            if let Some(port) = runtime.get("port").and_then(|v| v.as_integer()) {
                config.port = port as u16;
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
