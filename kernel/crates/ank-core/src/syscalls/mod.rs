use crate::plugins::PluginManager;
use crate::scribe::CommitMetadata;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use thiserror::Error;

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
}

impl SyscallExecutor {
    pub fn new(
        plugin_manager: Arc<tokio::sync::RwLock<PluginManager>>,
        vcm: Arc<VirtualContextManager>,
        scribe: Arc<ScribeManager>,
        swap: Arc<LanceSwapManager>,
        mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
    ) -> Self {
        Self {
            plugin_manager,
            vcm,
            scribe,
            swap,
            mcp_registry,
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
                    .execute_plugin(tenant_id, &plugin_name, &args_json)
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
    Regex::new(r#"\[SYS_CALL_PLUGIN\("([^"]+)",\s*(\{.*?\})\)\]"#)
        .expect("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
});
#[allow(clippy::expect_used)]
static READ_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[READ_FILE\("([^"]+)"\)\]"#)
        .expect("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
});
#[allow(clippy::expect_used)]
static WRITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[WRITE_FILE\("([^"]+)",\s*"([\s\S]*?)",\s*(\{.*?\})\)\]"#)
        .expect("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
});
#[allow(clippy::expect_used)]
static MCP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_MCP_EXEC\("([^"]+)",\s*(\{.*?\})\)\]"#)
        .expect("FATAL: hardcoded syscall regex is invalid — this is a compile-time bug")
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
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry);

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
        let executor = SyscallExecutor::new(manager, vcm, scribe, swap, mcp_registry);

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
