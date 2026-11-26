use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

const MAX_STDERR_LINES: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntigravitySdkStatus {
    pub sdk_installed: bool,
    pub version: Option<String>,
    pub mode: String,
    pub active_sessions: Vec<String>,
    pub error: Option<String>,
}

type PendingRequests = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>>;

#[derive(Clone)]
struct WorkerHandle {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingRequests,
    recent_stderr: Arc<Mutex<VecDeque<String>>>,
    alive: Arc<AtomicBool>,
}

/// CORE-384 / CORE-403: Runner que conecta `AgentOrchestrator` con
/// `tools/antigravity_bridge/agy_worker.py` mediante I/O multiplexado por `id`,
/// permitiendo sesiones concurrentes en distintos workspaces y captura de `stderr`.
pub struct AntigravityRunner {
    workspace_root: PathBuf,
    enabled: AtomicBool,
    worker: Mutex<Option<WorkerHandle>>,
    /// Cola de mensajes enviados desde el asistente personal hacia la ventana de Antigravity IDE (CORE-385),
    /// particionada por `tenant_id` (CORE-400).
    ide_outbox: Mutex<HashMap<String, Vec<String>>>,
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
            ide_outbox: Mutex::new(HashMap::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    pub async fn push_ide_outbox(&self, tenant_id: &str, msg: String) {
        let mut map = self.ide_outbox.lock().await;
        map.entry(tenant_id.to_string()).or_default().push(msg);
    }

    pub async fn pop_ide_outbox(&self, tenant_id: &str) -> Option<String> {
        let mut map = self.ide_outbox.lock().await;
        let q = map.get_mut(tenant_id)?;
        if q.is_empty() {
            None
        } else {
            let item = q.remove(0);
            if q.is_empty() {
                map.remove(tenant_id);
            }
            Some(item)
        }
    }

    pub async fn pending_ide_outbox_count(&self, tenant_id: &str) -> usize {
        self.ide_outbox
            .lock()
            .await
            .get(tenant_id)
            .map(|q| q.len())
            .unwrap_or(0)
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

    async fn spawn_worker_handle(&self) -> anyhow::Result<WorkerHandle> {
        let py = self.resolve_python_bin();
        let script = self.resolve_worker_script();
        if !script.exists() {
            anyhow::bail!("No se encontró el worker de Antigravity en {:?}", script);
        }

        info!(
            python = ?py,
            script = ?script,
            "[AntigravityRunner] Iniciando daemon agy_worker.py (stdin/stdout multiplexado)"
        );

        let mut cmd = Command::new(&py);
        cmd.arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PYTHONIOENCODING", "utf-8")
            .kill_on_drop(true);

        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open stdin for agy_worker"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open stdout for agy_worker"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open stderr for agy_worker"))?;

        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        let recent_stderr = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_STDERR_LINES)));
        let alive = Arc::new(AtomicBool::new(true));

        let stderr_buf = Arc::clone(&recent_stderr);
        tokio::spawn(async move {
            let mut err_lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = err_lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    warn!(target: "antigravity_worker_stderr", "[agy_worker stderr] {}", trimmed);
                }
                let mut q = stderr_buf.lock().await;
                if q.len() >= MAX_STDERR_LINES {
                    q.pop_front();
                }
                q.push_back(line);
            }
        });

        let pending_reader = Arc::clone(&pending);
        let alive_reader = Arc::clone(&alive);
        let stderr_reader = Arc::clone(&recent_stderr);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(raw_line)) = lines.next_line().await {
                let Ok(ev) = serde_json::from_str::<Value>(&raw_line) else {
                    continue;
                };
                let Some(ev_id) = ev.get("id").and_then(|v| v.as_str()).map(str::to_string) else {
                    continue;
                };
                let tx_opt = {
                    let map = pending_reader.lock().await;
                    map.get(&ev_id).cloned()
                };
                if let Some(tx) = tx_opt {
                    if tx.send(ev).is_err() {
                        pending_reader.lock().await.remove(&ev_id);
                    }
                }
            }

            alive_reader.store(false, Ordering::SeqCst);
            let pending_count = {
                let mut map = pending_reader.lock().await;
                let n = map.len();
                map.drain();
                n
            };
            let tail = {
                let q = stderr_reader.lock().await;
                q.iter()
                    .rev()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            if pending_count > 0 || !tail.is_empty() {
                warn!(
                    pending_aborted = pending_count,
                    stderr_tail = %tail,
                    "[AntigravityRunner] stdout de agy_worker cerrado inesperadamente"
                );
            }
        });

        Ok(WorkerHandle {
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            recent_stderr,
            alive,
        })
    }

    async fn get_or_spawn_worker(&self) -> anyhow::Result<WorkerHandle> {
        let mut guard = self.worker.lock().await;
        if let Some(existing) = guard.as_ref() {
            let is_alive = existing.alive.load(Ordering::SeqCst);
            let exited = existing
                .child
                .lock()
                .await
                .try_wait()
                .ok()
                .flatten()
                .is_some();
            if is_alive && !exited {
                return Ok(existing.clone());
            }
            existing.alive.store(false, Ordering::SeqCst);
            existing.pending.lock().await.drain();
            *guard = None;
        }

        let handle = self.spawn_worker_handle().await?;
        *guard = Some(handle.clone());
        Ok(handle)
    }

    async fn invalidate_worker(&self, dead_handle: &WorkerHandle) {
        dead_handle.alive.store(false, Ordering::SeqCst);
        dead_handle.pending.lock().await.drain();
        {
            let mut child = dead_handle.child.lock().await;
            let _ = child.kill().await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), child.wait()).await;
        }
        let mut guard = self.worker.lock().await;
        if let Some(current) = guard.as_ref() {
            if Arc::ptr_eq(&current.pending, &dead_handle.pending) {
                *guard = None;
            }
        }
    }

    async fn stderr_summary(handle: &WorkerHandle) -> String {
        let q = handle.recent_stderr.lock().await;
        q.iter()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ")
    }

    async fn send_request(
        &self,
        handle: &WorkerHandle,
        req_id: &str,
        payload: Value,
    ) -> anyhow::Result<mpsc::UnboundedReceiver<Value>> {
        let (tx, rx) = mpsc::unbounded_channel();
        handle.pending.lock().await.insert(req_id.to_string(), tx);

        let line = payload.to_string() + "\n";
        let write_res = {
            let mut stdin = handle.stdin.lock().await;
            match stdin.write_all(line.as_bytes()).await {
                Ok(()) => stdin.flush().await,
                Err(e) => Err(e),
            }
        };

        if let Err(e) = write_res {
            handle.pending.lock().await.remove(req_id);
            self.invalidate_worker(handle).await;
            anyhow::bail!("Fallo al escribir a agy_worker: {}", e);
        }

        Ok(rx)
    }

    /// Apaga limpiamente el subproceso `agy_worker.py` (CORE-403).
    pub async fn shutdown(&self) {
        let handle = self.worker.lock().await.take();
        if let Some(h) = handle {
            h.alive.store(false, Ordering::SeqCst);
            h.pending.lock().await.drain();
            let mut child = h.child.lock().await;
            let _ = child.kill().await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await;
        }
    }

    pub async fn check_status(&self) -> AntigravitySdkStatus {
        let handle = match self.get_or_spawn_worker().await {
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
        let mut rx = match self
            .send_request(
                &handle,
                &req_id,
                json!({
                    "id": req_id,
                    "cmd": "status"
                }),
            )
            .await
        {
            Ok(rx) => rx,
            Err(e) => {
                return AntigravitySdkStatus {
                    sdk_installed: false,
                    version: None,
                    mode: "error".to_string(),
                    active_sessions: Vec::new(),
                    error: Some(e.to_string()),
                };
            }
        };

        let read_res = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
        handle.pending.lock().await.remove(&req_id);

        match read_res {
            Ok(Some(val)) => {
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
            }
            _ => {
                let err_tail = Self::stderr_summary(&handle).await;
                self.invalidate_worker(&handle).await;
                let msg = if err_tail.is_empty() {
                    "Timeout consultando agy_worker.py".to_string()
                } else {
                    format!("Timeout consultando agy_worker.py (stderr: {})", err_tail)
                };
                AntigravitySdkStatus {
                    sdk_installed: false,
                    version: None,
                    mode: "timeout".to_string(),
                    active_sessions: Vec::new(),
                    error: Some(msg),
                }
            }
        }
    }

    /// Envía un turno de conversación (`chat`) a la sesión `session_id` de Antigravity SDK
    /// usando `self.workspace_root` y multiplexado por `id`.
    pub async fn dispatch_turn<F>(
        &self,
        session_id: &str,
        system_instructions: &str,
        prompt: &str,
        on_activity: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(String),
    {
        self.dispatch_turn_in_workspace(
            &self.workspace_root,
            session_id,
            system_instructions,
            prompt,
            on_activity,
        )
        .await
    }

    /// Envía un turno de conversación (`chat`) asociado a `workspace` sin retener un lock global
    /// durante el streaming, permitiendo sesiones concurrentes en múltiples workspaces (CORE-403).
    pub async fn dispatch_turn_in_workspace<F>(
        &self,
        workspace: &Path,
        session_id: &str,
        system_instructions: &str,
        prompt: &str,
        mut on_activity: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(String),
    {
        let handle = self.get_or_spawn_worker().await?;
        let req_id = format!("chat-{}", uuid::Uuid::new_v4());
        let payload = json!({
            "id": req_id,
            "cmd": "chat",
            "session_id": session_id,
            "workspace": workspace.to_string_lossy(),
            "system_instructions": system_instructions,
            "prompt": prompt,
            "allow_write": true,
        });

        let mut rx = self.send_request(&handle, &req_id, payload).await?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
        let mut accumulated_tokens = String::new();

        loop {
            let next_ev = tokio::time::timeout_at(deadline, rx.recv()).await;
            let ev = match next_ev {
                Ok(Some(ev)) => ev,
                Ok(None) => {
                    handle.pending.lock().await.remove(&req_id);
                    let tail = Self::stderr_summary(&handle).await;
                    self.invalidate_worker(&handle).await;
                    if tail.is_empty() {
                        anyhow::bail!("El proceso agy_worker se cerró inesperadamente.");
                    } else {
                        anyhow::bail!("El proceso agy_worker se cerró inesperadamente: {}", tail);
                    }
                }
                Err(_) => {
                    handle.pending.lock().await.remove(&req_id);
                    anyhow::bail!("Timeout esperando respuesta de Antigravity SDK");
                }
            };

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
                    handle.pending.lock().await.remove(&req_id);
                    let full = ev
                        .get("full_text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| accumulated_tokens.trim().to_string());
                    return Ok(full);
                }
                "error" => {
                    handle.pending.lock().await.remove(&req_id);
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
        runner.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_antigravity_runner_concurrent_multi_workspace_and_shutdown() -> anyhow::Result<()>
    {
        std::env::set_var("AEGIS_AGY_MOCK", "1");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(&manifest_dir);

        let tmp = tempfile::tempdir()?;
        let ws_a = tmp.path().join("workspace-alpha");
        let ws_b = tmp.path().join("workspace-beta");
        std::fs::create_dir_all(&ws_a)?;
        std::fs::create_dir_all(&ws_b)?;

        let runner = AntigravityRunner::new(repo_root);
        let (res_a, res_b) = tokio::join!(
            runner.dispatch_turn_in_workspace(&ws_a, "sess-alpha", "Inst", "Hola alpha", |_| {}),
            runner.dispatch_turn_in_workspace(&ws_b, "sess-beta", "Inst", "Hola beta", |_| {}),
        );

        let reply_a = res_a?;
        let reply_b = res_b?;
        assert!(
            reply_a.contains("workspace-alpha"),
            "Expected workspace-alpha tag in reply_a: {}",
            reply_a
        );
        assert!(
            reply_b.contains("workspace-beta"),
            "Expected workspace-beta tag in reply_b: {}",
            reply_b
        );

        runner.shutdown().await;
        assert!(runner.worker.lock().await.is_none());
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

    #[tokio::test]
    async fn test_ide_outbox_is_partitioned_by_tenant() {
        let runner = AntigravityRunner::new(Path::new("."));
        runner
            .push_ide_outbox("alice", "msg_alice".to_string())
            .await;
        runner.push_ide_outbox("bob", "msg_bob".to_string()).await;

        assert_eq!(runner.pending_ide_outbox_count("alice").await, 1);
        assert_eq!(runner.pending_ide_outbox_count("bob").await, 1);
        assert_eq!(
            runner.pop_ide_outbox("alice").await.as_deref(),
            Some("msg_alice")
        );
        assert!(runner.pop_ide_outbox("alice").await.is_none());
        assert_eq!(runner.pending_ide_outbox_count("bob").await, 1);
        assert_eq!(
            runner.pop_ide_outbox("bob").await.as_deref(),
            Some("msg_bob")
        );
    }
}
