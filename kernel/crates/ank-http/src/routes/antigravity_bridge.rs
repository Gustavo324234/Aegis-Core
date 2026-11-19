//! CORE-385: Endpoints HTTP para la integración con Google Antigravity SDK e IDE Hooks.

use crate::{
    citadel::CitadelAuthenticated,
    routes::status::{is_direct_local_cli_probe, OptionalPeerAddr},
    state::AppState,
};
use ank_core::agents::event::AgentEvent;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_antigravity_status))
        .route("/config", post(update_antigravity_config))
        .route("/ide/notify", post(ide_notify))
        .route("/ide/outbox", post(push_ide_outbox))
        .route("/ide/outbox/pop", get(pop_ide_outbox))
}

#[derive(Debug, Serialize)]
pub struct AntigravityStatusResponse {
    pub sdk_installed: bool,
    pub version: String,
    pub mode: String,
    pub active_sessions: Vec<String>,
    pub use_antigravity_sdk: bool,
    pub pending_outbox_count: usize,
    pub error: Option<String>,
}

async fn get_antigravity_status(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> impl IntoResponse {
    let sdk = state.agent_orchestrator.agy_runner.check_status().await;
    let pending_outbox_count = state
        .agent_orchestrator
        .agy_runner
        .pending_ide_outbox_count()
        .await;

    Json(AntigravityStatusResponse {
        sdk_installed: sdk.sdk_installed,
        version: sdk.version.unwrap_or_else(|| "0.1.23".to_string()),
        mode: sdk.mode,
        active_sessions: sdk.active_sessions,
        use_antigravity_sdk: state.agent_orchestrator.agy_runner.is_enabled(),
        pending_outbox_count,
        error: sdk.error,
    })
}

#[derive(Debug, Deserialize)]
pub struct AntigravityConfigRequest {
    pub use_antigravity_sdk: bool,
}

async fn update_antigravity_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<AntigravityConfigRequest>,
) -> impl IntoResponse {
    state
        .agent_orchestrator
        .agy_runner
        .set_enabled(body.use_antigravity_sdk);

    if let Ok(db) = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash) {
        let _ = db.set_kv(
            "use_antigravity_sdk",
            if body.use_antigravity_sdk {
                "true"
            } else {
                "false"
            },
        );
    }

    Json(json!({
        "status": "ok",
        "use_antigravity_sdk": state.agent_orchestrator.agy_runner.is_enabled()
    }))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn is_authorized_hook_or_local(
    state: &AppState,
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
) -> bool {
    if let Ok(expected) = std::env::var("AEGIS_HOOK_TOKEN") {
        let expected_trimmed = expected.trim();
        if !expected_trimmed.is_empty() {
            if let Some(provided) = headers
                .get("x-aegis-hook-token")
                .and_then(|v| v.to_str().ok())
            {
                if constant_time_eq(provided.trim().as_bytes(), expected_trimmed.as_bytes()) {
                    return true;
                }
            }
        }
    }

    let citadel_tenant = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok());
    let citadel_key = headers.get("x-citadel-key").and_then(|v| v.to_str().ok());
    if let (Some(t), Some(k)) = (citadel_tenant, citadel_key) {
        if crate::citadel::verify_ws_tenant(state, t, k).await.is_ok() {
            return true;
        }
    }

    is_direct_local_cli_probe(peer_addr, headers)
}

#[derive(Debug, Deserialize)]
pub struct IdeNotifyRequest {
    pub conversation_id: Option<String>,
    pub project_name: Option<String>,
    pub summary: String,
}

async fn ide_notify(
    State(state): State<AppState>,
    headers: HeaderMap,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
    Json(body): Json<IdeNotifyRequest>,
) -> impl IntoResponse {
    if !is_authorized_hook_or_local(&state, &headers, peer_addr).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let summary = body.summary.trim().to_string();
    if summary.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "summary cannot be empty" })),
        )
            .into_response();
    }

    let project_name = body
        .project_name
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            body.conversation_id
                .as_ref()
                .map(|cid| format!("Antigravity ({})", &cid[..cid.len().min(8)]))
        })
        .unwrap_or_else(|| "Antigravity IDE".to_string());

    state
        .agent_orchestrator
        .emit_event(AgentEvent::SupervisorCompleted {
            agent_id: uuid::Uuid::nil(),
            project_name: project_name.clone(),
            summary,
        });

    Json(json!({
        "status": "notified",
        "project_name": project_name
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct PushOutboxRequest {
    pub instruction: String,
    pub project_name: Option<String>,
    pub dispatch_sdk: Option<bool>,
}

async fn push_ide_outbox(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<PushOutboxRequest>,
) -> impl IntoResponse {
    let instruction = body.instruction.trim().to_string();
    if instruction.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "instruction cannot be empty" })),
        )
            .into_response();
    }

    state
        .agent_orchestrator
        .agy_runner
        .push_ide_outbox(instruction.clone())
        .await;

    let should_dispatch = body
        .dispatch_sdk
        .unwrap_or_else(|| state.agent_orchestrator.agy_runner.is_enabled());

    let mut dispatched_agent_id: Option<String> = None;
    if should_dispatch {
        let project = body
            .project_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Antigravity".to_string());

        if let Ok(agent_id) = state
            .agent_orchestrator
            .create_project(
                project,
                instruction.clone(),
                Some(ank_core::pcb::TaskType::Code),
                Some(auth.tenant_id.clone()),
            )
            .await
        {
            let _ = state
                .agent_orchestrator
                .dispatch(agent_id, instruction, vec![])
                .await;
            let nodes = state
                .agent_orchestrator
                .tree_snapshot_for_tenant(&auth.tenant_id)
                .await;
            state
                .agent_orchestrator
                .emit_event(AgentEvent::TreeSnapshot { nodes });
            dispatched_agent_id = Some(agent_id.to_string());
        }
    }

    let pending_count = state
        .agent_orchestrator
        .agy_runner
        .pending_ide_outbox_count()
        .await;

    Json(json!({
        "status": "queued",
        "dispatched_agent_id": dispatched_agent_id,
        "pending_outbox_count": pending_count
    }))
    .into_response()
}

async fn pop_ide_outbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    OptionalPeerAddr(peer_addr): OptionalPeerAddr,
) -> impl IntoResponse {
    if !is_authorized_hook_or_local(&state, &headers, peer_addr).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    match state.agent_orchestrator.agy_runner.pop_ide_outbox().await {
        Some(instruction) => Json(json!({
            "has_message": true,
            "instruction": instruction
        }))
        .into_response(),
        None => Json(json!({
            "has_message": false,
            "instruction": serde_json::Value::Null
        }))
        .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_hook_access_requires_direct_loopback_without_proxy_headers() {
        let local_peer = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 51234));
        let remote_peer = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5)),
            51234,
        ));

        let mut local_headers = HeaderMap::new();
        local_headers.insert("host", HeaderValue::from_static("127.0.0.1:8090"));
        assert!(is_direct_local_cli_probe(local_peer, &local_headers));
        assert!(!is_direct_local_cli_probe(remote_peer, &local_headers));

        let mut proxied_headers = local_headers.clone();
        proxied_headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.1"));
        assert!(!is_direct_local_cli_probe(local_peer, &proxied_headers));

        let mut browser_headers = local_headers.clone();
        browser_headers.insert("origin", HeaderValue::from_static("https://evil.example"));
        assert!(!is_direct_local_cli_probe(local_peer, &browser_headers));

        let mut fetch_meta_headers = local_headers;
        fetch_meta_headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!is_direct_local_cli_probe(local_peer, &fetch_meta_headers));
    }
}
