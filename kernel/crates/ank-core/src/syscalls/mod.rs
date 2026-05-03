use crate::enclave::TenantDB;
use crate::plugins::PluginManager;
use crate::scheduler::SchedulerEvent;
use crate::scribe::CommitMetadata;
use crate::scribe::ScribeManager;
use crate::vcm::swap::LanceSwapManager;
use crate::vcm::VirtualContextManager;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio::sync::mpsc;

pub mod maker;

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
        }
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
                let result = pm
                    .execute_plugin(
                        tenant_id,
                        &plugin_name,
                        &args_json,
                        pcb.session_key.as_deref(),
                    )
                    .await
                    .map_err(|e: crate::plugins::PluginError| {
                        SyscallError::PluginError(e.to_string())
                    })?;

                Ok(format!("[SYSTEM_RESULT: {}]", result))
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

    async fn search_spotify(
        &self,
        token: &str,
        query: &str,
        max_results: u8,
        _tenant_id: &str,
    ) -> Result<String, SyscallError> {
        let token = match (!token.is_empty()).then(|| token.to_string()) {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Spotify token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
            urlencoding::encode(query),
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("Spotify API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Spotify API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let results: Vec<serde_json::Value> = data["tracks"]["items"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "provider": "spotify",
                            "track_id": t["id"].as_str().unwrap_or(""),
                            "track_uri": t["uri"].as_str().unwrap_or(""),
                            "title": t["name"].as_str().unwrap_or(""),
                            "artist": t["artists"][0]["name"].as_str().unwrap_or(""),
                            "album": t["album"]["name"].as_str().unwrap_or(""),
                            "duration_ms": t["duration_ms"].as_u64().unwrap_or(0),
                            "thumbnail": t["album"]["images"][0]["url"].as_str().unwrap_or(""),
                            "preview_url": t["preview_url"].as_str().unwrap_or(""),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({
                "results": results,
                "provider": "spotify"
            }))
            .unwrap_or_default()
        ))
    }

    async fn search_youtube_oauth(
        &self,
        token: &str,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match (!token.is_empty()).then(|| token.to_string()) {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://www.googleapis.com/youtube/v3/search\
             ?part=snippet&type=video&videoCategoryId=10\
             &q={}&maxResults={}",
            urlencoding::encode(query),
            max_results.clamp(1, 5)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("YouTube API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "YouTube API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let results: Vec<serde_json::Value> = data["items"]
            .as_array()
            .map(|items| {
                items.iter()
                    .map(|item| {
                        serde_json::json!({
                            "provider": "youtube",
                            "video_id": item["id"]["videoId"].as_str().unwrap_or(""),
                            "title": item["snippet"]["title"].as_str().unwrap_or(""),
                            "channel": item["snippet"]["channelTitle"].as_str().unwrap_or(""),
                            "thumbnail": item["snippet"]["thumbnails"]["default"]["url"].as_str().unwrap_or("")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({
                "results": results,
                "provider": "youtube"
            }))
            .unwrap_or_default()
        ))
    }

    async fn google_calendar(
        &self,
        token: Option<String>,
        days: u8,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let now = chrono::Utc::now();
        let time_max = now + chrono::Duration::days(days as i64);
        let time_min_str = now.to_rfc3339();
        let time_max_str = time_max.to_rfc3339();

        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events\
             ?timeMin={}&timeMax={}&maxResults={}&singleEvents=true&orderBy=startTime\
             &fields=items(summary,start,end,location,description,attendees)",
            urlencoding::encode(&time_min_str),
            urlencoding::encode(&time_max_str),
            max_results.clamp(1, 20)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                SyscallError::IOError(format!("Google Calendar API request failed: {}", e))
            })?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Google Calendar API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let events: Vec<serde_json::Value> = data["items"]
            .as_array()
            .map(|items| {
                items.iter().map(|evt| {
                    let start = evt.get("start");
                    let end = evt.get("end");
                    let attendees = evt["attendees"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|a| a["email"].as_str())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    serde_json::json!({
                        "title": evt["summary"].as_str().unwrap_or("Sin título"),
                        "start": start.and_then(|s| s["dateTime"].as_str().or_else(|| s["date"].as_str())),
                        "end": end.and_then(|e| e["dateTime"].as_str().or_else(|| e["date"].as_str())),
                        "location": evt["location"].as_str().unwrap_or(""),
                        "description": evt["description"].as_str().unwrap_or(""),
                        "attendees": attendees
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "events": events })).unwrap_or_default()
        ))
    }

    async fn google_drive(
        &self,
        token: Option<String>,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let q = if query.is_empty() {
            "trashed=false".to_string()
        } else {
            format!(
                "name contains '{}' and trashed=false",
                urlencoding::encode(query)
            )
        };

        let url = format!(
            "https://www.googleapis.com/drive/v3/files\
             ?q={}&fields=files(id,name,mimeType,modifiedTime,webViewLink,size)\
             &orderBy=modifiedTime desc&pageSize={}",
            q,
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                SyscallError::IOError(format!("Google Drive API request failed: {}", e))
            })?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Google Drive API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let files: Vec<serde_json::Value> = data["files"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|f| {
                        let mime = f["mimeType"].as_str().unwrap_or("");
                        let file_type = match mime {
                            "application/vnd.google-apps.spreadsheet" => "spreadsheet",
                            "application/vnd.google-apps.document" => "document",
                            "application/vnd.google-apps.presentation" => "presentation",
                            "application/vnd.google-apps.folder" => "folder",
                            "application/pdf" => "pdf",
                            _ if mime.starts_with("image/") => "image",
                            _ if mime.starts_with("video/") => "video",
                            _ => "file",
                        };

                        serde_json::json!({
                            "id": f["id"].as_str().unwrap_or(""),
                            "name": f["name"].as_str().unwrap_or(""),
                            "type": file_type,
                            "modified": f["modifiedTime"].as_str().unwrap_or(""),
                            "url": f["webViewLink"].as_str().unwrap_or("")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "files": files })).unwrap_or_default()
        ))
    }

    async fn gmail(
        &self,
        token: Option<String>,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = match token {
            Some(t) => t,
            None => {
                return Ok(
                    "[SYSTEM_RESULT: Google token expired. Please reconnect in Settings.]"
                        .to_string(),
                )
            }
        };

        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages\
             ?q={}&maxResults={}",
            urlencoding::encode(query),
            max_results.clamp(1, 10)
        );

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SyscallError::IOError(format!("Gmail API request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SyscallError::IOError(format!(
                "Gmail API error: {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SyscallError::IOError(e.to_string()))?;

        let message_ids: Vec<String> = data["messages"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut emails = Vec::new();
        for id in message_ids {
            let msg_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}\
                 ?fields=payload/headers,snippet",
                id
            );

            let msg_resp = self
                .http_client
                .get(&msg_url)
                .bearer_auth(&token)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await;

            if let Ok(msg_resp) = msg_resp {
                if msg_resp.status().is_success() {
                    if let Ok(msg_data) = msg_resp.json::<serde_json::Value>().await {
                        let headers = msg_data["payload"]["headers"].as_array();
                        let get_header = |name: &str| -> Option<String> {
                            headers?
                                .iter()
                                .find(|h| {
                                    h["name"]
                                        .as_str()
                                        .map(|n| n.to_lowercase() == name.to_lowercase())
                                        .unwrap_or(false)
                                })
                                .and_then(|h| h["value"].as_str().map(String::from))
                        };

                        let from = get_header("from").unwrap_or_default();
                        let subject = get_header("subject").unwrap_or_default();
                        let date = get_header("date").unwrap_or_default();
                        let unread = get_header("status")
                            .map(|s| s.to_lowercase() == "unread")
                            .unwrap_or(false);

                        emails.push(serde_json::json!({
                            "from": from,
                            "subject": subject,
                            "snippet": msg_data["snippet"].as_str().unwrap_or(""),
                            "date": date,
                            "unread": unread
                        }));
                    }
                }
            }
        }

        Ok(format!(
            "[SYSTEM_RESULT: {}]",
            serde_json::to_string(&serde_json::json!({ "emails": emails })).unwrap_or_default()
        ))
    }
}

/// --- REGEX PATTERNS ---
// The patterns below are hardcoded string literals that are valid regex syntax by construction.
// `expect` is the only way to initialize `LazyLock<Regex>` from a `Result`; a failure here
// would indicate a programmer error in the literal, not a runtime condition, making `expect`
// the semantically correct choice. The `#[allow]` is scoped to these four static initialisers.
#[allow(clippy::expect_used)]
static PLUGIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("([^"]+)",\s*(\{.*?\})\)\]"#).unwrap_or_else(|_| {
        panic!("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
    })
});
#[allow(clippy::expect_used)]
static READ_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[READ_FILE\("([^"]+)"\)\]"#).unwrap_or_else(|_| {
        panic!("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
    })
});
#[allow(clippy::expect_used)]
static WRITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[WRITE_FILE\("([^"]+)",\s*"([\s\S]*?)",\s*(\{.*?\})\)\]"#).unwrap_or_else(|_| {
        panic!("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
    })
});
#[allow(clippy::expect_used)]
static MCP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_MCP_EXEC\("([^"]+)",\s*(\{.*?\})\)\]"#).unwrap_or_else(|_| {
        panic!("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
    })
});
#[allow(clippy::expect_used)]
static MUSIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("music_search",\s*(\{.*?\})\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: music syscall regex is invalid"))
});
#[allow(clippy::expect_used)]
static GCAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("google_calendar",\s*(\{.*?\})\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: google_calendar regex is invalid"))
});
#[allow(clippy::expect_used)]
static GDRIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("google_drive",\s*(\{.*?\})\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: google_drive regex is invalid"))
});
#[allow(clippy::expect_used)]
static GMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("gmail",\s*(\{.*?\})\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: gmail regex is invalid"))
});
#[allow(clippy::expect_used)]
static MAKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_MAKER\("([^"]+)",\s*"([\s\S]*?)",\s*(\{.*?\})\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded maker regex is invalid"))
});
// DEPRECATED (Epic 42) — eliminar en post-launch. Reemplazado por SYS_AGENT_SPAWN (Epic 45).
#[allow(clippy::expect_used)]
static SPAWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_SPAWN\("([^"]+)",\s*"([^"]+)"\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded spawn syscall regex is invalid"))
});

/// CORE-169: SYS_EXEC
#[allow(clippy::expect_used)]
static EXEC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{"syscall"\s*:\s*"SYS_EXEC"\s*,\s*"params"\s*:\s*(\{.*?\})\}"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded SYS_EXEC regex is invalid"))
});

/// CORE-172: SYS_GIT_BRANCH
#[allow(clippy::expect_used)]
static GIT_BRANCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{"syscall"\s*:\s*"SYS_GIT_BRANCH"\s*,\s*"params"\s*:\s*(\{.*?\})\}"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded SYS_GIT_BRANCH regex is invalid"))
});

/// CORE-172: SYS_GIT_COMMIT
#[allow(clippy::expect_used)]
static GIT_COMMIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{"syscall"\s*:\s*"SYS_GIT_COMMIT"\s*,\s*"params"\s*:\s*(\{.*?\})\}"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded SYS_GIT_COMMIT regex is invalid"))
});

/// CORE-172: SYS_GIT_PUSH
#[allow(clippy::expect_used)]
static GIT_PUSH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{"syscall"\s*:\s*"SYS_GIT_PUSH"\s*,\s*"params"\s*:\s*(\{.*?\})\}"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded SYS_GIT_PUSH regex is invalid"))
});

/// No-op kept for backwards compatibility. Regexes are now initialized lazily via `LazyLock`.
pub fn init_syscall_regexes() {}

/// Convierte un string de task_type a la variante del enum.
pub fn parse_task_type(s: &str) -> crate::pcb::TaskType {
    match s.to_lowercase().as_str() {
        "code" | "coding" => crate::pcb::TaskType::Code,
        "planning" => crate::pcb::TaskType::Planning,
        "analysis" => crate::pcb::TaskType::Analysis,
        "summarization" => crate::pcb::TaskType::Summarization,
        "creative" => crate::pcb::TaskType::Creative,
        "local" => crate::pcb::TaskType::Local,
        _ => crate::pcb::TaskType::Chat,
    }
}

/// Retorna `true` si el buffer puede ser el inicio de una syscall de texto (legacy token mode).
/// Solo los siguientes prefijos deben retener tokens:
///   - `[SYS_CALL_...` — plugin, music, calendar, drive, gmail, maker, spawn
///   - `[READ_FILE...` / `[WRITE_FILE...` — VCM file ops
///   - `{"syscall":...` — SYS_EXEC, SYS_GIT_* (JSON mode)
///
/// Cualquier otro `[` (markdown links, labels, etc.) se emite sin retener.
fn could_be_syscall_prefix(buffer: &str) -> bool {
    // Caso JSON: {"syscall":...
    if buffer.contains("{\"syscall\"") || buffer.contains("{ \"syscall\"") {
        return true;
    }
    // Caso texto: [SYS_CALL_... o [READ_FILE... o [WRITE_FILE...
    if let Some(bracket_pos) = buffer.rfind('[') {
        let after = &buffer[bracket_pos..];
        return after.starts_with("[SYS_CALL_")
            || after.starts_with("[READ_FILE")
            || after.starts_with("[WRITE_FILE")
            || after.starts_with("[SYS_");
    }
    false
}

/// Parser de Syscalls Cognitivas.
/// Detecta llamadas estructuradas dentro del stream de texto de la IA.
pub fn parse_syscall(text: &str) -> Option<Syscall> {
    // 1. Check for Plugin Call
    if let Some(caps) = PLUGIN_RE.captures(text) {
        return Some(Syscall::PluginCall {
            plugin_name: caps[1].to_string(),
            args_json: caps[2].to_string(),
        });
    }

    // 2. Check for Read File
    if let Some(caps) = READ_RE.captures(text) {
        return Some(Syscall::ReadFile {
            uri: caps[1].to_string(),
        });
    }

    // 3. Check for Write File
    if let Some(caps) = WRITE_RE.captures(text) {
        let uri = caps[1].to_string();
        let content = caps[2].to_string();
        let metadata_json = &caps[3];

        if let Ok(metadata) = serde_json::from_str::<CommitMetadata>(metadata_json) {
            return Some(Syscall::WriteFile {
                uri,
                content,
                metadata,
            });
        }
    }

    // 4. Check for MCP Tool Call
    if let Some(caps) = MCP_RE.captures(text) {
        return Some(Syscall::McpExec {
            tool_name: caps[1].to_string(),
            args_json: caps[2].to_string(),
        });
    }

    // 5. Check for Music Search
    if let Some(caps) = MUSIC_RE.captures(text) {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let max = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u8;
            return Some(Syscall::MusicSearch {
                query,
                max_results: max.clamp(1, 5),
            });
        }
    }

    // 6. Check for Google Calendar
    if let Some(caps) = GCAL_RE.captures(text) {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let days = args["days"].as_u64().unwrap_or(7) as u8;
            let max = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u8;
            return Some(Syscall::GoogleCalendar {
                days: days.clamp(1, 30),
                max_results: max.clamp(1, 20),
            });
        }
    }

    // 7. Check for Google Drive
    if let Some(caps) = GDRIVE_RE.captures(text) {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let max = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as u8;
            return Some(Syscall::GoogleDrive {
                query,
                max_results: max.clamp(1, 10),
            });
        }
    }

    // 8. Check for Gmail
    if let Some(caps) = GMAIL_RE.captures(text) {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let max = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as u8;
            return Some(Syscall::Gmail {
                query,
                max_results: max.clamp(1, 10),
            });
        }
    }

    // 9. Check for Maker Call
    if let Some(caps) = MAKER_RE.captures(text) {
        return Some(Syscall::MakerCall {
            script_type: caps[1].to_string(),
            code: caps[2].to_string(),
            params_json: caps[3].to_string(),
        });
    }

    // 10. Check for Spawn
    if let Some(caps) = SPAWN_RE.captures(text) {
        return Some(Syscall::Spawn {
            task_description: caps[1].to_string(),
            role: caps[2].to_string(),
        });
    }

    // 11. CORE-169: SYS_EXEC
    // {"syscall":"SYS_EXEC","params":{"command":"cargo","args":["build"],"blocking":true}}
    if let Some(caps) = EXEC_RE.captures(text) {
        if let Ok(params) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let command = params["command"].as_str().unwrap_or("").to_string();
            let args = params["args"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let blocking = params["blocking"].as_bool().unwrap_or(true);
            if !command.is_empty() {
                return Some(Syscall::Exec {
                    command,
                    args,
                    blocking,
                });
            }
        }
    }

    // 13. CORE-172: SYS_GIT_BRANCH
    if let Some(caps) = GIT_BRANCH_RE.captures(text) {
        if let Ok(params) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let branch_name = params["branch_name"].as_str().unwrap_or("").to_string();
            let from = params["from"].as_str().unwrap_or("main").to_string();
            if !branch_name.is_empty() {
                return Some(Syscall::GitBranch { branch_name, from });
            }
        }
    }

    // 14. CORE-172: SYS_GIT_COMMIT
    if let Some(caps) = GIT_COMMIT_RE.captures(text) {
        if let Ok(params) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let files = params["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let message = params["message"].as_str().unwrap_or("").to_string();
            if !message.is_empty() {
                return Some(Syscall::GitCommit { files, message });
            }
        }
    }

    // 15. CORE-172: SYS_GIT_PUSH
    if let Some(caps) = GIT_PUSH_RE.captures(text) {
        if let Ok(params) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
            let branch_name = params["branch_name"].as_str().unwrap_or("").to_string();
            if !branch_name.is_empty() {
                return Some(Syscall::GitPush { branch_name });
            }
        }
    }

    None
}

/// --- STREAM INTERCEPTOR ---
/// Detecta syscalls en tiempo real dentro de un stream de tokens.
pub enum StreamItem {
    Token(String),
    Syscall(Syscall),
}

pub struct StreamInterceptor<S> {
    stream: S,
    buffer: String,
    finished: bool,
}

// SAFETY: StreamInterceptor is owned by a single task at a time.
// S is Send, which allows moving between threads. Sync is forced here to satisfy
// tokio::spawn requirements for the composite Future.
unsafe impl<S: Send> Sync for StreamInterceptor<S> {}

impl<S> StreamInterceptor<S>
where
    S: tokio_stream::Stream<Item = Result<String, crate::chal::ExecutionError>>
        + Unpin
        + Send
        + Sync,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: String::new(),
            finished: false,
        }
    }

    pub async fn next_item(&mut self) -> Option<StreamItem> {
        use tokio_stream::StreamExt;

        if self.finished && self.buffer.is_empty() {
            return None;
        }

        // Si tenemos una syscall completa en el buffer, la extraemos
        if let Some(syscall) = parse_syscall(&self.buffer) {
            // Limpiar el buffer hasta el final de la syscall (asumiendo que termina con ])
            if let Some(pos) = self.buffer.find(']') {
                self.buffer.drain(0..=pos);
            } else {
                self.buffer.clear();
            }
            return Some(StreamItem::Syscall(syscall));
        }

        // Si no hay syscall, leemos el siguiente token
        while let Some(res) = self.stream.next().await {
            match res {
                Ok(token) => {
                    self.buffer.push_str(&token);

                    // Solo acumulamos si el buffer parece el inicio de una syscall de texto.
                    // Syscalls de texto: "[SYS_CALL_..." o "{\"syscall\"...".
                    // Cualquier otro "[" (markdown, links, etc.) se emite inmediatamente.
                    if could_be_syscall_prefix(&self.buffer) {
                        // Si ya tenemos el cierre, intentamos parsear
                        if self.buffer.contains(']') || self.buffer.contains('}') {
                            if let Some(syscall) = parse_syscall(&self.buffer) {
                                if let Some(pos) = self.buffer.find(']') {
                                    self.buffer.drain(0..=pos);
                                } else {
                                    self.buffer.clear();
                                }
                                return Some(StreamItem::Syscall(syscall));
                            }
                            // Tiene cierre pero no parsea como syscall — emitir como token normal
                            let content = self.buffer.clone();
                            self.buffer.clear();
                            return Some(StreamItem::Token(content));
                        }
                        // Si no cerramos pero el buffer crece mucho sin cerrar, soltamos como tokens
                        if self.buffer.len() > 2048 {
                            let content = self.buffer.clone();
                            self.buffer.clear();
                            return Some(StreamItem::Token(content));
                        }
                        continue;
                    } else {
                        // Token normal — emitir sin retener
                        let content = self.buffer.clone();
                        self.buffer.clear();
                        return Some(StreamItem::Token(content));
                    }
                }
                Err(_) => {
                    self.finished = true;
                    return None;
                }
            }
        }

        self.finished = true;
        if !self.buffer.is_empty() {
            let content = self.buffer.clone();
            self.buffer.clear();
            Some(StreamItem::Token(content))
        } else {
            None
        }
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
        assert!(matches!(res, Err(SyscallError::PluginError(_))));
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
}
