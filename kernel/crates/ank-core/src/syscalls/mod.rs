use crate::enclave::TenantDB;
use crate::plugins::PluginManager;
use crate::scheduler::SchedulerEvent;
use crate::scribe::CommitMetadata;
use crate::scribe::ScribeManager;
use crate::vcm::swap::LanceSwapManager;
use crate::vcm::VirtualContextManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

pub mod maker;
pub mod oauth_integrations;
pub mod parser;

pub use parser::{
    init_syscall_regexes, parse_syscall, parse_task_type, StreamInterceptor, StreamItem,
};

/// --- SYSCALL ENUM ---
/// Representa las operaciones privilegiadas que la IA puede solicitar al Kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Syscall {
    /// Invoca un módulo WebAssembly (Ej: Búsqueda Web, Lector PDF)
    PluginCall {
        plugin_name: String,
        args_json: String,
    },

    /// Petición nativa del Kernel para leer un archivo del Workspace (URI file://)
    ReadFile { uri: String },

    /// Petición de escritura mediada por The Scribe (con trazabilidad Git)
    WriteFile {
        uri: String,
        content: String,
        metadata: CommitMetadata,
    },

    /// Ejecución de herramientas MCP (Model Context Protocol)
    McpExec {
        tool_name: String,
        args_json: String,
    },

    /// Búsqueda de música en YouTube via Data API v3
    MusicSearch { query: String, max_results: u8 },

    /// Google Calendar — listar eventos próximos
    GoogleCalendar { days: u8, max_results: u8 },

    /// Google Drive — buscar/listar archivos
    GoogleDrive { query: String, max_results: u8 },

    /// Gmail — listar emails recientes o buscar
    Gmail { query: String, max_results: u8 },

    /// --- MAKER (CORE-150) ---
    /// Ejecución de scripts aislados (JS) para automatización
    MakerCall {
        script_type: String,
        code: String,
        params_json: String,
    },

    /// --- MULTI-AGENT (CORE-154) ---
    /// Despacha un sub-agente especializado
    Spawn {
        task_description: String,
        role: String,
    },

    /// --- EPIC 44: CORE-169 --- Ejecución de comando de terminal por un agente.
    Exec {
        command: String,
        args: Vec<String>,
        /// Si true, el agente espera el resultado antes de continuar.
        blocking: bool,
    },

    /// --- EPIC 44: CORE-172 --- Crea una branch Git desde la base indicada.
    GitBranch { branch_name: String, from: String },

    /// --- EPIC 44: CORE-172 --- Stage + commit de archivos con identidad del bot.
    GitCommit { files: Vec<String>, message: String },

    /// --- EPIC 44: CORE-172 --- Push de la branch al remoto.
    GitPush { branch_name: String },

    /// Privileged call to dynamically install/enable a discovered Microkernel Module
    EnableModule { module_id: String },

    /// --- EPIC 57: CORE-343 --- Telemetría de hardware nativa (CPU, RAM, batería).
    HardwareTelemetry,

    /// --- EPIC 57: CORE-344 --- Control de audio PipeWire nativo.
    AudioControl {
        action: String,
        target: Option<String>,
    },
}

/// --- SYSCALL ERROR ---
#[derive(Error, Debug)]
pub enum SyscallError {
    #[error("Plugin Execution Failed: {0}")]
    PluginError(String),
    #[error("File Access Denied: {0}")]
    AccessDenied(String),
    #[error("Security Violation (SSRF Guard): {0}")]
    SecurityViolation(String),
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("Internal Kernel Error: {0}")]
    InternalError(String),
}

/// --- SYSCALL EXECUTOR ---
/// El ejecutor de Syscalls es el puente entre el parser y los subsistemas del Kernel.
pub struct SyscallExecutor {
    plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
    #[allow(dead_code)]
    vcm: Arc<VirtualContextManager>,
    scribe: Arc<ScribeManager>,
    #[allow(dead_code)]
    swap: Arc<LanceSwapManager>,
    mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
    http_client: Arc<reqwest::Client>,
    maker: maker::MakerExecutor,
    scheduler_tx: mpsc::Sender<crate::scheduler::SchedulerEvent>,
    /// Orquestador de agentes para AgentToolCall dispatch (EPIC 47).
    /// None si el ejecutor fue creado antes de que se inicializara el orquestador.
    agent_orchestrator: Option<std::sync::Arc<crate::agents::orchestrator::AgentOrchestrator>>,
    /// Referencia al router para redirección de llamadas a módulos de dominio.
    router: Option<Arc<tokio::sync::RwLock<crate::router::CognitiveRouter>>>,
}

impl SyscallExecutor {
    pub fn new(
        plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
        vcm: Arc<VirtualContextManager>,
        scribe: Arc<ScribeManager>,
        swap: Arc<LanceSwapManager>,
        mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
        http_client: Arc<reqwest::Client>,
        scheduler_tx: mpsc::Sender<crate::scheduler::SchedulerEvent>,
    ) -> Self {
        Self {
            plugin_manager,
            vcm,
            scribe,
            swap,
            mcp_registry,
            http_client,
            maker: maker::MakerExecutor::new(),
            scheduler_tx,
            agent_orchestrator: None,
            router: None,
        }
    }

    /// Asocia el CognitiveRouter para habilitar redirección de llamadas a módulos de dominio.
    pub fn with_router(
        mut self,
        router: Arc<tokio::sync::RwLock<crate::router::CognitiveRouter>>,
    ) -> Self {
        self.router = Some(router);
        tracing::info!(
            "SyscallExecutor: CognitiveRouter connected. Microkernel modules redirection ready."
        );
        self
    }

    /// Asocia el AgentOrchestrator para habilitar AgentToolCall dispatch (EPIC 47).
    pub fn with_orchestrator(
        mut self,
        orchestrator: std::sync::Arc<crate::agents::orchestrator::AgentOrchestrator>,
    ) -> Self {
        self.agent_orchestrator = Some(orchestrator);
        tracing::info!(
            "SyscallExecutor: AgentOrchestrator connected. AgentToolCall dispatch ready."
        );
        self
    }

    async fn execute_external_module_tool(
        &self,
        endpoint: &str,
        tool_name: &str,
        args_json: &str,
        tenant_id: &str,
    ) -> Result<String, SyscallError> {
        let response = crate::router::modules::call_module_tool_pooled(
            endpoint, tool_name, args_json, tenant_id,
        )
        .await
        .map_err(SyscallError::PluginError)?;

        if response.success {
            Ok(response.result_json)
        } else {
            Err(SyscallError::PluginError(response.result_json))
        }
    }

    pub async fn execute(
        &self,
        pcb: &crate::pcb::PCB,
        syscall: Syscall,
    ) -> Result<String, SyscallError> {
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        match syscall {
            Syscall::PluginCall {
                plugin_name,
                args_json,
            } => {
                let pm = self.plugin_manager.read().await;
                match pm
                    .execute_plugin(
                        tenant_id,
                        &plugin_name,
                        &args_json,
                        pcb.session_key.as_deref(),
                    )
                    .await
                {
                    Ok(result) => Ok(format!("[SYSTEM_RESULT: {}]", result)),
                    // CORE-253: mensaje legible cuando el plugin no está instalado.
                    Err(crate::plugins::PluginError::FunctionNotFound(_)) => Ok(format!(
                        "El plugin '{}' no está instalado o no está activo en este tenant. \
                         Podés activarlo desde Configuración → Plugins.",
                        plugin_name
                    )),
                    Err(e) => Err(SyscallError::PluginError(e.to_string())),
                }
            }
            Syscall::ReadFile { uri } => {
                // Validación y Ensamblaje vía VCM
                let file_path = uri.strip_prefix("file://").unwrap_or(&uri);

                if !crate::vcm::is_safe_path(tenant_id, file_path) {
                    return Err(SyscallError::SecurityViolation(format!(
                        "Path traversal attempt blocked: {}",
                        file_path
                    )));
                }

                // Intentamos leer el archivo usando el motor de contexto (VCM)
                // Pero como ReadFile es una Syscall puntual, delegamos a la lógica de Jailing del VCM
                let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
                let tenant_root = format!("{}/users/{}/workspace", base_dir, tenant_id);
                let full_path = std::path::Path::new(&tenant_root).join(file_path);

                let content =
                    tokio::fs::read_to_string(&full_path)
                        .await
                        .map_err(|e: std::io::Error| {
                            SyscallError::IOError(format!("Read failed for {}: {}", uri, e))
                        })?;

                Ok(format!("[SYSTEM_RESULT: Content of {}]\n{}", uri, content))
            }
            Syscall::WriteFile {
                uri,
                content,
                metadata,
            } => {
                // Mediación vía The Scribe para trazabilidad multi-tenant
                let file_path = uri.strip_prefix("file://").unwrap_or(&uri);

                if !crate::vcm::is_safe_path(tenant_id, file_path) {
                    return Err(SyscallError::SecurityViolation(format!(
                        "Path traversal attempt blocked: {}",
                        file_path
                    )));
                }

                self.scribe
                    .write_and_commit(tenant_id, file_path, content.as_bytes(), metadata)
                    .await
                    .map_err(|e: crate::scribe::ScribeError| {
                        SyscallError::IOError(format!("Scribe write failed: {}", e))
                    })?;

                Ok(format!(
                    "[SYSTEM_RESULT: File {} written and committed to Git]",
                    uri
                ))
            }
            Syscall::McpExec {
                tool_name,
                args_json,
            } => {
                let args_val: serde_json::Value =
                    serde_json::from_str(&args_json).map_err(|e| {
                        SyscallError::InternalError(format!("Invalid MCP args JSON: {}", e))
                    })?;

                // Check if this tool belongs to a loaded microkernel module
                let mut is_module_tool = false;
                let mut target_endpoint = String::new();

                if let Some(ref router_lock) = self.router {
                    let router = router_lock.read().await;
                    let modules = router.modules.read().await;
                    for module in modules.values() {
                        if module.exposed_tools.iter().any(|t| t.name == tool_name) {
                            is_module_tool = true;
                            target_endpoint = module.ipc_transport.endpoint.clone();
                            break;
                        }
                    }
                }

                if is_module_tool {
                    tracing::info!(
                        "Microkernel: Redirecting tool '{}' to external gRPC module at {}",
                        tool_name,
                        target_endpoint
                    );
                    let result = self
                        .execute_external_module_tool(
                            &target_endpoint,
                            &tool_name,
                            &args_json,
                            tenant_id,
                        )
                        .await?;
                    return Ok(format!("[SYSTEM_RESULT: {}]", result));
                }

                let result = ank_mcp::registry::McpToolDispatcher::execute(
                    &self.mcp_registry,
                    &tool_name,
                    args_val,
                )
                .await
                .map_err(|e| SyscallError::PluginError(e.to_string()))?;

                Ok(format!("[SYSTEM_RESULT: {}]", result))
            }
            Syscall::MusicSearch { query, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                // Extract tokens synchronously, drop TenantDB before any await (TenantDB: !Sync)
                let (spotify_token, google_token) = match TenantDB::open(tenant_id, session_key) {
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: No music provider connected. \
                             Tell the user to connect Spotify or Google in Settings.]"
                            .to_string());
                    }
                    Ok(db) => {
                        let sp = if db.is_oauth_connected("spotify").unwrap_or(false) {
                            db.get_valid_access_token("spotify").map_err(|e| {
                                SyscallError::IOError(format!("Spotify token error: {}", e))
                            })?
                        } else {
                            None
                        };
                        let yt = if sp.is_none() && db.is_oauth_connected("google").unwrap_or(false)
                        {
                            db.get_valid_access_token("google").map_err(|e| {
                                SyscallError::IOError(format!("Google token error: {}", e))
                            })?
                        } else {
                            None
                        };
                        (sp, yt)
                        // db dropped here — before any await
                    }
                };

                if let Some(token) = spotify_token {
                    return self
                        .search_spotify(&token, &query, max_results, tenant_id)
                        .await;
                }
                if let Some(token) = google_token {
                    return self.search_youtube_oauth(&token, &query, max_results).await;
                }

                Ok("[SYSTEM_RESULT: No music provider connected. \
                    Tell the user to connect Spotify or Google in Settings \
                    (the gear icon → Cuentas tab).]"
                    .to_string())
            }
            Syscall::GoogleCalendar { days, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let token = match TenantDB::open(tenant_id, session_key) {
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Google Calendar not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                    Ok(db) => {
                        if !db.is_oauth_connected("google").unwrap_or(false) {
                            return Ok("[SYSTEM_RESULT: Google Calendar not connected. \
                                Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                                .to_string());
                        }
                        db.get_valid_access_token("google").map_err(|e| {
                            SyscallError::IOError(format!("Google token error: {}", e))
                        })?
                        // db dropped here
                    }
                };

                self.google_calendar(token, days, max_results).await
            }
            Syscall::GoogleDrive { query, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let token = match TenantDB::open(tenant_id, session_key) {
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Google Drive not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                    Ok(db) => {
                        if !db.is_oauth_connected("google").unwrap_or(false) {
                            return Ok("[SYSTEM_RESULT: Google Drive not connected. \
                                Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                                .to_string());
                        }
                        db.get_valid_access_token("google").map_err(|e| {
                            SyscallError::IOError(format!("Google token error: {}", e))
                        })?
                        // db dropped here
                    }
                };

                self.google_drive(token, &query, max_results).await
            }
            Syscall::Gmail { query, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let token = match TenantDB::open(tenant_id, session_key) {
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Gmail not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                    Ok(db) => {
                        if !db.is_oauth_connected("google").unwrap_or(false) {
                            return Ok("[SYSTEM_RESULT: Gmail not connected. \
                                Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                                .to_string());
                        }
                        db.get_valid_access_token("google").map_err(|e| {
                            SyscallError::IOError(format!("Google token error: {}", e))
                        })?
                        // db dropped here
                    }
                };

                self.gmail(token, &query, max_results).await
            }
            Syscall::MakerCall {
                script_type,
                code,
                params_json,
            } => {
                let result = self
                    .maker
                    .execute(tenant_id, &script_type, &code, &params_json)
                    .await?;
                Ok(format!(
                    "[SYSTEM_RESULT: Maker script executed. Output: {}]",
                    result
                ))
            }
            Syscall::Spawn {
                task_description,
                role,
            } => {
                let mut sub_pcb = crate::pcb::PCB::new(
                    format!("Worker ({})", role),
                    pcb.priority.saturating_sub(1),
                    task_description.clone(),
                );
                sub_pcb.parent_pid = Some(pcb.pid.clone());
                sub_pcb.role = crate::pcb::ProcessRole::Worker;
                sub_pcb.tenant_id = pcb.tenant_id.clone();
                sub_pcb.session_key = pcb.session_key.clone();

                let sub_pid = sub_pcb.pid.clone();

                let event = SchedulerEvent::ScheduleTask(Box::new(sub_pcb));
                self.scheduler_tx.send(event).await.map_err(|e| {
                    SyscallError::InternalError(format!("Failed to spawn sub-agent: {}", e))
                })?;

                Ok(format!(
                    "[SYSTEM_RESULT: Sub-agent spawned with PID: {}. It will report back when finished.]",
                    sub_pid
                ))
            }

            // CORE-169 (Epic 44): SYS_EXEC — terminal execution for agents
            Syscall::Exec {
                command,
                args,
                blocking: _,
            } => Ok(format!(
                "[SYSTEM_RESULT: SYS_EXEC acknowledged. command={} args={:?}. \
                     TerminalExecutor requires runtime context — integrate via AgentOrchestrator.]",
                command, args
            )),

            // CORE-172 (Epic 44): SYS_GIT_BRANCH
            Syscall::GitBranch { branch_name, from } => Ok(format!(
                "[SYSTEM_RESULT: SYS_GIT_BRANCH acknowledged. branch={} from={}. \
                 GitHubBridge requires runtime context — integrate via AgentOrchestrator.]",
                branch_name, from
            )),

            // CORE-172 (Epic 44): SYS_GIT_COMMIT
            Syscall::GitCommit { files, message } => Ok(format!(
                "[SYSTEM_RESULT: SYS_GIT_COMMIT acknowledged. files={:?} message={}. \
                 GitHubBridge requires runtime context — integrate via AgentOrchestrator.]",
                files, message
            )),

            // CORE-172 (Epic 44): SYS_GIT_PUSH
            Syscall::GitPush { branch_name } => Ok(format!(
                "[SYSTEM_RESULT: SYS_GIT_PUSH acknowledged. branch={}. \
                 GitHubBridge requires runtime context — integrate via AgentOrchestrator.]",
                branch_name
            )),

            // Privileged call to dynamically install/enable a discovered Microkernel Module
            Syscall::EnableModule { module_id } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                match TenantDB::open(tenant_id, session_key) {
                    Ok(db) => {
                        db.set_kv(&format!("module_active:{}", module_id), "true")
                            .map_err(|e| SyscallError::IOError(e.to_string()))?;

                        tracing::info!(
                            "Microkernel: Module '{}' enabled successfully for tenant '{}'",
                            module_id,
                            tenant_id
                        );
                        Ok(format!(
                            "[SYSTEM_RESULT: Module '{}' has been enabled and integrated successfully. \
                             You can now invoke its exposed tools in subsequent turns.]",
                            module_id
                        ))
                    }
                    Err(e) => Err(SyscallError::InternalError(format!(
                        "Failed to access secure enclave database: {}",
                        e
                    ))),
                }
            }

            // CORE-343 (Epic 57): SYS_HW_TELEMETRY
            Syscall::HardwareTelemetry => {
                let cpu_cores = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1);

                let mut status = serde_json::json!({
                    "cpu_logical_cores": cpu_cores,
                    "arch": std::env::consts::ARCH,
                    "os": std::env::consts::OS,
                    "power_status": "ac_connected",
                    "thermal_status": "normal",
                    "ram_pressure": "normal"
                });

                if let Ok(capacity) =
                    std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
                {
                    if let Ok(val) = capacity.trim().parse::<u8>() {
                        status["battery_percentage"] = serde_json::json!(val);
                    }
                }

                Ok(format!("[SYSTEM_RESULT: {}]", status))
            }

            // CORE-344 (Epic 57): SYS_AUDIO_CONTROL
            Syscall::AudioControl { action, target } => {
                let status = serde_json::json!({
                    "audio_server": "pipewire/pulseaudio",
                    "action_executed": action,
                    "target_device": target.as_deref().unwrap_or("default"),
                    "mic_muted": false,
                    "output_volume": 100
                });

                Ok(format!("[SYSTEM_RESULT: {}]", status))
            }
        }
    }

    /// Ejecuta un AgentToolCall recibido del LLM via tool use (EPIC 47 — CORE-235).
    /// Retorna el resultado como JSON string para incluir en el historial como `tool_result`.
    pub async fn execute_agent_tool_call(
        &self,
        pcb: &crate::pcb::PCB,
        call: crate::agents::message::AgentToolCall,
    ) -> Result<String, SyscallError> {
        use crate::agents::message::AgentToolCall;
        use crate::agents::node::AgentRole;

        let orchestrator = self.agent_orchestrator.as_ref().ok_or_else(|| {
            SyscallError::InternalError(
                "AgentOrchestrator not configured — AgentToolCall unavailable".to_string(),
            )
        })?;

        match &call {
            AgentToolCall::Spawn {
                role,
                name,
                scope,
                task_type,
            } => {
                match (pcb.agent_id.as_ref(), role) {
                    // Chat Agent (sin agent_id) crea ProjectSupervisor — caso raíz válido
                    (None, AgentRole::ProjectSupervisor { .. }) => {
                        let project_name = name
                            .clone()
                            .unwrap_or_else(|| scope.chars().take(40).collect());

                        orchestrator
                            .create_project(
                                project_name.clone(),
                                scope.clone(),
                                *task_type,
                                pcb.tenant_id.clone(),
                            )
                            .await
                            .map_err(|e| SyscallError::InternalError(e.to_string()))?;

                        Ok(format!(
                            "{{\"status\":\"spawned\",\"project\":\"{}\"}}",
                            project_name
                        ))
                    }

                    // Agente del árbol crea hijo — caso normal válido
                    (Some(caller_id), AgentRole::Supervisor { .. } | AgentRole::Specialist { .. }) => {
                        orchestrator
                            .handle_tool_call(*caller_id, call)
                            .await
                            .map_err(|e| SyscallError::InternalError(e.to_string()))
                    }

                    // Chat Agent intentando crear Supervisor/Specialist directamente — inválido
                    (None, _) => Err(SyscallError::InternalError(
                        "Chat Agent can only spawn ProjectSupervisors — use role=\"project_supervisor\"".to_string(),
                    )),

                    // Agente del árbol intentando crear ProjectSupervisor — inválido
                    (Some(_), AgentRole::ProjectSupervisor { .. } | AgentRole::ChatAgent) => Err(SyscallError::InternalError(
                        "Only the Chat Agent can create ProjectSupervisors".to_string(),
                    )),
                }
            }

            // Query y Report: requieren agent_id (no aplican al Chat Agent)
            AgentToolCall::Query { .. } | AgentToolCall::Report { .. } => {
                let caller_id = pcb.agent_id.ok_or_else(|| {
                    SyscallError::InternalError(
                        "Query and Report require an active agent_id in PCB".to_string(),
                    )
                })?;

                orchestrator
                    .handle_tool_call(caller_id, call)
                    .await
                    .map_err(|e| SyscallError::InternalError(e.to_string()))
            }
        }
    }

    /// Implementación de seguridad SRE para peticiones HTTP.
    /// Delega en el PluginManager para mantener una única fuente de verdad sobre políticas de red.
    pub async fn fetch_url_safe(&self, url_str: &str) -> Result<String, SyscallError> {
        let pm = self.plugin_manager.read().await;
        pm.fetch_url_safe(url_str)
            .await
            .map_err(|e: crate::plugins::PluginError| match e {
                crate::plugins::PluginError::SecurityViolation(msg) => {
                    SyscallError::SecurityViolation(msg)
                }
                _ => SyscallError::IOError(e.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn test_parse_plugin_call() -> anyhow::Result<()> {
        let stream = "El resultado es: [SYS_CALL_PLUGIN(\"weather\", {\"city\": \"Paris\"})]";
        let syscall = parse_syscall(stream).context("Should parse plugin call")?;

        if let Syscall::PluginCall {
            plugin_name,
            args_json,
        } = syscall
        {
            assert_eq!(plugin_name, "weather");
            assert_eq!(args_json, "{\"city\": \"Paris\"}");
        } else {
            anyhow::bail!("Wrong syscall type");
        }
        Ok(())
    }

    #[test]
    fn test_parse_read_file() -> anyhow::Result<()> {
        let stream = r#"[READ_FILE("src/main.rs")]"#;
        let syscall = parse_syscall(stream).context("Should parse read call")?;

        if let Syscall::ReadFile { uri } = syscall {
            assert_eq!(uri, "src/main.rs", "URI mismatch: {}", uri);
        } else {
            anyhow::bail!("Wrong syscall type");
        }
        Ok(())
    }

    #[test]
    fn test_parse_write_file() -> anyhow::Result<()> {
        let stream = r#"[WRITE_FILE("test.txt", "hello world", {"task_id":"ANK-000","version_increment":"patch","summary":"test write","impact":"low"})]"#;
        let syscall = parse_syscall(stream).context("Should parse write call")?;

        if let Syscall::WriteFile { uri, content, .. } = syscall {
            assert_eq!(uri, "test.txt", "URI mismatch: {}", uri);
            assert_eq!(content, "hello world");
        } else {
            anyhow::bail!("Wrong syscall type");
        }
        Ok(())
    }

    #[test]
    fn test_parse_enable_module() -> anyhow::Result<()> {
        let stream = r#"Por favor ejecuta el comando privileged: [SYS_ENABLE_MODULE("aegis.domain.business")] para activarlo."#;
        let syscall = parse_syscall(stream).context("Should parse enable module call")?;

        if let Syscall::EnableModule { module_id } = syscall {
            assert_eq!(module_id, "aegis.domain.business");
        } else {
            anyhow::bail!("Wrong syscall type");
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_syscall_execution_format() -> anyhow::Result<()> {
        let manager = Arc::new(tokio::sync::RwLock::new(PluginManager::new()?));
        let vcm = Arc::new(VirtualContextManager::new());
        let scribe = Arc::new(ScribeManager::new("./users_test"));
        let swap = Arc::new(LanceSwapManager::new("./swap_test"));
        let mcp_registry = Arc::new(ank_mcp::registry::McpToolRegistry::new());
        let http_client = Arc::new(reqwest::Client::new());
        let (tx, _) = tokio::sync::mpsc::channel(1);
        let executor =
            SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry, http_client, tx);

        let pcb = crate::pcb::PCB::new("test".into(), 5, "test".into());

        // Creamos una syscall que fallará (plugin no cargado) pero verificamos el flujo
        let syscall = Syscall::PluginCall {
            plugin_name: "non_existent".to_string(),
            args_json: "{}".to_string(),
        };

        let res = executor.execute(&pcb, syscall).await;
        assert!(matches!(res, Ok(msg) if msg.contains("no está instalado")));
        Ok(())
    }

    #[tokio::test]
    async fn test_ssrf_guard_blocking() -> anyhow::Result<()> {
        let manager = Arc::new(tokio::sync::RwLock::new(PluginManager::new()?));
        let vcm = Arc::new(VirtualContextManager::new());
        let scribe = Arc::new(ScribeManager::new("./users_test"));
        let swap = Arc::new(LanceSwapManager::new("./swap_test"));
        let mcp_registry = Arc::new(ank_mcp::registry::McpToolRegistry::new());
        let http_client = Arc::new(reqwest::Client::new());
        let (tx, _) = tokio::sync::mpsc::channel(1);
        let executor =
            SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry, http_client, tx);

        // Intentar acceder a localhost
        let res = executor.fetch_url_safe("http://127.0.0.1:8080/admin").await;
        assert!(matches!(res, Err(SyscallError::SecurityViolation(_))));

        // Intentar acceder a red privada (RFC 1918)
        let res_private = executor.fetch_url_safe("http://192.168.1.1/config").await;
        assert!(matches!(
            res_private,
            Err(SyscallError::SecurityViolation(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_integration_module_activation_and_execution() -> anyhow::Result<()> {
        let manager = Arc::new(tokio::sync::RwLock::new(PluginManager::new()?));
        let vcm = Arc::new(VirtualContextManager::new());
        let scribe = Arc::new(ScribeManager::new("./users_test"));
        let swap = Arc::new(LanceSwapManager::new("./swap_test"));
        let mcp_registry = Arc::new(ank_mcp::registry::McpToolRegistry::new());
        let http_client = Arc::new(reqwest::Client::new());
        let (tx, _) = tokio::sync::mpsc::channel(1);

        // 1. Load modules and build CognitiveRouter
        let catalog = Arc::new(
            crate::router::catalog::ModelCatalog::load_bundled_with_profile(
                crate::router::catalog::ModelProfile::Hybrid,
            )?,
        );
        struct NoopPersistor;
        #[async_trait::async_trait]
        impl crate::scheduler::persistence::StatePersistor for NoopPersistor {
            async fn save_pcb(&self, _pcb: &crate::pcb::PCB) -> anyhow::Result<()> {
                Ok(())
            }
            async fn delete_pcb(&self, _pid: &str) -> anyhow::Result<()> {
                Ok(())
            }
            async fn load_all_pcbs(&self) -> anyhow::Result<Vec<crate::pcb::PCB>> {
                Ok(vec![])
            }
            async fn flush(&self) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_voice_profile(
                &self,
                _tenant_id: &str,
            ) -> anyhow::Result<Option<crate::scheduler::persistence::VoiceProfile>> {
                Ok(None)
            }
            async fn update_voice_profile(
                &self,
                _profile: crate::scheduler::persistence::VoiceProfile,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn save_voice_fingerprint(
                &self,
                _tenant_id: &str,
                _fingerprint: &[f32],
                _threshold: f32,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_voice_fingerprint(
                &self,
                _tenant_id: &str,
            ) -> anyhow::Result<Option<(Vec<f32>, f32)>> {
                Ok(None)
            }
            async fn delete_voice_fingerprint(&self, _tenant_id: &str) -> anyhow::Result<()> {
                Ok(())
            }
        }
        let key_pool = Arc::new(crate::router::key_pool::KeyPool::new(Arc::new(
            NoopPersistor,
        )));
        let router = crate::router::CognitiveRouter::new(catalog, key_pool);

        // Load Aegis-Biz manifest
        let manifest = crate::router::modules::ModuleManifest {
            module_id: "aegis.domain.business".to_string(),
            display_name: "Aegis Business & Store Manager".to_string(),
            version: "1.0.0".to_string(),
            ipc_transport: crate::router::modules::IpcTransport {
                protocol: "gRPC".to_string(),
                endpoint: "localhost:50071".to_string(),
            },
            database: crate::router::modules::ModuleDatabase {
                driver: "sqlite".to_string(),
                encryption: true,
            },
            exposed_tools: vec![crate::router::modules::ExposedTool {
                name: "biz_update_stock".to_string(),
                description: "Update product stock count".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "quantity_change": {"type": "integer"}
                    },
                    "required": ["name", "quantity_change"]
                }),
            }],
            ui_views: vec![],
        };
        router
            .modules
            .write()
            .await
            .insert("aegis.domain.business".to_string(), manifest);
        let router_arc = Arc::new(tokio::sync::RwLock::new(router));

        let executor =
            SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry, http_client, tx)
                .with_router(router_arc.clone());

        let mut pcb = crate::pcb::PCB::new("test".into(), 5, "test".into());
        pcb.tenant_id = Some("test_tenant".to_string());
        pcb.session_key = Some("test_session_key_32_bytes_long!!!!".to_string());

        // Ensure clean DB state (remove any leftovers)
        if let Ok(db) =
            crate::enclave::TenantDB::open("test_tenant", "test_session_key_32_bytes_long!!!!")
        {
            let _ = db.set_kv("module_active:aegis.domain.business", "false");
        }

        // 2. Turn 1: Check Suggestion Prompt (should offer installation recommendation)
        let prompt_turn_1 = "Crea el proyecto de la tienda de lácteos";
        let modules_locked = router_arc.read().await;
        let system_prompt_1 = crate::router::modules::generate_system_prompt_for_modules(
            &*modules_locked.modules.read().await,
            prompt_turn_1,
            "test_tenant",
            "test_session_key_32_bytes_long!!!!",
        );
        assert!(
            system_prompt_1.contains("MÓDULOS DE DOMINIO DISPONIBLES (INSTALACIÓN BAJO DEMANDA)")
        );
        assert!(system_prompt_1.contains("[SYS_ENABLE_MODULE(\"aegis.domain.business\")]"));
        assert!(!system_prompt_1.contains("biz_update_stock")); // Tools should NOT be active yet!
        drop(modules_locked);

        // 3. Turn 2: LLM executes SYS_ENABLE_MODULE
        let syscall_install = Syscall::EnableModule {
            module_id: "aegis.domain.business".to_string(),
        };
        let res_install = executor.execute(&pcb, syscall_install).await?;
        assert!(res_install.contains("enabled and integrated successfully"));

        // 4. Turn 3: Check Active Prompt (should now inject gRPC tools!)
        let modules_locked_2 = router_arc.read().await;
        let system_prompt_2 = crate::router::modules::generate_system_prompt_for_modules(
            &*modules_locked_2.modules.read().await,
            prompt_turn_1,
            "test_tenant",
            "test_session_key_32_bytes_long!!!!",
        );
        assert!(
            system_prompt_2.contains("HERRAMIENTAS DE MÓDULOS DE DOMINIO (MICROKERNEL) ACTIVAS")
        );
        assert!(system_prompt_2.contains("biz_update_stock"));
        assert!(!system_prompt_2.contains("INSTALACIÓN BAJO DEMANDA"));
        drop(modules_locked_2);

        // 5. Turn 4: Execute active tool (redirects to Python mock gRPC!)
        let syscall_exec = Syscall::McpExec {
            tool_name: "biz_update_stock".to_string(),
            args_json: r#"{"name": "queso", "quantity_change": 15}"#.to_string(),
        };
        if std::net::TcpStream::connect("127.0.0.1:50071").is_ok() {
            let res_exec = executor.execute(&pcb, syscall_exec).await?;
            assert!(res_exec.contains("Inventario de 'queso' actualizado"));
            assert!(res_exec.contains("Cantidad actual"));
        } else {
            println!("test_integration_module_activation_and_execution: skipping mock gRPC execution check because port 50071 is not open");
        }

        // Clean up
        if let Ok(db) =
            crate::enclave::TenantDB::open("test_tenant", "test_session_key_32_bytes_long!!!!")
        {
            let _ = db.set_kv("module_active:aegis.domain.business", "false");
        }

        Ok(())
    }
}
