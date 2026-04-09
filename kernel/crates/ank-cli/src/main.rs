use ank_proto::v1::kernel_service_client::KernelServiceClient;
use ank_proto::v1::Empty;
use ank_proto::v1::Priority;
use ank_proto::v1::TaskRequest;
use ank_proto::v1::TaskSubscription;
use ank_proto::v1::TenantCreateRequest;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process;
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
    /// Renderiza la telemetría del sistema (CPU, VRAM, Estado)
    Status,
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

#[tokio::main]
async fn main() -> Result<()> {
    // Zero-Panic SRE policy
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Establecer cliente auth
    let mut client = match create_client(&cli.server, cli.tenant.clone(), cli.key.clone()).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error connecting to server: {}", e);
            process::exit(1);
        }
    };

    match cli.command {
        Commands::Status => match client.get_system_status(Request::new(Empty {})).await {
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
            // Task Submission
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

            // Stream Output
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

            // Hook for graceful ctrl-c
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
                                    print!("{}", t); // Print thoughts/token streams directly as text
                                }
                                ank_proto::v1::task_event::Payload::Syscall(s) => {
                                    println!("\n[SYSCALL] {} ({:?})", s.name, s.arguments);
                                }
                                ank_proto::v1::task_event::Payload::Output(o) => {
                                    println!("\n[RESULT] {}", o);
                                    break; // Terminate early when result is ready
                                }
                                ank_proto::v1::task_event::Payload::Error(err) => {
                                    eprintln!("\n[ERROR] {}", err);
                                    break;
                                }
                                ank_proto::v1::task_event::Payload::StatusUpdate(pcb) => {
                                    if pcb.state == 4 || pcb.state == 5 {
                                        // Completed or Terminated
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
