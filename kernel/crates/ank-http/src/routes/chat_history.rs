use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new().route("/history", get(get_chat_history))
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    timestamp: String,
}

// ── GET /api/chat/history?limit=50 ───────────────────────────────────────────

async fn get_chat_history(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Value>, AegisHttpError> {
    let limit = query.limit.unwrap_or(50).min(200);

    let log_path = state
        .config
        .data_dir
        .join("users")
        .join(&auth.tenant_id)
        .join("workspace")
        .join("chat_history.log");

    let content = tokio::fs::read_to_string(&log_path)
        .await
        .unwrap_or_default();

    let messages = parse_chat_history(&content, limit);

    Ok(Json(json!({ "messages": messages })))
}

fn parse_chat_history(content: &str, limit: usize) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = content
        .lines()
        .filter(|line| !line.trim().is_empty() && line.starts_with('['))
        .filter_map(parse_line)
        .collect();

    if messages.len() > limit {
        let start = messages.len() - limit;
        messages.drain(..start);
    }

    messages
}

fn parse_line(line: &str) -> Option<ChatMessage> {
    // Expected format: [ISO8601] ROLE: content
    let rest = line.strip_prefix('[')?;
    let (timestamp, rest) = rest.split_once(']')?;
    let rest = rest.trim_start();
    let (role_raw, content) = rest.split_once(':')?;
    let role_raw = role_raw.trim();
    let content = content.trim().to_string();
    let timestamp = timestamp.trim().to_string();

    if timestamp.is_empty() || content.is_empty() {
        return None;
    }

    let role = if role_raw.eq_ignore_ascii_case("USER") {
        "user"
    } else {
        "assistant"
    }
    .to_string();

    Some(ChatMessage {
        role,
        content,
        timestamp,
    })
}
