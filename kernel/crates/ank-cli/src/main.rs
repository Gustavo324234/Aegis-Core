use ank_proto::v1::kernel_service_client::KernelServiceClient;
use ank_proto::v1::Empty;
use ank_proto::v1::Priority;
use ank_proto::v1::TaskRequest;
use ank_proto::v1::TaskSubscription;
use ank_proto::v1::TenantCreateRequest;
use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{self, Command};
use tokio_stream::StreamExt;
use tonic::codegen::InterceptedService;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;

#[derive(Parser, Debug)]
#[command(version, about = "Aegis OS Admin CLI", long_about = None)]
struct Cli {
    #[arg(short, long, global = true, env = "AEGIS_TENANT_ID")]
    tenant: Option<String>,

    #[arg(short, long, global = true, env = "AEGIS_SESSION_KEY")]
    key: Option<String>,

    #[arg(short, long, global = true, default_value = "http://localhost:50051")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Renderiza el estado del servicio + health check HTTP
    Status,
    /// Inicia el servicio AegisOS
    Start,
    /// Detiene el servicio AegisOS
    Stop,
    /// Reinicia el servicio AegisOS
    Restart,
    /// Muestra las últimas líneas del log con opción de follow
    Logs {
        /// Número de líneas a mostrar (default: 50)
        #[arg(default_value = "50")]
        lines: usize,
        /// Mantener el stream de logs activo (follow)
        #[arg(short, long)]
        follow: bool,
    },
    /// Imprime la URL con el setup token de primer arranque
    Token,
    /// Muestra la versión del cliente y del servidor
    Version,
    /// Diagnóstico completo de telemetría y SRE
    Diag,
    /// Actualiza Aegis OS al último release
    Update {
        /// Actualizar a la versión de desarrollo (nightly)
        #[arg(long)]
        beta: bool,
        /// Actualizar a la versión estable
        #[arg(long)]
        stable: bool,
    },

    // ── comandos gRPC legados para retrocompatibilidad ───────────────────────
    /// Renderiza la telemetría del sistema (gRPC)
    GrpcStatus,
    /// Lista los procesos activos en el Kernel
    Ps,
    /// Envía un prompt a la IA y hace streaming de la salida
    Run { prompt: String },
    /// Comandos avanzados de administración (Requiere Master Admin)
    Admin {
        #[command(subcommand)]
        admin_command: AdminCommands,
    },
}

#[derive(Subcommand, Debug)]
enum AdminCommands {
    /// Crea un nuevo Tenant / Enclave
    CreateTenant { name: String },
}

/// Token del interceptor que usaremos para adjuntar los credenciales gRPC
#[derive(Clone)]
struct CitadelInterceptor {
    tenant_id: Option<String>,
    session_key: Option<String>,
}

impl tonic::service::Interceptor for CitadelInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, tonic::Status> {
        if let Some(tenant) = &self.tenant_id {
            if let Ok(val) = MetadataValue::try_from(tenant) {
                request.metadata_mut().insert("x-aegis-tenant-id", val);
            }
        }
        if let Some(key) = &self.session_key {
            if let Ok(val) = MetadataValue::try_from(key) {
                request.metadata_mut().insert("x-aegis-session-key", val);
            }
        }
        Ok(request)
    }
}

type AegisClient = KernelServiceClient<InterceptedService<Channel, CitadelInterceptor>>;

async fn create_client(
    server_url: &str,
    tenant: Option<String>,
    key: Option<String>,
) -> Result<AegisClient> {
    let channel = Channel::from_shared(server_url.to_string())?
        .connect()
        .await
        .context("Failed to connect to the ANK Server")?;

    let interceptor = CitadelInterceptor {
        tenant_id: tenant,
        session_key: key,
    };

    Ok(KernelServiceClient::with_interceptor(channel, interceptor))
}

trait ServiceBackend {
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    fn status(&self) -> Result<()>;
    fn logs(&self, lines: usize, follow: bool) -> Result<()>;
}

#[cfg(target_os = "windows")]
struct WindowsBackend {
    data_dir: PathBuf,
}

#[cfg(target_os = "windows")]
impl WindowsBackend {
    fn new() -> Self {
        let base = std::env::var("AEGIS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("aegis")
            });
        Self { data_dir: base }
    }
}

#[cfg(target_os = "windows")]
impl ServiceBackend for WindowsBackend {
    fn start(&self) -> Result<()> {
        let output = Command::new("sc.exe").args(["start", "AegisOS"]).output()?;
        if output.status.success() {
            println!("{}", "Service start command issued successfully.".green());
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to start service: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    fn stop(&self) -> Result<()> {
        let output = Command::new("sc.exe").args(["stop", "AegisOS"]).output()?;
        if output.status.success() {
            println!("{}", "Service stop command issued successfully.".green());
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to stop service: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    fn restart(&self) -> Result<()> {
        self.stop()?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        self.start()
    }

    fn status(&self) -> Result<()> {
        let output = Command::new("sc.exe").args(["query", "AegisOS"]).output()?;

        println!("{}", "Aegis OS — Status".bold().cyan());
        println!("{}", "─────────────────────────────────────────".cyan());
        let query_out = String::from_utf8_lossy(&output.stdout);
        if query_out.contains("RUNNING") {
            println!("Service:  {}  (AegisOS)", "● Running".green().bold());
        } else if query_out.contains("STOPPED") {
            println!("Service:  {}  (AegisOS)", "● Stopped".red().bold());
        } else {
            println!("Service:  {}  (AegisOS)", "● State Unknown".yellow().bold());
        }

        // Call HTTP health check
        let port = get_http_port();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()?;
        match client
            .get(format!("http://localhost:{}/health", port))
            .send()
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!(
                        "HTTP:     {}   http://localhost:{}",
                        "✓ Online".green(),
                        port
                    );
                    // parse connection-info for Cloudflare Tunnel
                    if let Ok(conn_resp) = client
                        .get(format!(
                            "http://localhost:{}/api/system/connection-info",
                            port
                        ))
                        .send()
                    {
                        if let Ok(json) = conn_resp.json::<serde_json::Value>() {
                            if let Some(tunnel_url) =
                                json.get("tunnel_url").and_then(|v| v.as_str())
                            {
                                println!("Remote:   {}   {}", "✓ Active".green(), tunnel_url);
                            }
                        }
                    }
                } else {
                    println!(
                        "HTTP:     {}",
                        format!("✗ Status Error ({})", resp.status()).red()
                    );
                }
            }
            Err(_) => {
                println!(
                    "HTTP:     {}   http://localhost:{}",
                    "✗ Offline".red(),
                    port
                );
            }
        }
        println!("{}", "─────────────────────────────────────────".cyan());
        Ok(())
    }

    fn logs(&self, lines: usize, follow: bool) -> Result<()> {
        let logs_dir = self.data_dir.join("logs");
        if !logs_dir.exists() {
            return Err(anyhow!(
                "Logs directory does not exist: {}",
                logs_dir.display()
            ));
        }
        // find latest daily ank.log file
        let mut entries = std::fs::read_dir(logs_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("ank.log"))
            .collect::<Vec<_>>();

        entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
        let latest = entries
            .last()
            .ok_or_else(|| anyhow!("No logs found in logs directory"))?;
        let path = latest.path();

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let all_lines = reader.lines().filter_map(|l| l.ok()).collect::<Vec<_>>();
        let start = all_lines.len().saturating_sub(lines);
        for line in &all_lines[start..] {
            println!("{}", line);
        }

        if follow {
            let mut file = File::open(&path)?;
            file.seek(SeekFrom::End(0))?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                    Ok(_) => {
                        print!("{}", line);
                    }
                    Err(e) => {
                        eprintln!("Error reading log: {}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
struct SystemdBackend;

#[cfg(not(target_os = "windows"))]
impl SystemdBackend {
    fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "windows"))]
impl ServiceBackend for SystemdBackend {
    fn start(&self) -> Result<()> {
        let status = Command::new("sudo")
            .args(["systemctl", "start", "aegis"])
            .status()?;
        if status.success() {
            println!("{}", "Service started successfully.".green());
            Ok(())
        } else {
            Err(anyhow!("Failed to start service via systemctl"))
        }
    }

    fn stop(&self) -> Result<()> {
        let status = Command::new("sudo")
            .args(["systemctl", "stop", "aegis"])
            .status()?;
        if status.success() {
            println!("{}", "Service stopped successfully.".green());
            Ok(())
        } else {
            Err(anyhow!("Failed to stop service via systemctl"))
        }
    }

    fn restart(&self) -> Result<()> {
        let status = Command::new("sudo")
            .args(["systemctl", "restart", "aegis"])
            .status()?;
        if status.success() {
            println!("{}", "Service restarted successfully.".green());
            Ok(())
        } else {
            Err(anyhow!("Failed to restart service via systemctl"))
        }
    }

    fn status(&self) -> Result<()> {
        println!("{}", "Aegis OS — Status".bold().cyan());
        println!("{}", "─────────────────────────────────────────".cyan());
        let output = Command::new("systemctl")
            .args(["is-active", "aegis"])
            .output()?;
        let is_active = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if is_active == "active" {
            println!("Service:  {}  (aegis)", "● Running".green().bold());
        } else {
            println!(
                "Service:  {}  (aegis)",
                format!("● Inactive ({})", is_active).red().bold()
            );
        }

        // Call HTTP health check
        let port = get_http_port();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()?;
        match client
            .get(format!("http://localhost:{}/health", port))
            .send()
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    println!(
                        "HTTP:     {}   http://localhost:{}",
                        "✓ Online".green(),
                        port
                    );
                    if let Ok(conn_resp) = client
                        .get(format!(
                            "http://localhost:{}/api/system/connection-info",
                            port
                        ))
                        .send()
                    {
                        if let Ok(json) = conn_resp.json::<serde_json::Value>() {
                            if let Some(tunnel_url) =
                                json.get("tunnel_url").and_then(|v| v.as_str())
                            {
                                println!("Remote:   {}   {}", "✓ Active".green(), tunnel_url);
                            }
                        }
                    }
                } else {
                    println!(
                        "HTTP:     {}",
                        format!("✗ Status Error ({})", resp.status()).red()
                    );
                }
            }
            Err(_) => {
                println!(
                    "HTTP:     {}   http://localhost:{}",
                    "✗ Offline".red(),
                    port
                );
            }
        }
        println!("{}", "─────────────────────────────────────────".cyan());
        Ok(())
    }

    fn logs(&self, lines: usize, follow: bool) -> Result<()> {
        let mut args = vec![
            "journalctl".to_string(),
            "-u".to_string(),
            "aegis".to_string(),
            "-n".to_string(),
            lines.to_string(),
        ];
        if follow {
            args.push("-f".to_string());
        }
        let mut child = Command::new("sudo").args(args).spawn()?;
        child.wait()?;
        Ok(())
    }
}

fn get_http_port() -> u16 {
    // 1. Honor direct environment variables
    if let Ok(p_str) = std::env::var("AEGIS_HTTP_PORT") {
        if let Ok(port) = p_str.parse::<u16>() {
            return port;
        }
    }
    if let Ok(p_str) = std::env::var("ANK_HTTP_PORT") {
        if let Ok(port) = p_str.parse::<u16>() {
            return port;
        }
    }

    // 2. Discover from aegis.env
    let mut env_paths = Vec::new();
    #[cfg(target_os = "windows")]
    {
        if let Ok(s) = std::env::var("AEGIS_DATA_DIR") {
            env_paths.push(PathBuf::from(s).join("aegis.env"));
        }
        env_paths.push(PathBuf::from(r"C:\ProgramData\Aegis\aegis.env"));
        if let Some(d) = dirs::data_dir() {
            env_paths.push(d.join("aegis").join("aegis.env"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(s) = std::env::var("AEGIS_DATA_DIR") {
            env_paths.push(PathBuf::from(s).join("aegis.env"));
        }
        env_paths.push(PathBuf::from("/etc/aegis/aegis.env"));
        env_paths.push(PathBuf::from("/var/lib/aegis/aegis.env"));
    }

    for path in &env_paths {
        if path.exists() {
            if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                for line in reader.lines().filter_map(|l| l.ok()) {
                    let trim = line.trim();
                    if trim.starts_with("AEGIS_HTTP_PORT=") {
                        if let Some(val) = trim.strip_prefix("AEGIS_HTTP_PORT=") {
                            if let Ok(port) = val.trim_matches('"').trim().parse::<u16>() {
                                return port;
                            }
                        }
                    }
                    if trim.starts_with("ANK_HTTP_PORT=") {
                        if let Some(val) = trim.strip_prefix("ANK_HTTP_PORT=") {
                            if let Ok(port) = val.trim_matches('"').trim().parse::<u16>() {
                                return port;
                            }
                        }
                    }
                }
            }
        }
    }

    8000
}

fn get_backend() -> Box<dyn ServiceBackend> {
    #[cfg(target_os = "windows")]
    return Box::new(WindowsBackend::new());
    #[cfg(not(target_os = "windows"))]
    return Box::new(SystemdBackend::new());
}

fn get_setup_token() -> Result<()> {
    let base_dir = std::env::var("AEGIS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            #[cfg(target_os = "windows")]
            let base = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("aegis");
            #[cfg(not(target_os = "windows"))]
            let base = PathBuf::from("/var/lib/aegis");
            base
        });
    let logs_dir = base_dir.join("logs");
    if !logs_dir.exists() {
        return Err(anyhow!(
            "Logs directory does not exist. Aegis might not be initialized yet."
        ));
    }
    let mut entries = std::fs::read_dir(logs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("ank.log"))
        .collect::<Vec<_>>();

    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

    let re = regex::Regex::new(r"setup_token=([a-f0-9]{32})")?;
    for entry in entries.iter().rev() {
        let file = File::open(entry.path())?;
        let reader = BufReader::new(file);
        let lines_vec = reader
            .lines()
            .filter_map(|l| l.ok())
            .collect::<Vec<String>>();
        for line in lines_vec.iter().rev() {
            if let Some(caps) = re.captures(line) {
                let token = &caps[1];
                let hostname = std::env::var("COMPUTERNAME")
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .unwrap_or_else(|_| "localhost".to_string());
                println!(
                    "\n{} http://{}:8000?setup_token={}",
                    "Setup URL:".green().bold(),
                    hostname,
                    token
                );
                return Ok(());
            }
        }
    }
    println!(
        "{}",
        "No active setup token found in the logs. Aegis OS might already be initialized.".yellow()
    );
    Ok(())
}

fn run_diagnostic_report() -> Result<()> {
    println!("{}", "Aegis OS — Diagnostic Report".bold().cyan());
    println!("Generated: {}", chrono::Utc::now().to_rfc3339());
    println!("{}", "─────────────────────────────────────────".cyan());
    println!("SYSTEM");
    println!(
        "  OS:      {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    println!("\nSERVICE");
    let backend = get_backend();
    let _ = backend.status();

    println!("\nRECENT ERRORS (last 20 lines)");
    let base_dir = std::env::var("AEGIS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            #[cfg(target_os = "windows")]
            let base = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("aegis");
            #[cfg(not(target_os = "windows"))]
            let base = PathBuf::from("/var/lib/aegis");
            base
        });
    let logs_dir = base_dir.join("logs");
    if logs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(logs_dir) {
            let mut entries = entries.filter_map(|e| e.ok()).collect::<Vec<_>>();
            entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
            if let Some(latest) = entries.last() {
                if let Ok(file) = File::open(latest.path()) {
                    let reader = BufReader::new(file);
                    let errors = reader
                        .lines()
                        .filter_map(|l| l.ok())
                        .filter(|line| {
                            line.to_uppercase().contains("ERROR")
                                || line.to_uppercase().contains("WARN")
                        })
                        .collect::<Vec<_>>();
                    let start = errors.len().saturating_sub(20);
                    if errors.is_empty() {
                        println!("  [no errors found]");
                    } else {
                        for err in &errors[start..] {
                            println!("  {}", err);
                        }
                    }
                }
            }
        }
    } else {
        println!("  [logs directory not found]");
    }
    println!("{}", "─────────────────────────────────────────".cyan());
    Ok(())
}

fn run_update(beta: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let _ = beta;
        println!("Starting Aegis OS Update on Windows...");
        let cmd = "powershell -ExecutionPolicy Bypass -c \"irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex\"";
        let status = Command::new("powershell")
            .args(["-ExecutionPolicy", "Bypass", "-c", cmd])
            .status()?;
        if status.success() {
            println!("{}", "Aegis OS updated successfully.".green());
            Ok(())
        } else {
            Err(anyhow!("PowerShell installer update failed"))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        println!("Starting Aegis OS Update on Linux...");
        let tag = if beta { "nightly" } else { "latest" };
        let cmd = format!(
            "curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash -s -- --tag {}",
            tag
        );
        let status = Command::new("bash").args(["-c", &cmd]).status()?;
        if status.success() {
            println!("{}", "Aegis OS updated successfully.".green());
            Ok(())
        } else {
            Err(anyhow!("Linux installer update failed"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Zero-Panic SRE policy
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Comprobar si el comando es una operación de servicio local (no requiere conexión gRPC)
    match &cli.command {
        Commands::Start => {
            let backend = get_backend();
            backend.start()?;
            return Ok(());
        }
        Commands::Stop => {
            let backend = get_backend();
            backend.stop()?;
            return Ok(());
        }
        Commands::Restart => {
            let backend = get_backend();
            backend.restart()?;
            return Ok(());
        }
        Commands::Status => {
            let backend = get_backend();
            backend.status()?;
            return Ok(());
        }
        Commands::Logs { lines, follow } => {
            let backend = get_backend();
            backend.logs(*lines, *follow)?;
            return Ok(());
        }
        Commands::Token => {
            get_setup_token()?;
            return Ok(());
        }
        Commands::Diag => {
            run_diagnostic_report()?;
            return Ok(());
        }
        Commands::Update { beta, stable: _ } => {
            run_update(*beta)?;
            return Ok(());
        }
        Commands::Version => {
            println!("Aegis OS Admin CLI v{}", env!("CARGO_PKG_VERSION"));
            let port = get_http_port();
            // Try to get server version via HTTP
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(1))
                .build()?;
            if let Ok(resp) = client
                .get(format!("http://localhost:{}/health", port))
                .send()
            {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(srv_ver) = json.get("version").and_then(|v| v.as_str()) {
                        println!("Aegis OS Kernel v{}", srv_ver);
                    }
                }
            }
            return Ok(());
        }
        _ => {}
    }

    // Comandos gRPC legados
    let mut client = match create_client(&cli.server, cli.tenant.clone(), cli.key.clone()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error connecting to server: {}", e);
            process::exit(1);
        }
    };

    match cli.command {
        Commands::GrpcStatus => match client.get_system_status(Request::new(Empty {})).await {
            Ok(resp) => {
                let status = resp.into_inner();
                println!("========== AEGIS SYSTEM STATUS ==========");
                let state_str = if status.state == 0 {
                    "INITIALIZING"
                } else {
                    "OPERATIONAL"
                };
                println!("State          : {}", state_str);
                println!("CPU Load       : {:.2}%", status.cpu_load * 100.0);
                println!("VRAM Allocated : {:.2} MB", status.vram_allocated_mb);
                println!("VRAM Total     : {:.2} MB", status.vram_total_mb);
                println!("Processes      : {}", status.total_processes);
                println!("Workers        : {}", status.active_workers);
                println!("Uptime         : {}", status.uptime);
                println!("Loaded Models  : {:?}", status.loaded_models);
                println!("=========================================");
            }
            Err(e) => handle_grpc_err(e),
        },
        Commands::Ps => match client.list_processes(Request::new(Empty {})).await {
            Ok(resp) => {
                let list = resp.into_inner();
                if list.processes.is_empty() {
                    println!("No active processes.");
                } else {
                    println!(
                        "{:<15} | {:<15} | {:<20} | {:<10}",
                        "PID", "STATE", "NAME", "PRIO"
                    );
                    println!("{:-<15}-+-{:-<15}-+-{:-<20}-+-{:-<10}", "", "", "", "");
                    for pcb in list.processes {
                        let state_name = match pcb.state {
                            0 => "PENDING",
                            1 => "RUNNING",
                            2 => "BLOCKED",
                            3 => "SUSPENDED",
                            4 => "COMPLETED",
                            5 => "TERMINATED",
                            _ => "UNKNOWN",
                        };
                        println!(
                            "{:<15} | {:<15} | {:<20} | {:<10}",
                            pcb.pid, state_name, pcb.process_name, pcb.priority
                        );
                    }
                }
            }
            Err(e) => handle_grpc_err(e),
        },
        Commands::Run { prompt } => {
            let task_req = TaskRequest {
                prompt,
                priority: Priority::Normal as i32,
                policy: None,
                initial_context: None,
                tenant_id: cli.tenant.clone(),
                task_type: "chat".to_string(),
            };

            let pid = match client.submit_task(Request::new(task_req)).await {
                Ok(resp) => resp.into_inner().pid,
                Err(e) => {
                    handle_grpc_err(e);
                    return Ok(());
                }
            };
            println!("Task submitted. PID: {}", pid);

            let watch_req = TaskSubscription {
                pid: pid.clone(),
                tenant_id: cli.tenant,
            };

            let mut stream = match client.watch_task(Request::new(watch_req)).await {
                Ok(resp) => resp.into_inner(),
                Err(e) => {
                    handle_grpc_err(e);
                    return Ok(());
                }
            };

            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    println!("\n[SRE Guard] Stream cancelled by user (Ctrl+C). Exiting cleanly.");
                    process::exit(0);
                }
            });

            while let Some(message) = stream.next().await {
                match message {
                    Ok(event) => {
                        if let Some(payload) = event.payload {
                            match payload {
                                ank_proto::v1::task_event::Payload::Thought(t) => {
                                    print!("{}", t);
                                }
                                ank_proto::v1::task_event::Payload::Syscall(s) => {
                                    println!("\n[SYSCALL] {} ({:?})", s.name, s.arguments);
                                }
                                ank_proto::v1::task_event::Payload::Output(o) => {
                                    println!("\n[RESULT] {}", o);
                                    break;
                                }
                                ank_proto::v1::task_event::Payload::Error(err) => {
                                    eprintln!("\n[ERROR] {}", err);
                                    break;
                                }
                                ank_proto::v1::task_event::Payload::StatusUpdate(pcb) => {
                                    if pcb.state == 4 || pcb.state == 5 {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        handle_grpc_err(e);
                        break;
                    }
                }
            }
            println!("\nStream completed.");
        }
        Commands::Admin { admin_command } => match admin_command {
            AdminCommands::CreateTenant { name } => {
                let req = TenantCreateRequest { username: name };
                match client.create_tenant(Request::new(req)).await {
                    Ok(resp) => {
                        let result = resp.into_inner();
                        println!("========== TENANT CREATED ==========");
                        println!("Tenant ID  : {}", result.tenant_id);
                        println!("Port       : {}", result.network_port);
                        println!("Passphrase : {}", result.temporary_passphrase);
                        println!("Message    : {}", result.message);
                        println!("====================================");
                    }
                    Err(e) => handle_grpc_err(e),
                }
            }
        },
        _ => {}
    }

    Ok(())
}

fn handle_grpc_err(err: tonic::Status) {
    if err.code() == tonic::Code::Unauthenticated {
        eprintln!("Access denied: Please check your Citadel credentials (AEGIS_TENANT_ID / AEGIS_SESSION_KEY)");
    } else {
        eprintln!("gRPC Error ({}): {}", err.code(), err.message());
    }
}
