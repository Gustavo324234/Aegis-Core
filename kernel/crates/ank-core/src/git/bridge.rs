use crate::workspace::config::WorkspaceSettings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct GitHubBridge {
    project_root: PathBuf,
    github_token: String,
    pub repo_owner: String,
    pub repo_name: String,
    bot_name: String,
    bot_email: String,
    pub http: reqwest::Client,
}

impl GitHubBridge {
    pub fn new(settings: &WorkspaceSettings) -> anyhow::Result<Self> {
        let token = settings
            .github_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GitHub token not configured"))?;
        let repo = settings
            .github_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GitHub repo not configured"))?;
        let (owner, name) = repo
            .split_once('/')
            .ok_or_else(|| anyhow::anyhow!("Invalid repo format, expected owner/repo"))?;
        let root = settings
            .project_root
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Project root not configured"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", token)
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid token format"))?,
        );
        headers.insert(
            "Accept",
            "application/vnd.github+json"
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid header value"))?,
        );

        let http = reqwest::Client::builder()
            .user_agent("AegisOS/1.0")
            .default_headers(headers)
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

    // ── Git local ─────────────────────────────────────────────────────────

    pub async fn create_branch(&self, branch_name: &str, from: &str) -> anyhow::Result<()> {
        self.git(&["checkout", from]).await?;
        self.git(&["checkout", "-b", branch_name]).await?;
        Ok(())
    }

    pub async fn commit(&self, files: &[&str], message: &str) -> anyhow::Result<String> {
        let mut add_args = vec!["add"];
        add_args.extend_from_slice(files);
        self.git(&add_args).await?;

        let user_name_cfg = format!("user.name={}", self.bot_name);
        let user_email_cfg = format!("user.email={}", self.bot_email);
        self.git(&[
            "-c",
            &user_name_cfg,
            "-c",
            &user_email_cfg,
            "commit",
            "-m",
            message,
        ])
        .await?;

        let sha = self.git_output(&["rev-parse", "HEAD"]).await?;
        Ok(sha.trim().to_string())
    }

    pub async fn push(&self, branch_name: &str) -> anyhow::Result<()> {
        // Token en URL — redactar en cualquier log que sea necesario
        let remote_url = format!(
            "https://***@github.com/{}/{}",
            self.repo_owner, self.repo_name
        );
        // Usar el token real para el push pero no loguearlo
        let real_url = format!(
            "https://{}@github.com/{}/{}",
            self.github_token, self.repo_owner, self.repo_name
        );
        tracing::debug!("Pushing to {}", remote_url);
        self.git_with_url_push(&real_url, branch_name).await?;
        Ok(())
    }

    pub async fn status(&self) -> anyhow::Result<String> {
        self.git_output(&["status", "--short"]).await
    }

    pub async fn list_commits(
        &self,
        branch: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CommitInfo>> {
        let format = "--pretty=format:%H|%s|%an|%ae|%aI";
        let limit_str = format!("-{}", limit);
        let output = self
            .git_output(&["log", branch, &limit_str, format])
            .await?;
        if output.trim().is_empty() {
            return Ok(Vec::new());
        }
        output
            .lines()
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

    pub async fn list_branches(&self) -> anyhow::Result<Vec<BranchInfo>> {
        let output = self
            .git_output(&[
                "branch",
                "-a",
                "--format=%(refname:short)|%(objectname:short)|%(committerdate:iso)",
            ])
            .await?;
        let branches = output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.splitn(3, '|').collect();
                let name = parts.first().copied().unwrap_or("").to_string();
                let short_sha = parts.get(1).copied().unwrap_or("").to_string();
                let date_str = parts.get(2).copied().unwrap_or("");
                let last_commit_date = chrono::DateTime::parse_from_rfc3339(date_str)
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc));
                let is_remote = name.starts_with("remotes/");
                BranchInfo {
                    name,
                    short_sha,
                    last_commit_date,
                    is_remote,
                }
            })
            .collect();
        Ok(branches)
    }

    pub async fn current_branch(&self) -> anyhow::Result<String> {
        let output = self
            .git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
            .await?;
        Ok(output.trim().to_string())
    }

    // ── GitHub API ────────────────────────────────────────────────────────

    pub async fn create_pr(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<PullRequest> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            self.repo_owner, self.repo_name
        );
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "title": title,
                "body": body,
                "head": head,
                "base": base
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(PullRequest {
            number: resp["number"].as_u64().unwrap_or(0),
            title: resp["title"].as_str().unwrap_or("").to_string(),
            url: resp["html_url"].as_str().unwrap_or("").to_string(),
            head: head.to_string(),
            base: base.to_string(),
            state: resp["state"].as_str().unwrap_or("open").to_string(),
        })
    }

    pub async fn get_pr_checks(&self, pr_number: u64) -> anyhow::Result<Vec<CiCheck>> {
        let commits_url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/commits",
            self.repo_owner, self.repo_name, pr_number
        );
        let commits = self
            .http
            .get(&commits_url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let sha = commits
            .as_array()
            .and_then(|a| a.last())
            .and_then(|c| c["sha"].as_str())
            .ok_or_else(|| anyhow::anyhow!("No commits found for PR {}", pr_number))?
            .to_string();

        let checks_url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}/check-runs",
            self.repo_owner, self.repo_name, sha
        );
        let checks = self
            .http
            .get(&checks_url)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        let empty_vec = vec![];
        let runs = checks["check_runs"].as_array().unwrap_or(&empty_vec);
        Ok(runs
            .iter()
            .map(|c| CiCheck {
                name: c["name"].as_str().unwrap_or("").to_string(),
                state: match c["conclusion"].as_str() {
                    Some("success") => CiState::Success,
                    Some("failure") => CiState::Failure,
                    Some("cancelled") => CiState::Cancelled,
                    _ => CiState::Running,
                },
                url: c["html_url"].as_str().unwrap_or("").to_string(),
            })
            .collect())
    }

    pub async fn get_failed_ci_logs(&self, pr_number: u64) -> anyhow::Result<String> {
        let checks = self.get_pr_checks(pr_number).await?;
        let failed = checks
            .iter()
            .find(|c| c.state == CiState::Failure)
            .ok_or_else(|| anyhow::anyhow!("No failed check found for PR {}", pr_number))?;
        Ok(format!(
            "CI check '{}' failed. See: {}",
            failed.name, failed.url
        ))
    }

    pub async fn is_pr_merged(&self, pr_number: u64) -> anyhow::Result<bool> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            self.repo_owner, self.repo_name, pr_number
        );
        let status = self.http.get(&url).send().await?.status();
        Ok(status == 204)
    }

    // ── Helpers privados ──────────────────────────────────────────────────

    async fn git(&self, args: &[&str]) -> anyhow::Result<()> {
        let status = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.project_root)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("git command failed with status {}", status);
        }
        Ok(())
    }

    pub async fn git_output(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.project_root)
            .output()
            .await?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git command failed: {}", err);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn git_with_url_push(&self, remote_url: &str, branch: &str) -> anyhow::Result<()> {
        let status = tokio::process::Command::new("git")
            .args(["push", remote_url, branch])
            .current_dir(&self.project_root)
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("git push failed with status {}", status);
        }
        Ok(())
    }
}

// ── Data types ────────────────────────────────────────────────────────────────

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
