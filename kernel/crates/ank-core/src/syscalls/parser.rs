use crate::scribe::CommitMetadata;
use crate::syscalls::Syscall;
use regex::Regex;
use std::sync::LazyLock;

/// --- REGEX PATTERNS ---
// The patterns below are hardcoded string literals that are valid regex syntax by construction.
// `expect` is the only way to initialize `LazyLock<Regex>` from a `Result`; a failure here
// would indicate a programmer error in the literal, not a runtime condition, making `expect`
// the semantically correct choice. The `#[allow]` is scoped to these static initialisers.
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

/// Privileged Call: SYS_ENABLE_MODULE
#[allow(clippy::expect_used)]
static ENABLE_MODULE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_ENABLE_MODULE\("([^"]+)"\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: hardcoded SYS_ENABLE_MODULE regex is invalid"))
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
pub(crate) fn could_be_syscall_prefix(buffer: &str) -> bool {
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

    // 16. Privileged check for dynamic module installation/enablement
    if let Some(caps) = ENABLE_MODULE_RE.captures(text) {
        return Some(Syscall::EnableModule {
            module_id: caps[1].to_string(),
        });
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
