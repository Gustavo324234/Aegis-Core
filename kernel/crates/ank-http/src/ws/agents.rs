use crate::{
    citadel::{extract_ws_session_key_and_protocol, verify_ws_tenant, CitadelError},
    state::AppState,
};
use ank_core::agents::event::AgentEvent;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::sync::broadcast;

use tracing::{info, warn};

pub fn router() -> Router<AppState> {
    Router::new().route("/:tenant_id", get(ws_agents_handler))
}

/// WebSocket ws/agents/:tenant_id — stream de AgentEvent para la UI (CORE-200 / CORE-368).
/// Valida credenciales Citadel vía `Sec-WebSocket-Protocol: session-key.<key>` (o `x-citadel-key`),
/// devuelve el subprotocolo en el handshake 101, y envía `TreeSnapshot` + eventos incrementales.
pub async fn ws_agents_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (Some(session_key), protocol_header) = extract_ws_session_key_and_protocol(&headers) else {
        warn!(
            tenant = %tenant_id,
            "[AgentWS] Rejected unauthenticated WebSocket upgrade (missing key)"
        );
        return CitadelError::MissingKey.into_response();
    };

    if let Err(err) = verify_ws_tenant(&state, &tenant_id, &session_key).await {
        warn!(
            tenant = %tenant_id,
            error = %err,
            "[AgentWS] Rejected unauthorized WebSocket upgrade"
        );
        return err.into_response();
    }

    let ws = if let Some(proto) = protocol_header {
        ws.protocols([proto])
    } else {
        ws
    };

    ws.on_upgrade(move |socket| handle_agents_stream(socket, tenant_id, state))
}

async fn handle_agents_stream(mut socket: WebSocket, tenant_id: String, state: AppState) {
    info!(tenant = %tenant_id, "[AgentWS] Client connected.");

    // CORE-358: Enviar snapshot inicial del árbol filtrado por tenant
    let snapshot = {
        let nodes = state
            .agent_orchestrator
            .tree_snapshot_for_tenant(&tenant_id)
            .await;
        AgentEvent::TreeSnapshot { nodes }
    };

    if let Ok(json) = serde_json::to_string(&snapshot) {
        if socket.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Suscribirse al broadcast de eventos del orquestador
    let mut event_rx = state.agent_event_tx.subscribe();

    loop {
        tokio::select! {
            // Eventos del orquestador → cliente
            event = event_rx.recv() => {
                match event {
                    Ok(evt) => {
                        // CORE-FIX (security): the AgentEvent broadcast is a single
                        // global channel — only forward events for THIS tenant,
                        // resolving the owner via the agent tree.
                        if !state
                            .agent_orchestrator
                            .agent_event_belongs_to_tenant(&evt, &tenant_id)
                            .await
                        {
                            continue;
                        }
                        match serde_json::to_string(&evt) {
                            Ok(json) => {
                                if socket.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => warn!("[AgentWS] Failed to serialize event: {}", e),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[AgentWS] tenant={} lagged behind by {} events.", tenant_id, n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Mensajes del cliente (ping/pong o cierre)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }

    info!(tenant = %tenant_id, "[AgentWS] Client disconnected.");
}
