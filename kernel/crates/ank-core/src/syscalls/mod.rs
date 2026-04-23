use crate::enclave::TenantDB;
use crate::plugins::PluginManager;
use crate::scribe::CommitMetadata;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use thiserror::Error;

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

use crate::scribe::ScribeManager;
use crate::vcm::swap::LanceSwapManager;
use crate::vcm::VirtualContextManager;

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
}

impl SyscallExecutor {
    pub fn new(
        plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
        vcm: Arc<VirtualContextManager>,
        scribe: Arc<ScribeManager>,
        swap: Arc<LanceSwapManager>,
        mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
        http_client: Arc<reqwest::Client>,
    ) -> Self {
        Self {
            plugin_manager,
            vcm,
            scribe,
            swap,
            mcp_registry,
            http_client,
            maker: maker::MakerExecutor::new(),
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
                let tenant_root = format!("./users/{}/workspace", tenant_id);
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

                let db = match TenantDB::open(tenant_id, session_key) {
                    Ok(db) => db,
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: No music provider connected. \
                             Tell the user to connect Spotify or Google in Settings.]"
                            .to_string());
                    }
                };

                if db.is_oauth_connected("spotify").unwrap_or(false) {
                    return self
                        .search_spotify(&db, &query, max_results, tenant_id)
                        .await;
                }

                if db.is_oauth_connected("google").unwrap_or(false) {
                    return self.search_youtube_oauth(&db, &query, max_results).await;
                }

                Ok("[SYSTEM_RESULT: No music provider connected. \
                    Tell the user to connect Spotify or Google in Settings \
                    (the gear icon → Cuentas tab).]"
                    .to_string())
            }
            Syscall::GoogleCalendar { days, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let db = match TenantDB::open(tenant_id, session_key) {
                    Ok(db) => db,
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Google Calendar not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                };

                if !db.is_oauth_connected("google").unwrap_or(false) {
                    return Ok("[SYSTEM_RESULT: Google Calendar not connected. \
                        Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                        .to_string());
                }

                self.google_calendar(&db, days, max_results).await
            }
            Syscall::GoogleDrive { query, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let db = match TenantDB::open(tenant_id, session_key) {
                    Ok(db) => db,
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Google Drive not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                };

                if !db.is_oauth_connected("google").unwrap_or(false) {
                    return Ok("[SYSTEM_RESULT: Google Drive not connected. \
                        Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                        .to_string());
                }

                self.google_drive(&db, &query, max_results).await
            }
            Syscall::Gmail { query, max_results } => {
                let session_key = pcb.session_key.as_deref().unwrap_or("default");
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let db = match TenantDB::open(tenant_id, session_key) {
                    Ok(db) => db,
                    Err(_) => {
                        return Ok("[SYSTEM_RESULT: Gmail not available. \
                            Tell the user to connect Google in Settings.]"
                            .to_string());
                    }
                };

                if !db.is_oauth_connected("google").unwrap_or(false) {
                    return Ok("[SYSTEM_RESULT: Gmail not connected. \
                        Tell the user to connect Google in Settings (gear icon → Cuentas tab).]"
                        .to_string());
                }

                self.gmail(&db, &query, max_results).await
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
        db: &TenantDB,
        query: &str,
        max_results: u8,
        _tenant_id: &str,
    ) -> Result<String, SyscallError> {
        let token = db
            .get_valid_access_token("spotify")
            .map_err(|e| SyscallError::IOError(format!("Spotify token error: {}", e)))?;

        let token = match token {
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
        db: &TenantDB,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = db
            .get_valid_access_token("google")
            .map_err(|e| SyscallError::IOError(format!("Google token error: {}", e)))?;

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
        db: &TenantDB,
        days: u8,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = db
            .get_valid_access_token("google")
            .map_err(|e| SyscallError::IOError(format!("Google token error: {}", e)))?;

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
        db: &TenantDB,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = db
            .get_valid_access_token("google")
            .map_err(|e| SyscallError::IOError(format!("Google token error: {}", e)))?;

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
        db: &TenantDB,
        query: &str,
        max_results: u8,
    ) -> Result<String, SyscallError> {
        let token = db
            .get_valid_access_token("google")
            .map_err(|e| SyscallError::IOError(format!("Google token error: {}", e)))?;

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

/// --- STREAM INTERCEPTOR (REAL-TIME) ---
/// Esta estructura se encarga de analizar el stream de tokens mientras se generan
/// para detectar triggers ([SYS) y detener la inferencia inmediatamente.
pub struct StreamInterceptor {
    buffer: String,
    trigger_detected: bool,
    max_buffer_size: usize,
}

impl Default for StreamInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub enum InterceptorResult {
    Continue,
    PossibleSyscall,       // Detectamos el inicio '[' o '[SYS'
    SyscallReady(Syscall), // Ya tenemos la syscall completa
}

impl StreamInterceptor {
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(512),
            trigger_detected: false,
            max_buffer_size: 1024, // Ventana de seguridad
        }
    }

    /// Procesa un nuevo token y decide si se debe abortar la inferencia.
    pub fn push_token(&mut self, token: &str) -> InterceptorResult {
        self.buffer.push_str(token);

        // Si el buffer crece demasiado sin detectar nada, lo limpiamos manteniendo el final
        if self.buffer.len() > self.max_buffer_size {
            let drain_amount = self.buffer.len() - self.max_buffer_size;
            self.buffer.drain(..drain_amount);
        }

        // Detección de Trigger inicial
        if !self.trigger_detected {
            // Buscamos patrones conocidos de Syscall
            if self.buffer.contains("[")
                && (self.buffer.contains("[SYS")
                    || self.buffer.contains("[READ")
                    || self.buffer.contains("[WRITE"))
            {
                self.trigger_detected = true;
                return InterceptorResult::PossibleSyscall;
            }
            InterceptorResult::Continue
        } else {
            // Ya detectamos un trigger, buscamos el cierre ']'
            if self.buffer.contains(']') {
                if let Some(syscall) = parse_syscall(&self.buffer) {
                    return InterceptorResult::SyscallReady(syscall);
                }
            }
            InterceptorResult::PossibleSyscall
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
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

/// No-op kept for backwards compatibility. Regexes are now initialized lazily via `LazyLock`.
pub fn init_syscall_regexes() {}

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

    None
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
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry, http_client);

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
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry, http_client);

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
