use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::{
    enclave::TenantDB,
    git::GitHubBridge,
    pr_manager::{ManagedPr, PrManager, PrStatus},
    workspace::config::{MergeMode, WorkspaceConfig},
};
use axum::{
    extract::{Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_prs).post(create_pr_endpoint))
        .route("/:number/merge", post(merge_pr_endpoint))
        .route("/:number", patch(patch_pr_endpoint))
}

pub fn git_router() -> Router<AppState> {
    Router::new()
        .route("/status", get(git_status))
        .route("/branches", get(git_branches))
        .route("/commits", get(git_commits))
}

// ── GET /api/prs ──────────────────────────────────────────────────────────────

async fn list_prs(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Vec<ManagedPr>>, AegisHttpError> {
    let ws_tx = Arc::clone(&state.workspace_events);

    let prs = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<ManagedPr>> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        let settings = WorkspaceConfig::load_all(&db)?;
        let bridge = Arc::new(GitHubBridge::new(&settings)?);
        let manager = PrManager::new(bridge, (*ws_tx).clone());
        manager.list_all(&auth.tenant_id, &auth.session_key_hash)
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(prs))
}

// ── POST /api/prs ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreatePrBody {
    title: String,
    body: String,
    head: String,
    base: String,
    merge_mode: Option<String>,
    auto_fix_ci: Option<bool>,
}

async fn create_pr_endpoint(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<CreatePrBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let tenant_id = auth.tenant_id.clone();
    let session_key = auth.session_key_hash.clone();

    let settings = tokio::task::spawn_blocking({
        let tid = tenant_id.clone();
        let sk = session_key.clone();
        move || {
            let db = TenantDB::open(&tid, &sk)?;
            WorkspaceConfig::load_all(&db)
        }
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    let bridge = Arc::new(
        GitHubBridge::new(&settings)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Bridge init: {}", e)))?,
    );

    let pr = bridge
        .create_pr(&body.title, &body.body, &body.head, &body.base)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("GitHub API: {}", e)))?;

    let merge_mode = match body.merge_mode.as_deref() {
        Some("automatic") => MergeMode::Automatic,
        _ => MergeMode::Manual,
    };
    let auto_fix = body.auto_fix_ci.unwrap_or(true);
    let ws_tx = Arc::clone(&state.workspace_events);
    let bridge_clone = Arc::clone(&bridge);
    let pr_clone = pr.clone();

    tokio::task::spawn_blocking(move || {
        let manager = PrManager::new(bridge_clone, (*ws_tx).clone());
        manager.register_pr(&tenant_id, &session_key, &pr_clone, merge_mode, auto_fix)
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(json!({ "pr": pr })))
}

// ── PATCH /api/prs/:number ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PatchPrBody {
    merge_mode: Option<String>,
    auto_fix_ci: Option<bool>,
    status: Option<String>,
}

async fn patch_pr_endpoint(
    State(state): State<AppState>,
    Path(number): Path<u64>,
    auth: CitadelAuthenticated,
    Json(body): Json<PatchPrBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let ws_tx = Arc::clone(&state.workspace_events);

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        let settings = WorkspaceConfig::load_all(&db)?;
        let bridge = Arc::new(GitHubBridge::new(&settings)?);
        let manager = PrManager::new(Arc::clone(&bridge), (*ws_tx).clone());

        if let Some(mode) = &body.merge_mode {
            db.pr_set_merge_mode(number, mode)?;
        }
        if let Some(fix) = body.auto_fix_ci {
            db.pr_set_auto_fix_ci(number, fix)?;
        }
        if let Some(status_str) = &body.status {
            let status: PrStatus = status_str.parse().unwrap_or(PrStatus::Open);
            manager.update_status(&auth.tenant_id, &auth.session_key_hash, number, &status)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(json!({ "status": "updated" })))
}

// ── POST /api/prs/:number/merge ───────────────────────────────────────────────

async fn merge_pr_endpoint(
    State(state): State<AppState>,
    Path(number): Path<u64>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let tenant_id = auth.tenant_id.clone();
    let session_key = auth.session_key_hash.clone();

    let settings = tokio::task::spawn_blocking({
        let tid = tenant_id.clone();
        let sk = session_key.clone();
        move || {
            let db = TenantDB::open(&tid, &sk)?;
            WorkspaceConfig::load_all(&db)
        }
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    let bridge = Arc::new(
        GitHubBridge::new(&settings)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Bridge init: {}", e)))?,
    );

    let ws_tx = Arc::clone(&state.workspace_events);
    let manager = PrManager::new(Arc::clone(&bridge), (*ws_tx).clone());

    manager
        .merge_now(&tenant_id, &session_key, number)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Merge failed: {}", e)))?;

    Ok(Json(json!({ "status": "merged" })))
}

// ── GET /api/git/status ───────────────────────────────────────────────────────

async fn git_status(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let settings = tokio::task::spawn_blocking(move || {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        WorkspaceConfig::load_all(&db)
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    let bridge = GitHubBridge::new(&settings)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Bridge init: {}", e)))?;

    let result = bridge
        .status()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Git status: {}", e)))?;

    Ok(Json(json!({ "status": result })))
}

// ── GET /api/git/branches ─────────────────────────────────────────────────────

async fn git_branches(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let tenant_id = auth.tenant_id.clone();
    let session_key = auth.session_key_hash.clone();

    let settings = tokio::task::spawn_blocking(move || {
        let db = TenantDB::open(&tenant_id, &session_key)?;
        WorkspaceConfig::load_all(&db)
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    let bridge = GitHubBridge::new(&settings)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Bridge init: {}", e)))?;

    let branches = bridge
        .list_branches()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Git list_branches: {}", e)))?;

    Ok(Json(json!({ "branches": branches })))
}

// ── GET /api/git/commits ──────────────────────────────────────────────────────

async fn git_commits(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let tenant_id = auth.tenant_id.clone();
    let session_key = auth.session_key_hash.clone();

    let settings = tokio::task::spawn_blocking(move || {
        let db = TenantDB::open(&tenant_id, &session_key)?;
        WorkspaceConfig::load_all(&db)
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    let bridge = GitHubBridge::new(&settings)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Bridge init: {}", e)))?;

    let current = bridge
        .current_branch()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("current_branch: {}", e)))?;

    let commits = bridge
        .list_commits(&current, 20)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("list_commits: {}", e)))?;

    Ok(Json(json!({ "branch": current, "commits": commits })))
}
