use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::{enclave::TenantDB, workspace::config::WorkspaceConfig};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tree", get(fs_tree))
        .route("/file", get(fs_file))
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FileTreeDto {
    pub path: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub kind: FsEntryKind,
    pub children: Option<Vec<FsEntry>>,
    pub extension: Option<String>,
}

#[derive(Serialize)]
pub enum FsEntryKind {
    File,
    Directory,
}

#[derive(Serialize)]
pub struct FileContentDto {
    pub path: String,
    pub content: String,
    pub language: String,
    pub lines: usize,
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: Option<String>,
}

// ── Security ──────────────────────────────────────────────────────────────────

fn resolve_safe_path(project_root: &Path, requested: &str) -> anyhow::Result<PathBuf> {
    if requested.starts_with('/') || requested.contains(':') {
        anyhow::bail!("Absolute paths not allowed");
    }
    if requested.contains("../") {
        anyhow::bail!("Path traversal detected");
    }
    let joined = project_root.join(requested);
    let resolved = joined
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Cannot resolve path: {}", e))?;
    let root_canon = project_root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Cannot canonicalize project_root: {}", e))?;
    if !resolved.starts_with(&root_canon) {
        anyhow::bail!("Path traversal detected");
    }
    Ok(resolved)
}

fn is_blocked_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if name == ".env" || name == "aegis.env" {
        return true;
    }
    for ext in &["key", "pem", "p12"] {
        if name.ends_with(&format!(".{}", ext)) {
            return true;
        }
    }
    for word in &["secret", "password", "token"] {
        if name.contains(word) {
            return true;
        }
    }
    false
}

fn is_allowed_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "toml"
            | "json"
            | "md"
            | "yaml"
            | "yml"
            | "gitignore"
            | "sh"
    ) || path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == ".env.example")
        .unwrap_or(false)
}

const IGNORED_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", ".aegis"];

fn ext_str(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string())
}

fn language_from_ext(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        "yaml" | "yml" => "yaml",
        "sh" => "shell",
        _ => "text",
    }
    .to_string()
}

fn build_tree(dir: &Path, root: &Path, depth: u8) -> Vec<FsEntry> {
    if depth == 0 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let children = build_tree(&path, root, depth - 1);
            result.push(FsEntry {
                name,
                path: rel,
                kind: FsEntryKind::Directory,
                children: Some(children),
                extension: None,
            });
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            result.push(FsEntry {
                name,
                path: rel,
                kind: FsEntryKind::File,
                children: None,
                extension: ext_str(&path),
            });
        }
    }
    result.sort_by(|a, b| {
        let a_is_dir = matches!(a.kind, FsEntryKind::Directory);
        let b_is_dir = matches!(b.kind, FsEntryKind::Directory);
        b_is_dir.cmp(&a_is_dir).then(a.name.cmp(&b.name))
    });
    result
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn fs_tree(
    _state: State<AppState>,
    auth: CitadelAuthenticated,
    Query(params): Query<PathQuery>,
) -> Result<Json<FileTreeDto>, AegisHttpError> {
    let requested = params.path.unwrap_or_else(|| ".".to_string());

    let dto = tokio::task::spawn_blocking(move || -> anyhow::Result<FileTreeDto> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)?;
        let settings = WorkspaceConfig::load_all(&db)?;
        let project_root = settings
            .project_root
            .ok_or_else(|| anyhow::anyhow!("project_root not configured"))?;
        let root = PathBuf::from(&project_root);
        let target = resolve_safe_path(&root, &requested)?;
        let root_canon = root
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Cannot canonicalize root: {}", e))?;
        let entries = build_tree(&target, &root_canon, 3);
        Ok(FileTreeDto {
            path: requested,
            entries,
        })
    })
    .await
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Task panicked: {}", e)))??;

    Ok(Json(dto))
}

async fn fs_file(
    _state: State<AppState>,
    auth: CitadelAuthenticated,
    Query(params): Query<PathQuery>,
) -> Result<Json<FileContentDto>, (StatusCode, Json<serde_json::Value>)> {
    let requested = params.path.unwrap_or_default();
    if requested.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "path query param required" })),
        ));
    }

    let result = tokio::task::spawn_blocking(move || -> Result<FileContentDto, FsFileError> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|e| FsFileError::Internal(e.to_string()))?;
        let settings =
            WorkspaceConfig::load_all(&db).map_err(|e| FsFileError::Internal(e.to_string()))?;
        let project_root = settings
            .project_root
            .ok_or_else(|| FsFileError::Internal("project_root not configured".to_string()))?;
        let root = PathBuf::from(&project_root);
        let resolved = resolve_safe_path(&root, &requested).map_err(|_| FsFileError::Traversal)?;

        if is_blocked_file(&resolved) {
            return Err(FsFileError::Forbidden);
        }
        if !is_allowed_extension(&resolved) {
            return Err(FsFileError::Forbidden);
        }

        let metadata =
            std::fs::metadata(&resolved).map_err(|e| FsFileError::Internal(e.to_string()))?;
        if metadata.len() > 500 * 1024 {
            return Err(FsFileError::TooLarge);
        }

        let content =
            std::fs::read_to_string(&resolved).map_err(|e| FsFileError::Internal(e.to_string()))?;
        let lines = content.lines().count();
        let language = language_from_ext(&resolved);

        Ok(FileContentDto {
            path: requested,
            content,
            language,
            lines,
        })
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Task panicked: {}", e) })),
        )
    })?;

    match result {
        Ok(dto) => Ok(Json(dto)),
        Err(FsFileError::Forbidden) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Access denied to this file" })),
        )),
        Err(FsFileError::Traversal) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Path traversal detected" })),
        )),
        Err(FsFileError::TooLarge) => Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({ "error": "File exceeds 500KB limit" })),
        )),
        Err(FsFileError::Internal(msg)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )),
    }
}

enum FsFileError {
    Forbidden,
    Traversal,
    TooLarge,
    Internal(String),
}
