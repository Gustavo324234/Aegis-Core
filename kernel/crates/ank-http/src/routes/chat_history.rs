use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/history", get(get_chat_history).delete(clear_chat_history))
        .route("/traces", get(get_agent_traces).delete(clear_agent_traces))
        .route(
            "/ui-state",
            get(get_chat_ui_state).patch(update_chat_ui_state),
        )
        .route("/visual-clear", post(visual_clear_chat))
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
    before: Option<usize>,
    respect_clean: Option<bool>,
}

#[derive(Deserialize)]
struct TracesQuery {
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ChatUiState {
    visual_clear_before: usize,
    clean_on_open: bool,
}

impl Default for ChatUiState {
    fn default() -> Self {
        Self {
            visual_clear_before: 0,
            clean_on_open: true,
        }
    }
}

#[derive(Deserialize)]
struct UpdateChatUiStatePayload {
    clean_on_open: bool,
}

struct ChatHistoryPage {
    messages: Vec<ChatMessage>,
    total: usize,
    start_index: usize,
    end_index: usize,
    has_more: bool,
}

fn tenant_workspace_dir(state: &AppState, tenant_id: &str) -> PathBuf {
    state
        .config
        .data_dir
        .join("users")
        .join(tenant_id)
        .join("workspace")
}

async fn load_ui_state(workspace: &Path) -> ChatUiState {
    let path = workspace.join("chat_ui_state.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => ChatUiState::default(),
    }
}

async fn save_ui_state(workspace: &Path, ui_state: &ChatUiState) -> Result<(), AegisHttpError> {
    tokio::fs::create_dir_all(workspace).await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to prepare workspace: {}", e))
    })?;
    let path = workspace.join("chat_ui_state.json");
    let body = serde_json::to_string_pretty(ui_state).map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to serialize chat UI state: {}", e))
    })?;
    tokio::fs::write(&path, body).await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to persist chat UI state: {}", e))
    })
}

// ── GET /api/chat/history?limit=50&before=<idx>&respect_clean=<bool> ─────────

async fn get_chat_history(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let limit = query.limit.unwrap_or(50).min(200);
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let log_path = workspace.join("chat_history.log");

    let content = tokio::fs::read_to_string(&log_path)
        .await
        .unwrap_or_default();
    let ui_state = load_ui_state(&workspace).await;

    let min_start = if query.respect_clean.unwrap_or(false) && query.before.is_none() {
        if ui_state.clean_on_open {
            Some(usize::MAX)
        } else {
            Some(ui_state.visual_clear_before)
        }
    } else {
        None
    };

    let page = slice_chat_history(&content, limit, query.before, min_start);

    Ok(Json(json!({
        "messages": page.messages,
        "total": page.total,
        "start_index": page.start_index,
        "end_index": page.end_index,
        "has_more": page.has_more,
        "visual_clear_before": ui_state.visual_clear_before,
        "clean_on_open": ui_state.clean_on_open,
    })))
}

async fn get_chat_ui_state(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let ui_state = load_ui_state(&workspace).await;
    let content = tokio::fs::read_to_string(workspace.join("chat_history.log"))
        .await
        .unwrap_or_default();
    let total = parse_all_chat_history(&content).len();

    Ok(Json(json!({
        "visual_clear_before": ui_state.visual_clear_before.min(total),
        "clean_on_open": ui_state.clean_on_open,
        "total": total,
    })))
}

async fn update_chat_ui_state(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(payload): Json<UpdateChatUiStatePayload>,
) -> Result<Json<Value>, AegisHttpError> {
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let mut ui_state = load_ui_state(&workspace).await;
    ui_state.clean_on_open = payload.clean_on_open;
    save_ui_state(&workspace, &ui_state).await?;

    Ok(Json(json!({
        "visual_clear_before": ui_state.visual_clear_before,
        "clean_on_open": ui_state.clean_on_open,
    })))
}

async fn visual_clear_chat(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let content = tokio::fs::read_to_string(workspace.join("chat_history.log"))
        .await
        .unwrap_or_default();
    let total = parse_all_chat_history(&content).len();

    let mut ui_state = load_ui_state(&workspace).await;
    ui_state.visual_clear_before = total;
    save_ui_state(&workspace, &ui_state).await?;

    Ok(Json(json!({
        "visual_clear_before": total,
        "clean_on_open": ui_state.clean_on_open,
        "total": total,
        "has_more": total > 0,
    })))
}

pub(crate) async fn load_recent_session_messages(
    data_dir: &Path,
    tenant_id: &str,
    max_messages: usize,
) -> Vec<ank_core::chal::ChatMessage> {
    let log_path = data_dir
        .join("users")
        .join(tenant_id)
        .join("workspace")
        .join("chat_history.log");
    let Ok(content) = tokio::fs::read_to_string(&log_path).await else {
        return Vec::new();
    };
    let parsed = parse_all_chat_history(&content);
    let mut filtered: Vec<ank_core::chal::ChatMessage> = parsed
        .into_iter()
        .filter(|m| {
            let trimmed = m.content.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("⏰ Recordatorio:")
                && !trimmed
                    .starts_with("No pude completar la respuesta tras ejecutar la herramienta")
        })
        .map(|m| ank_core::chal::ChatMessage {
            role: if m.role.eq_ignore_ascii_case("user") {
                ank_core::chal::ChatRole::User
            } else {
                ank_core::chal::ChatRole::Assistant
            },
            content: Some(m.content),
            ..Default::default()
        })
        .collect();
    if filtered.len() > max_messages {
        filtered = filtered.split_off(filtered.len() - max_messages);
    }
    filtered
}

fn parse_all_chat_history(content: &str) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    for raw_line in content.lines() {
        if let Some(msg) = parse_line(raw_line) {
            messages.push(msg);
        } else if let Some(last) = messages.last_mut() {
            if !last.content.is_empty() || !raw_line.trim().is_empty() {
                if !last.content.is_empty() {
                    last.content.push('\n');
                }
                last.content.push_str(raw_line);
            }
        }
    }

    for msg in &mut messages {
        msg.content = msg.content.trim_matches(['\n', '\r']).to_string();
    }
    messages.retain(|m| !m.content.trim().is_empty());
    messages
}

fn slice_chat_history(
    content: &str,
    limit: usize,
    before: Option<usize>,
    min_start: Option<usize>,
) -> ChatHistoryPage {
    let all = parse_all_chat_history(content);
    let total = all.len();
    let end = before.unwrap_or(total).min(total);
    let floor = min_start.unwrap_or(0).min(end);
    let start = end.saturating_sub(limit).max(floor);
    let messages = all[start..end].to_vec();

    ChatHistoryPage {
        messages,
        total,
        start_index: start,
        end_index: end,
        has_more: start > 0,
    }
}

#[cfg(test)]
fn parse_chat_history(content: &str, limit: usize) -> Vec<ChatMessage> {
    slice_chat_history(content, limit, None, None).messages
}

fn is_history_timestamp(ts: &str) -> bool {
    ts.len() >= 19
        && ts.as_bytes()[..4].iter().all(|b| b.is_ascii_digit())
        && ts.as_bytes()[4] == b'-'
        && ts.contains('T')
}

fn parse_line(line: &str) -> Option<ChatMessage> {
    // Expected format: [ISO8601] ROLE: content
    let rest = line.strip_prefix('[')?;
    let (timestamp, rest) = rest.split_once(']')?;
    let timestamp = timestamp.trim().to_string();
    if !is_history_timestamp(&timestamp) {
        return None;
    }

    let rest = rest.trim_start();
    let (role_raw, content) = rest.split_once(':')?;
    let role_raw = role_raw.trim();

    let role = if role_raw.eq_ignore_ascii_case("USER") {
        "user"
    } else if role_raw.eq_ignore_ascii_case("ASSISTANT") || role_raw.eq_ignore_ascii_case("SYSTEM")
    {
        "assistant"
    } else {
        return None;
    }
    .to_string();

    let content = content
        .strip_prefix(' ')
        .unwrap_or(content)
        .trim_end()
        .to_string();

    Some(ChatMessage {
        role,
        content,
        timestamp,
    })
}

// ── GET /api/chat/traces?limit=100 ────────────────────────────────────────────

async fn get_agent_traces(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Query(query): Query<TracesQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let limit = query.limit.unwrap_or(100).min(500);
    let log_path = tenant_workspace_dir(&state, &auth.tenant_id).join("agent_traces.log");

    let content = tokio::fs::read_to_string(&log_path)
        .await
        .unwrap_or_default();

    let mut traces: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|s| s.to_string())
        .collect();

    if traces.len() > limit {
        let start = traces.len() - limit;
        traces.drain(..start);
    }

    Ok(Json(json!({ "traces": traces })))
}

// ── DELETE /api/chat/history ───────────────────────────────────────────────

async fn clear_chat_history(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let log_path = workspace.join("chat_history.log");

    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to prepare workspace: {}", e))
    })?;
    tokio::fs::write(&log_path, "").await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to truncate chat history: {}", e))
    })?;

    let mut ui_state = load_ui_state(&workspace).await;
    ui_state.visual_clear_before = 0;
    save_ui_state(&workspace, &ui_state).await?;

    Ok(Json(json!({ "success": true })))
}

// ── DELETE /api/chat/traces ────────────────────────────────────────────────

async fn clear_agent_traces(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let workspace = tenant_workspace_dir(&state, &auth.tenant_id);
    let log_path = workspace.join("agent_traces.log");

    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to prepare workspace: {}", e))
    })?;
    tokio::fs::write(&log_path, "").await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to truncate agent traces: {}", e))
    })?;

    Ok(Json(json!({ "success": true })))
}

#[cfg(test)]
mod tests {
    use super::{parse_chat_history, slice_chat_history};

    #[test]
    fn parses_multiline_markdown_tables_and_links() {
        let raw = "\n[2026-11-13T03:26:30+00:00] USER: Que modulos tienes?\n\n[2026-11-13T03:26:42+00:00] ASSISTANT: Inventario:\n\n| Tipo | Estado |\n|---|---|\n| Modulo | Desactivado |\n\n[Ver detalle](https://example.com): info\n> Nota final\n\n[2026-11-13T03:27:00+00:00] USER: gracias\n";
        let msgs = parse_chat_history(raw, 10);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Que modulos tienes?");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(
            msgs[1].content,
            "Inventario:\n\n| Tipo | Estado |\n|---|---|\n| Modulo | Desactivado |\n\n[Ver detalle](https://example.com): info\n> Nota final"
        );
        assert_eq!(msgs[2].role, "user");
        assert_eq!(msgs[2].content, "gracias");
    }

    #[test]
    fn enforces_limit_after_combining_multiline_messages() {
        let raw = "[2026-11-13T03:26:01+00:00] USER: uno\n[2026-11-13T03:26:02+00:00] ASSISTANT: dos\nlinea 2\n[2026-11-13T03:26:03+00:00] USER: tres\n";
        let msgs = parse_chat_history(raw, 2);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "dos\nlinea 2");
        assert_eq!(msgs[1].content, "tres");
    }

    #[test]
    fn paginates_infinite_history_with_before_cursor_and_clean_cutoff() {
        let raw = "\
[2026-11-13T03:00:01+00:00] USER: m0
[2026-11-13T03:00:02+00:00] ASSISTANT: m1
[2026-11-13T03:00:03+00:00] USER: m2
[2026-11-13T03:00:04+00:00] ASSISTANT: m3
[2026-11-13T03:00:05+00:00] USER: m4
";
        // Clean on open: returns 0 visible messages while advertising all 5 archived above
        let clean = slice_chat_history(raw, 2, None, Some(usize::MAX));
        assert_eq!(clean.messages.len(), 0);
        assert_eq!(clean.total, 5);
        assert_eq!(clean.start_index, 5);
        assert_eq!(clean.end_index, 5);
        assert!(clean.has_more);

        // First scroll up from start_index = 5 returns m3..m4
        let p1 = slice_chat_history(raw, 2, Some(5), None);
        assert_eq!(p1.messages.len(), 2);
        assert_eq!(p1.messages[0].content, "m3");
        assert_eq!(p1.messages[1].content, "m4");
        assert_eq!(p1.start_index, 3);
        assert!(p1.has_more);

        // Next scroll up with before = 3 returns m1..m2
        let p2 = slice_chat_history(raw, 2, Some(3), None);
        assert_eq!(p2.messages.len(), 2);
        assert_eq!(p2.messages[0].content, "m1");
        assert_eq!(p2.messages[1].content, "m2");
        assert_eq!(p2.start_index, 1);
        assert!(p2.has_more);

        // Final scroll up with before = 1 returns m0 and marks has_more = false
        let p3 = slice_chat_history(raw, 2, Some(1), None);
        assert_eq!(p3.messages.len(), 1);
        assert_eq!(p3.messages[0].content, "m0");
        assert_eq!(p3.start_index, 0);
        assert!(!p3.has_more);

        // Visual cutoff mode (when clean_on_open = false, visual_clear_before = 4) hides m0..m3 and shows m4
        let partial = slice_chat_history(raw, 10, None, Some(4));
        assert_eq!(partial.messages.len(), 1);
        assert_eq!(partial.messages[0].content, "m4");
        assert_eq!(partial.start_index, 4);
        assert!(partial.has_more);
    }
}
