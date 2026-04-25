use serde::Serialize;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum LineKind {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalLine {
    pub kind: LineKind,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct TerminalExecutor {
    project_root: PathBuf,
    allowed_commands: Vec<String>,
    timeout_secs: u64,
}

impl TerminalExecutor {
    pub fn new(project_root: PathBuf, allowed_commands: Vec<String>) -> Self {
        Self {
            project_root,
            allowed_commands,
            timeout_secs: 300,
        }
    }

    /// Ejecuta el comando y streama output via `tx`. Retorna exit code.
    pub async fn exec(
        &self,
        command: &str,
        args: &[&str],
        tx: mpsc::Sender<TerminalLine>,
    ) -> anyhow::Result<i32> {
        self.validate_command(command, args)?;

        let mut child = tokio::process::Command::new(command)
            .args(args)
            .current_dir(&self.project_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let tx_out = tx.clone();
        let tx_err = tx.clone();

        let stdout_task = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = TerminalLine {
                    kind: LineKind::Stdout,
                    content: line,
                    timestamp: chrono::Utc::now(),
                };
                if tx_out.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let msg = TerminalLine {
                    kind: LineKind::Stderr,
                    content: line,
                    timestamp: chrono::Utc::now(),
                };
                if tx_err.send(msg).await.is_err() {
                    break;
                }
            }
        });

        let timeout = tokio::time::Duration::from_secs(self.timeout_secs);
        let result = tokio::time::timeout(timeout, child.wait()).await;

        match result {
            Ok(Ok(status)) => {
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                Ok(status.code().unwrap_or(-1))
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Process wait error: {}", e)),
            Err(_) => {
                let _ = tx
                    .send(TerminalLine {
                        kind: LineKind::System,
                        content: format!("Command timed out after {} seconds", self.timeout_secs),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
                stdout_task.abort();
                stderr_task.abort();
                Err(anyhow::anyhow!(
                    "Command timed out after {} seconds",
                    self.timeout_secs
                ))
            }
        }
    }

    fn validate_command(&self, command: &str, args: &[&str]) -> anyhow::Result<()> {
        let binary = command.split_whitespace().next().unwrap_or("");

        if !self.allowed_commands.iter().any(|a| a == binary) {
            anyhow::bail!("Command '{}' not in terminal allowlist", binary);
        }

        let full_cmd = std::iter::once(command)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");

        for forbidden in &["&&", "||", ";", "|", "$(", "`"] {
            if full_cmd.contains(forbidden) {
                anyhow::bail!("Forbidden shell operator '{}' in command", forbidden);
            }
        }

        for arg in args {
            if arg.contains("../") {
                anyhow::bail!("Path traversal '../' forbidden in arguments");
            }
        }

        if binary == "git" {
            let allowed_git_subcmds = [
                "status", "log", "diff", "branch", "checkout", "add", "commit", "push", "pull",
                "fetch",
            ];
            if let Some(subcmd) = args.first() {
                if !allowed_git_subcmds.contains(subcmd) {
                    anyhow::bail!("git subcommand '{}' not allowed", subcmd);
                }
            }
        }

        Ok(())
    }
}
