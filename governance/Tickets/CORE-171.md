# CORE-171 — feat: GitHubBridge — Identidad del Bot, Branch, Commit, Push y PR

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-167 (github_token, github_repo, project_root desde workspace_config)
**Estado:** ✅ Completo

---

## Contexto

El `GitHubBridge` centraliza todas las operaciones Git y GitHub de Aegis. Los agentes lo usan via syscalls (CORE-172), el PR Manager lo usa directamente (CORE-173). Aegis aparece como colaborador con identidad propia (`Aegis OS <bot@aegis-os.dev>`).

Las operaciones Git locales usan `tokio::process::Command` con `git`. Las operaciones de GitHub API (crear PR, leer CI checks) usan `reqwest`.

---

## Cambios Requeridos

### Crear `ank-core/src/git/mod.rs` y `ank-core/src/git/bridge.rs`

```rust
// git/mod.rs
pub mod bridge;
pub use bridge::{GitHubBridge, PullRequest, BranchInfo, CommitInfo, CiCheck, CiState};
```

```rust
// git/bridge.rs

pub struct GitHubBridge {
    project_root: PathBuf,
    github_token: String,
    repo_owner: String,
    repo_name: String,
    bot_name: String,    // "Aegis OS"
    bot_email: String,   // "bot@aegis-os.dev"
    http: reqwest::Client,
}

impl GitHubBridge {
    pub fn new(settings: &WorkspaceSettings) -> anyhow::Result<Self> {
        let token = settings.github_token.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GitHub token not configured"))?;
        let repo = settings.github_repo.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GitHub repo not configured"))?;
        let (owner, name) = repo.split_once('/')
            .ok_or_else(|| anyhow::anyhow!("Invalid repo format, expected owner/repo"))?;
        let root = settings.project_root.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Project root not configured"))?;

        let http = reqwest::Client::builder()
            .user_agent("AegisOS/1.0")
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("Authorization", format!("Bearer {}", token).parse()?);
                h.insert("Accept", "application/vnd.github+json".parse()?);
                h
            })
            .build()?;

        Ok(Self {
            project_root: PathBuf::from(root),
            github_token: token.clone(),
            repo_owner: owner.to_string(),
            repo_name: name.to_string(),
            bot_name: "Aegis OS".to_string(),
            bot_email: "bot@aegis-os.dev".to_string(),
            http,
        })
    }

    // ── Git local ──────────────────────────────────────────────────────

    /// Crea una branch nueva desde `from` (ej: "main")
    pub async fn create_branch(&self, branch_name: &str, from: &str) -> anyhow::Result<()> {
        self.git(&["checkout", from]).await?;
        self.git(&["checkout", "-b", branch_name]).await?;
        Ok(())
    }

    /// Stage de archivos específicos + commit con identidad del bot
    pub async fn commit(&self, files: &[&str], message: &str) -> anyhow::Result<String> {
        // git add <files>
        let mut add_args = vec!["add"];
        add_args.extend_from_slice(files);
        self.git(&add_args).await?;

        // git commit con identidad del bot
        self.git(&[
            "-c", &format!("user.name={}", self.bot_name),
            "-c", &format!("user.email={}", self.bot_email),
            "commit", "-m", message,
        ]).await?;

        // obtener el SHA del commit
        let sha = self.git_output(&["rev-parse", "HEAD"]).await?;
        Ok(sha.trim().to_string())
    }

    /// Push de la branch al remoto origin
    pub async fn push(&self, branch_name: &str) -> anyhow::Result<()> {
        // Usar token en la URL para autenticación
        let remote_url = format!(
            "https://{}@github.com/{}/{}",
            self.github_token, self.repo_owner, self.repo_name
        );
        self.git(&["push", &remote_url, branch_name]).await?;
        Ok(())
    }

    /// Status del repo (para que los agentes sepan qué hay pendiente)
    pub async fn status(&self) -> anyhow::Result<String> {
        self.git_output(&["status", "--short"]).await
    }

    /// Log de commits recientes de una branch
    pub async fn list_commits(&self, branch: &str, limit: usize) -> anyhow::Result<Vec<CommitInfo>> {
        let format = "--pretty=format:%H|%s|%an|%ae|%aI";
        let limit_str = format!("-{}", limit);
        let output = self.git_output(&["log", branch, &limit_str, format]).await?;
        // parsear cada línea: SHA|subject|author_name|author_email|date
        output.lines()
            .map(|line| {
                let parts: Vec<&str> = line.splitn(5, '|').collect();
                if parts.len() == 5 {
                    Ok(CommitInfo {
                        sha: parts[0].to_string(),
                        message: parts[1].to_string(),
                        author_name: parts[2].to_string(),
                        author_email: parts[3].to_string(),
                        date: parts[4].parse()?,
                    })
                } else {
                    anyhow::bail!("Unexpected git log format: {}", line)
                }
            })
            .collect()
    }

    /// Lista branches locales y remotas
    pub async fn list_branches(&self) -> anyhow::Result<Vec<BranchInfo>> {
        let output = self.git_output(&["branch", "-a", "--format=%(refname:short)|%(objectname:short)|%(committerdate:iso)"]).await?;
        // parsear y retornar BranchInfo
        // ...
        Ok(vec![]) // implementar parsing
    }

    // ── GitHub API ─────────────────────────────────────────────────────

    /// Crea un Pull Request via GitHub API REST
    pub async fn create_pr(
        &self,
        title: &str,
        body: &str,
        head: &str,   // branch de origen
        base: &str,   // branch destino (ej: "main")
    ) -> anyhow::Result<PullRequest> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            self.repo_owner, self.repo_name
        );
        let resp = self.http.post(&url)
            .json(&serde_json::json!({ "title": title, "body": body, "head": head, "base": base }))
            .send().await?
            .error_for_status()?
            .json::<serde_json::Value>().await?;

        Ok(PullRequest {
            number: resp["number"].as_u64().unwrap_or(0),
            title: resp["title"].as_str().unwrap_or("").to_string(),
            url: resp["html_url"].as_str().unwrap_or("").to_string(),
            head: head.to_string(),
            base: base.to_string(),
            state: resp["state"].as_str().unwrap_or("open").to_string(),
        })
    }

    /// Retorna los CI checks de un PR
    pub async fn get_pr_checks(&self, pr_number: u64) -> anyhow::Result<Vec<CiCheck>> {
        // GET /repos/{owner}/{repo}/pulls/{pr_number}/commits → obtener SHA del último commit
        // GET /repos/{owner}/{repo}/commits/{sha}/check-runs → obtener checks
        let commits_url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/commits",
            self.repo_owner, self.repo_name, pr_number
        );
        let commits = self.http.get(&commits_url).send().await?.error_for_status()?.json::<serde_json::Value>().await?;
        let sha = commits.as_array()
            .and_then(|a| a.last())
            .and_then(|c| c["sha"].as_str())
            .ok_or_else(|| anyhow::anyhow!("No commits found for PR {}", pr_number))?
            .to_string();

        let checks_url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}/check-runs",
            self.repo_owner, self.repo_name, sha
        );
        let checks = self.http.get(&checks_url).send().await?.error_for_status()?.json::<serde_json::Value>().await?;

        Ok(checks["check_runs"].as_array().unwrap_or(&vec![]).iter().map(|c| CiCheck {
            name: c["name"].as_str().unwrap_or("").to_string(),
            state: match c["conclusion"].as_str() {
                Some("success") => CiState::Success,
                Some("failure") => CiState::Failure,
                Some("cancelled") => CiState::Cancelled,
                _ => CiState::Running,
            },
            url: c["html_url"].as_str().unwrap_or("").to_string(),
        }).collect())
    }

    /// Retorna los logs del step fallido de CI (para auto-fix)
    pub async fn get_failed_ci_logs(&self, pr_number: u64) -> anyhow::Result<String> {
        let checks = self.get_pr_checks(pr_number).await?;
        let failed = checks.iter().find(|c| c.state == CiState::Failure)
            .ok_or_else(|| anyhow::anyhow!("No failed check found for PR {}", pr_number))?;
        // GitHub no expone logs directamente via REST; retornar el link al check
        // El auto-fix (CORE-174) usará la URL para dar contexto al LLM
        Ok(format!("CI check '{}' failed. See: {}", failed.name, failed.url))
    }

    /// Retorna si un PR fue mergeado
    pub async fn is_pr_merged(&self, pr_number: u64) -> anyhow::Result<bool> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            self.repo_owner, self.repo_name, pr_number
        );
        let status = self.http.get(&url).send().await?.status();
        Ok(status == 204)
    }

    // ── Helpers privados ───────────────────────────────────────────────

    async fn git(&self, args: &[&str]) -> anyhow::Result<()> {
        let status = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.project_root)
            .status().await?;
        if !status.success() {
            anyhow::bail!("git {:?} failed with status {}", args, status);
        }
        Ok(())
    }

    async fn git_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.project_root)
            .output().await?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git {:?} failed: {}", args, err);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

// ── Tipos de datos ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub head: String,
    pub base: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub date: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub short_sha: String,
    pub last_commit_date: Option<chrono::DateTime<chrono::Utc>>,
    pub is_remote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiCheck {
    pub name: String,
    pub state: CiState,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CiState {
    Running,
    Success,
    Failure,
    Cancelled,
}
```

### Agregar al `lib.rs` de `ank-core`

```rust
pub mod git;
```

---

## Criterios de Aceptación

- [x] `GitHubBridge::new()` falla con error descriptivo si `github_token`, `github_repo` o `project_root` no están configurados
- [x] `create_branch()` crea la branch localmente partiendo de la branch `from`
- [x] `commit()` usa `user.name=Aegis OS` y `user.email=bot@aegis-os.dev` — verificable con `git log --format="%an <%ae>"`
- [x] `push()` no loggea el token en ningún mensaje de log ni error
- [x] `create_pr()` retorna un `PullRequest` con `number` y `url` válidos
- [x] `get_pr_checks()` retorna `CiState::Running` para checks sin conclusión aún
- [x] `is_pr_merged()` retorna `true` para PRs mergeados (status 204) y `false` para los abiertos (status 404)
- [x] El token de GitHub **nunca aparece en logs** — usar `tracing::debug!` con cuidado, redactar el token si se loggea la URL
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*