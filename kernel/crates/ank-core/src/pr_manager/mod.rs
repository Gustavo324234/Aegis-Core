use crate::git::bridge::{CiState, GitHubBridge, PullRequest};
use crate::workspace::config::MergeMode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

// ── Workspace WebSocket event broadcast (CORE-175) ───────────────────────────

#[derive(Debug, Clone)]
pub struct WorkspaceWsEvent {
    pub tenant_id: String,
    pub payload: serde_json::Value,
}

// ── PR types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PrStatus {
    Open,
    CiRunning,
    CiPassed,
    CiFailed,
    AutoFixInProgress,
    MergeReady,
    Merged,
    Closed,
}

impl std::fmt::Display for PrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PrStatus::Open => "open",
            PrStatus::CiRunning => "ci_running",
            PrStatus::CiPassed => "ci_passed",
            PrStatus::CiFailed => "ci_failed",
            PrStatus::AutoFixInProgress => "auto_fix_in_progress",
            PrStatus::MergeReady => "merge_ready",
            PrStatus::Merged => "merged",
            PrStatus::Closed => "closed",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for PrStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ci_running" => PrStatus::CiRunning,
            "ci_passed" => PrStatus::CiPassed,
            "ci_failed" => PrStatus::CiFailed,
            "auto_fix_in_progress" => PrStatus::AutoFixInProgress,
            "merge_ready" => PrStatus::MergeReady,
            "merged" => PrStatus::Merged,
            "closed" => PrStatus::Closed,
            _ => PrStatus::Open,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPr {
    pub pr_number: u64,
    pub title: String,
    pub branch: String,
    pub base_branch: String,
    pub url: String,
    pub merge_mode: MergeMode,
    pub auto_fix_ci: bool,
    pub auto_fix_attempts: u32,
    pub status: PrStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── PrManager ─────────────────────────────────────────────────────────────────

pub struct PrManager {
    git: Arc<GitHubBridge>,
    ws_tx: broadcast::Sender<WorkspaceWsEvent>,
}

impl PrManager {
    pub fn new(git: Arc<GitHubBridge>, ws_tx: broadcast::Sender<WorkspaceWsEvent>) -> Self {
        Self { git, ws_tx }
    }

    // ── DB helpers ────────────────────────────────────────────────────────

    fn open_db(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
    ) -> anyhow::Result<crate::enclave::TenantDB> {
        crate::enclave::TenantDB::open(tenant_id, session_key_hash)
    }

    // ── Public API ────────────────────────────────────────────────────────

    pub fn register_pr(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr: &PullRequest,
        merge_mode: MergeMode,
        auto_fix_ci: bool,
    ) -> anyhow::Result<()> {
        let db = self.open_db(tenant_id, session_key_hash)?;
        let now = chrono::Utc::now().to_rfc3339();
        db.connection().execute(
            "INSERT OR REPLACE INTO managed_prs \
             (pr_number, title, branch, base_branch, url, merge_mode, auto_fix_ci, \
              auto_fix_attempts, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 'open', ?8, ?8)",
            rusqlite::params![
                pr.number as i64,
                &pr.title,
                &pr.head,
                &pr.base,
                &pr.url,
                match merge_mode {
                    MergeMode::Automatic => "automatic",
                    MergeMode::Manual => "manual",
                },
                auto_fix_ci as i64,
                &now,
            ],
        )?;
        Ok(())
    }

    pub fn update_status(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr_number: u64,
        status: &PrStatus,
    ) -> anyhow::Result<()> {
        let db = self.open_db(tenant_id, session_key_hash)?;
        let now = chrono::Utc::now().to_rfc3339();
        db.connection().execute(
            "UPDATE managed_prs SET status = ?1, updated_at = ?2 WHERE pr_number = ?3",
            rusqlite::params![status.to_string(), &now, pr_number as i64],
        )?;
        Ok(())
    }

    pub fn list_active(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
    ) -> anyhow::Result<Vec<ManagedPr>> {
        let db = self.open_db(tenant_id, session_key_hash)?;
        self.query_prs(
            &db,
            "SELECT pr_number, title, branch, base_branch, url, merge_mode, auto_fix_ci, \
             auto_fix_attempts, status, created_at, updated_at FROM managed_prs \
             WHERE status NOT IN ('merged', 'closed')",
        )
    }

    pub fn list_all(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
    ) -> anyhow::Result<Vec<ManagedPr>> {
        let db = self.open_db(tenant_id, session_key_hash)?;
        self.query_prs(
            &db,
            "SELECT pr_number, title, branch, base_branch, url, merge_mode, auto_fix_ci, \
             auto_fix_attempts, status, created_at, updated_at FROM managed_prs \
             ORDER BY updated_at DESC",
        )
    }

    fn query_prs(
        &self,
        db: &crate::enclave::TenantDB,
        sql: &str,
    ) -> anyhow::Result<Vec<ManagedPr>> {
        let mut stmt = db.connection().prepare(sql)?;
        let prs = stmt
            .query_map([], |row| {
                let pr_number: i64 = row.get(0)?;
                let merge_mode_str: String = row.get(5)?;
                let auto_fix_ci: i64 = row.get(6)?;
                let auto_fix_attempts: i64 = row.get(7)?;
                let status_str: String = row.get(8)?;
                let created_str: String = row.get(9)?;
                let updated_str: String = row.get(10)?;
                Ok((
                    pr_number,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    merge_mode_str,
                    auto_fix_ci,
                    auto_fix_attempts,
                    status_str,
                    created_str,
                    updated_str,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(
                |(
                    pr_number,
                    title,
                    branch,
                    base_branch,
                    url,
                    merge_mode_str,
                    auto_fix_ci,
                    auto_fix_attempts,
                    status_str,
                    created_str,
                    updated_str,
                )| {
                    ManagedPr {
                        pr_number: pr_number as u64,
                        title,
                        branch,
                        base_branch,
                        url,
                        merge_mode: if merge_mode_str == "automatic" {
                            MergeMode::Automatic
                        } else {
                            MergeMode::Manual
                        },
                        auto_fix_ci: auto_fix_ci != 0,
                        auto_fix_attempts: auto_fix_attempts as u32,
                        status: status_str.parse().unwrap_or(PrStatus::Open),
                        created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
                            .map(|d| d.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_str)
                            .map(|d| d.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                    }
                },
            )
            .collect();
        Ok(prs)
    }

    fn send_ws_event(&self, tenant_id: &str, payload: serde_json::Value) {
        let event = WorkspaceWsEvent {
            tenant_id: tenant_id.to_string(),
            payload,
        };
        let _ = self.ws_tx.send(event);
    }

    // ── Polling loop ──────────────────────────────────────────────────────

    /// Loop de polling — llamar en un tokio::spawn al iniciar el servidor.
    /// Itera sobre todos los tenants activos cada 30 segundos.
    pub async fn polling_loop(
        self: Arc<Self>,
        tenant_registry: Arc<tokio::sync::RwLock<Vec<(String, String)>>>,
    ) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

            let tenants = tenant_registry.read().await.clone();
            for (tenant_id, session_key_hash) in tenants {
                if let Ok(prs) = self.list_active(&tenant_id, &session_key_hash) {
                    for pr in prs {
                        if let Err(e) = self.evaluate_pr(&tenant_id, &session_key_hash, &pr).await {
                            warn!(
                                tenant = %tenant_id,
                                pr = pr.pr_number,
                                "PR evaluation error: {}",
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    async fn evaluate_pr(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr: &ManagedPr,
    ) -> anyhow::Result<()> {
        let checks = self.git.get_pr_checks(pr.pr_number).await?;
        let all_success = !checks.is_empty() && checks.iter().all(|c| c.state == CiState::Success);
        let any_failure = checks.iter().any(|c| c.state == CiState::Failure);

        if any_failure {
            if pr.auto_fix_ci && pr.status != PrStatus::AutoFixInProgress {
                self.trigger_auto_fix(tenant_id, session_key_hash, pr)
                    .await?;
                self.update_status(
                    tenant_id,
                    session_key_hash,
                    pr.pr_number,
                    &PrStatus::AutoFixInProgress,
                )?;
            } else if pr.status != PrStatus::AutoFixInProgress {
                self.update_status(
                    tenant_id,
                    session_key_hash,
                    pr.pr_number,
                    &PrStatus::CiFailed,
                )?;
                self.send_ws_event(
                    tenant_id,
                    serde_json::json!({
                        "event": "pr_update",
                        "data": { "pr_number": pr.pr_number, "status": "CiFailed", "url": pr.url }
                    }),
                );
            }
        } else if all_success {
            self.update_status(
                tenant_id,
                session_key_hash,
                pr.pr_number,
                &PrStatus::CiPassed,
            )?;
            match pr.merge_mode {
                MergeMode::Automatic => {
                    self.merge_pr(tenant_id, session_key_hash, pr).await?;
                }
                MergeMode::Manual => {
                    self.update_status(
                        tenant_id,
                        session_key_hash,
                        pr.pr_number,
                        &PrStatus::MergeReady,
                    )?;
                    self.send_ws_event(
                        tenant_id,
                        serde_json::json!({
                            "event": "pr_update",
                            "data": { "pr_number": pr.pr_number, "status": "MergeReady", "url": pr.url }
                        }),
                    );
                }
            }
        }

        if self.git.is_pr_merged(pr.pr_number).await? && pr.status != PrStatus::Merged {
            self.update_status(tenant_id, session_key_hash, pr.pr_number, &PrStatus::Merged)?;
            self.send_ws_event(
                tenant_id,
                serde_json::json!({
                    "event": "pr_merged",
                    "data": { "pr_number": pr.pr_number, "title": pr.title }
                }),
            );
        }

        Ok(())
    }

    async fn merge_pr(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr: &ManagedPr,
    ) -> anyhow::Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            self.git.repo_owner, self.git.repo_name, pr.pr_number
        );
        self.git
            .http
            .put(&url)
            .json(&serde_json::json!({ "merge_method": "squash" }))
            .send()
            .await?
            .error_for_status()?;

        self.update_status(tenant_id, session_key_hash, pr.pr_number, &PrStatus::Merged)?;
        self.send_ws_event(
            tenant_id,
            serde_json::json!({
                "event": "pr_merged",
                "data": { "pr_number": pr.pr_number, "title": pr.title }
            }),
        );
        info!(pr = pr.pr_number, "PR merged automatically");
        Ok(())
    }

    // ── CORE-174: Auto-fix CI ─────────────────────────────────────────────

    async fn trigger_auto_fix(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr: &ManagedPr,
    ) -> anyhow::Result<()> {
        let ci_error = self
            .git
            .get_failed_ci_logs(pr.pr_number)
            .await
            .unwrap_or_else(|_| "CI check failed. No logs available.".to_string());

        let diff = self
            .git
            .git_output(&["diff", "main", &pr.branch])
            .await
            .unwrap_or_default();
        let diff_truncated: String = diff.chars().take(3000).collect();

        let attempt = pr.auto_fix_attempts + 1;

        self.send_ws_event(
            tenant_id,
            serde_json::json!({
                "event": "ci_fix_attempt",
                "data": {
                    "pr_number": pr.pr_number,
                    "attempt": attempt,
                    "max_attempts": 3,
                    "error_summary": ci_error
                }
            }),
        );

        if pr.auto_fix_attempts >= 2 {
            self.update_status(
                tenant_id,
                session_key_hash,
                pr.pr_number,
                &PrStatus::CiFailed,
            )?;
            self.send_ws_event(
                tenant_id,
                serde_json::json!({
                    "event": "chat_notification",
                    "data": {
                        "message": format!(
                            "⚠️ Auto-fix agotó 3 intentos para PR #{} ('{}'). Revisión manual requerida. {}",
                            pr.pr_number, pr.title, pr.url
                        )
                    }
                }),
            );
            return Ok(());
        }

        let db = self.open_db(tenant_id, session_key_hash)?;
        db.connection().execute(
            "UPDATE managed_prs SET auto_fix_attempts = auto_fix_attempts + 1 WHERE pr_number = ?1",
            rusqlite::params![pr.pr_number as i64],
        )?;

        info!(
            pr = pr.pr_number,
            attempt = attempt,
            diff_len = diff_truncated.len(),
            "Auto-fix CI triggered (enqueue to scheduler in production)"
        );

        Ok(())
    }

    /// Merge manual inmediato (para modo Manual cuando CI pasó).
    pub async fn merge_now(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        pr_number: u64,
    ) -> anyhow::Result<()> {
        let (title, url) = {
            let db = self.open_db(tenant_id, session_key_hash)?;
            let mut stmt = db.connection().prepare(
                "SELECT title, url FROM managed_prs WHERE pr_number = ?1 AND status = 'merge_ready'",
            )?;
            stmt.query_row(rusqlite::params![pr_number as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| anyhow::anyhow!("PR {} not found or not in MergeReady state", pr_number))?
        };

        let api_url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            self.git.repo_owner, self.git.repo_name, pr_number
        );
        self.git
            .http
            .put(&api_url)
            .json(&serde_json::json!({ "merge_method": "squash" }))
            .send()
            .await?
            .error_for_status()?;

        self.update_status(tenant_id, session_key_hash, pr_number, &PrStatus::Merged)?;
        self.send_ws_event(
            tenant_id,
            serde_json::json!({
                "event": "pr_merged",
                "data": { "pr_number": pr_number, "title": title, "url": url }
            }),
        );
        Ok(())
    }
}
