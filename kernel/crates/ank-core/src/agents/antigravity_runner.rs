use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravitySdkStatus {
    pub sdk_installed: bool,
    pub version: Option<String>,
    pub mode: String,
    pub active_sessions: Vec<String>,
    pub error: Option<String>,
}

struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout_lines: Lines<BufReader<ChildStdout>>,
}

/// CORE-384: Runner que conecta `AgentOrchestrator` con `tools/antigravity_bridge/agy_worker.py`
/// para delegar subagentes/supervisores al SDK oficial `google-antigravity`, aprovechando
/// los créditos de la cuenta de Antigravity del usuario y manteniendo sesiones vivas por `session_id`.
pub struct AntigravityRunner {
    workspace_root: PathBuf,
    enabled: AtomicBool,
    worker: Mutex<Option<WorkerProcess>>,
    /// Cola de mensajes enviados desde el asistente personal hacia la ventana de Antigravity IDE (CORE-385).
    ide_outbox: Mutex<Vec<String>>,
}

impl AntigravityRunner {
    pub fn new(workspace_root: &Path) -> Self {
        let default_enabled = std::env::var("AEGIS_SUBAGENT_BACKEND")
            .map(|v| v.to_lowercase() != "internal" && v.to_lowercase() != "router")
            .unwrap_or(true);

        Self {
            workspace_root: workspace_root.to_path_buf(),
            enabled: AtomicBool::new(default_enabled),
            worker: Mutex::new(None),
            ide_outbox: Mutex::new(Vec::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    pub async fn push_ide_outbox(&self, msg: String) {
        let mut q = self.ide_outbox.lock().await;
        q.push(msg);
    }

    pub async fn pop_ide_outbox(&self) -> Option<String> {
        let mut q = self.ide_outbox.lock().await;
        if q.is_empty() {
            None
        } else {
            Some(q.remove(0))
        }
    }

    pub async fn pending_ide_outbox_count(&self) -> usize {
        self.ide_outbox.lock().await.len()
    }

    fn resolve_worker_script_with(
        explicit_bridge_dir: Option<&str>,
        data_dir: Option<&Path>,
        workspace_root: &Path,
        cwd: Option<&Path>,
        exe_path: Option<&Path>,
    ) -> PathBuf {
        if let Some(raw) = explicit_bridge_dir {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.is_file() {
                    return p;
                }
                let in_dir = p.join("agy_worker.py");
                if in_dir.exists() {
                    return in_dir;
                }
            }
        }

        let candidate = workspace_root
            .join("tools")
            .join("antigravity_bridge")
            .join("agy_worker.py");
        if candidate.exists() {
            return candidate;
        }
        // Fallback: buscar subiendo hasta 3 niveles por si workspace_root apunta a un subdirectorio
        let mut cur = workspace_root;
        for _ in 0..3 {
            if let Some(parent) = cur.parent() {
                let alt = parent
                    .join("tools")
                    .join("antigravity_bridge")
                    .join("agy_worker.py");
                if alt.exists() {
                    return alt;
                }
                cur = parent;
            }
        }

        if let Some(cwd_path) = cwd {
            let cwd_cand = cwd_path
                .join("tools")
                .join("antigravity_bridge")
                .join("agy_worker.py");
            if cwd_cand.exists() {
                return cwd_cand;
            }
            if let Some(parent) = cwd_path.parent() {
                let parent_cand = parent
                    .join("tools")
                    .join("antigravity_bridge")
                    .join("agy_worker.py");
                if parent_cand.exists() {
                    return parent_cand;
                }
            }
        }

        if let Some(exe) = exe_path {
            for anc in exe.ancestors().take(5) {
                let tools_cand = anc
                    .join("tools")
                    .join("antigravity_bridge")
                    .join("agy_worker.py");
                if tools_cand.exists() {
                    return tools_cand;
                }
                let direct_cand = anc.join("antigravity_bridge").join("agy_worker.py");
                if direct_cand.exists() {
                    return direct_cand;
                }
            }
        }

        if let Some(data) = data_dir {
            let data_cand = data.join("antigravity_bridge").join("agy_worker.py");
            if data_cand.exists() {
                return data_cand;
            }
        }

        PathBuf::from("tools/antigravity_bridge/agy_worker.py")
    }

    fn resolve_worker_script(&self) -> PathBuf {
        let explicit = std::env::var("AEGIS_ANTIGRAVITY_BRIDGE_DIR").ok();
        let data_dir = std::env::var("AEGIS_DATA_DIR")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let cwd = std::env::current_dir().ok();
        let exe = std::env::current_exe().ok();
        Self::resolve_worker_script_with(
            explicit.as_deref(),
            data_dir.as_deref(),
            &self.workspace_root,
            cwd.as_deref(),
            exe.as_deref(),
        )
    }

    fn resolve_python_bin(&self) -> PathBuf {
        let script = self.resolve_worker_script();
        let mut search_roots = Vec::new();
        if let Some(bridge_dir) = script.parent() {
            search_roots.push(bridge_dir);
        }
        let repo_root = script
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(self.workspace_root.as_path());
        search_roots.push(repo_root);
        search_roots.push(self.workspace_root.as_path());

        for root in search_roots {
            let candidates = [
                root.join(".venv-agy").join("Scripts").join("python.exe"),
                root.join(".venv-agy").join("bin").join("python"),
                root.join(".venv").join("Scripts").join("python.exe"),
                root.join(".venv").join("bin").join("python"),
            ];
            for c in candidates {
                if c.exists() {
                    return c;
                }
            }
        }

        if cfg!(windows) {
            PathBuf::from("python")
        } else {
            PathBuf::from("python3")
        }
    }

    async fn ensure_worker<'a>(
        &self,
        guard: &'a mut Option<WorkerProcess>,
    ) -> anyhow::Result<&'a mut WorkerProcess> {
        let needs_spawn = match guard.as_mut() {
            Some(wp) => wp.child.try_wait().ok().flatten().is_some(),
            None => true,
        };

        if needs_spawn {
            let py = self.resolve_python_bin();
            let script = self.resolve_worker_script();
            if !script.exists() {
                anyhow::bail!("No se encontró el worker de Antigravity en {:?}", script);
            }

            info!(
                python = ?py,
                script = ?script,
                "[AntigravityRunner] Iniciando daemon agy_worker.py"
            );

            let mut cmd = Command::new(&py);
            cmd.arg(&script)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .env("PYTHONIOENCODING", "utf-8");

            let mut child = cmd.spawn()?;
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to open stdin for agy_worker"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to open stdout for agy_worker"))?;

            *guard = Some(WorkerProcess {
                child,
                stdin,
                stdout_lines: BufReader::new(stdout).lines(),
            });
        }

        guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("WorkerProcess unavailable"))
    }

    pub async fn check_status(&self) -> AntigravitySdkStatus {
        let mut guard = self.worker.lock().await;
        let wp = match self.ensure_worker(&mut guard).await {
            Ok(w) => w,
            Err(e) => {
                return AntigravitySdkStatus {
                    sdk_installed: false,
                    version: None,
                    mode: "unavailable".to_string(),
                    active_sessions: Vec::new(),
                    error: Some(e.to_string()),
                };
            }
        };

        let req_id = format!("st-{}", uuid::Uuid::new_v4());
        let cmd_line = json!({
            "id": req_id,
            "cmd": "status"
        })
        .to_string()
            + "\n";

        if let Err(e) = wp.stdin.write_all(cmd_line.as_bytes()).await {
            *guard = None;
            return AntigravitySdkStatus {
                sdk_installed: false,
                version: None,
                mode: "error".to_string(),
                active_sessions: Vec::new(),
                error: Some(e.to_string()),
            };
        }
        let _ = wp.stdin.flush().await;

        let read_res = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            wp.stdout_lines.next_line(),
        )
        .await;

        match read_res {
            Ok(Ok(Some(line))) => {
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    let sdk_installed = val
                        .get("sdk_installed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let version = val
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let mode = val
                        .get("mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let active_sessions = val
                        .get("active_sessions")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    AntigravitySdkStatus {
                        sdk_installed,
                        version,
                        mode,
                        active_sessions,
                        error: None,
                    }
                } else {
                    AntigravitySdkStatus {
                        sdk_installed: false,
                        version: None,
                        mode: "parse_error".to_string(),
                        active_sessions: Vec::new(),
                        error: Some("Respuesta inválida del worker".to_string()),
                    }
                }
            }
            _ => {
                *guard = None;
                AntigravitySdkStatus {
                    sdk_installed: false,
                    version: None,
                    mode: "timeout".to_string(),
                    active_sessions: Vec::new(),
                    error: Some("Timeout consultando agy_worker.py".to_string()),
                }
            }
        }
    }

    /// Envía un turno de conversación (`chat`) a la sesión `session_id` de Antigravity SDK
    /// y reporta pensamientos (`thought`) y ejecuciones de herramientas (`tool_call`) en tiempo real.
    pub async fn dispatch_turn<F>(
        &self,
        session_id: &str,
        system_instructions: &str,
        prompt: &str,
        mut on_activity: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(String),
    {
        let mut guard = self.worker.lock().await;
        let wp = self.ensure_worker(&mut guard).await?;

        let req_id = format!("chat-{}", uuid::Uuid::new_v4());
        let payload = json!({
            "id": req_id,
            "cmd": "chat",
            "session_id": session_id,
            "workspace": self.workspace_root.to_string_lossy(),
            "system_instructions": system_instructions,
            "prompt": prompt,
            "allow_write": true,
        });

        let line = payload.to_string() + "\n";
        if let Err(e) = wp.stdin.write_all(line.as_bytes()).await {
            *guard = None;
            anyhow::bail!("Fallo al escribir a agy_worker: {}", e);
        }
        wp.stdin.flush().await?;

        let mut accumulated_tokens = String::new();

        while let Ok(Some(raw_line)) = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            wp.stdout_lines.next_line(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout esperando respuesta de Antigravity SDK"))?
        {
            let Ok(ev) = serde_json::from_str::<Value>(&raw_line) else {
                continue;
            };
            let ev_id = ev.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if ev_id != req_id {
                continue;
            }

            let ev_type = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
            match ev_type {
                "thought" => {
                    if let Some(txt) = ev.get("text").and_then(|v| v.as_str()) {
                        let preview: String = txt.chars().take(180).collect();
                        on_activity(format!("💭 Antigravity: {}", preview));
                    }
                }
                "tool_call" => {
                    let name = ev.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                    on_activity(format!("🛠️ Antigravity ejecutando `{}`", name));
                }
                "token" => {
                    if let Some(tok) = ev.get("text").and_then(|v| v.as_str()) {
                        accumulated_tokens.push_str(tok);
                    }
                }
                "completed" => {
                    let full = ev
                        .get("full_text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| accumulated_tokens.trim().to_string());
                    return Ok(full);
                }
                "error" => {
                    let code = ev.get("code").and_then(|v| v.as_str()).unwrap_or("ERROR");
                    let msg = ev
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Error desconocido en Antigravity SDK");
                    warn!(code = %code, "[AntigravityRunner] SDK error: {}", msg);
                    anyhow::bail!("{}: {}", code, msg);
                }
                _ => {}
            }
        }

        *guard = None;
        anyhow::bail!("El proceso agy_worker se cerró inesperadamente.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_antigravity_runner_mock_session() -> anyhow::Result<()> {
        std::env::set_var("AEGIS_AGY_MOCK", "1");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Subir desde kernel/crates/ank-core hasta la raíz del repo
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(&manifest_dir);

        let runner = AntigravityRunner::new(repo_root);
        let st = runner.check_status().await;
        assert!(st.sdk_installed, "Expected mock SDK installed: {:?}", st);

        let mut activities = Vec::new();
        let reply1 = runner
            .dispatch_turn(
                "test-session-rs",
                "Sos Antigravity",
                "Revisá el archivo README",
                |act| activities.push(act),
            )
            .await?;

        assert!(reply1.contains("turno #1"));
        assert!(!activities.is_empty());

        let reply2 = runner
            .dispatch_turn(
                "test-session-rs",
                "Sos Antigravity",
                "Continuá con el paso 2",
                |_| {},
            )
            .await?;

        assert!(reply2.contains("turno #2"));
        Ok(())
    }

    #[test]
    fn test_resolve_worker_script_from_explicit_bridge_dir() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let bridge_dir = tmp.path().join("custom_bridge");
        std::fs::create_dir_all(&bridge_dir)?;
        let script = bridge_dir.join("agy_worker.py");
        std::fs::write(&script, "# mock\n")?;

        let resolved = AntigravityRunner::resolve_worker_script_with(
            Some(bridge_dir.to_string_lossy().as_ref()),
            None,
            tmp.path(),
            Some(tmp.path()),
            None,
        );
        assert_eq!(resolved, script);
        Ok(())
    }

    #[test]
    fn test_resolve_worker_script_from_data_dir() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let data_dir = tmp.path().join("data");
        let bridge_dir = data_dir.join("antigravity_bridge");
        std::fs::create_dir_all(&bridge_dir)?;
        let script = bridge_dir.join("agy_worker.py");
        std::fs::write(&script, "# mock\n")?;

        let empty_ws = tmp.path().join("ws");
        std::fs::create_dir_all(&empty_ws)?;
        let fake_exe = tmp.path().join("bin").join("ank-server");

        let resolved = AntigravityRunner::resolve_worker_script_with(
            None,
            Some(&data_dir),
            &empty_ws,
            Some(&empty_ws),
            Some(&fake_exe),
        );
        assert_eq!(resolved, script);
        Ok(())
    }
}
