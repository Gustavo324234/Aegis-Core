use crate::state::AppState;
use ank_core::agents::event::AgentEvent;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use tokio::sync::broadcast;

use tracing::{info, warn};

pub fn router() -> Router<AppState> {
    Router::new().route("/{tenant_id}", get(ws_agents_handler))
}

/// WebSocket ws/agents/{tenant_id} — stream de AgentEvent para la UI (CORE-200).
/// Envía un TreeSnapshot al conectarse, luego eventos incrementales.
pub async fn ws_agents_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agents_stream(socket, tenant_id, state))
}

async fn handle_agents_stream(mut socket: WebSocket, tenant_id: String, state: AppState) {
    info!(tenant = %tenant_id, "[AgentWS] Client connected.");

    // Enviar snapshot inicial del árbol
    let snapshot = {
        let nodes = state.agent_orchestrator.tree_snapshot().await;
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
                        // Solo enviamos eventos del tenant correcto
                        if !agent_event_matches_tenant(&evt, &tenant_id) {
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

fn agent_event_matches_tenant(_evt: &AgentEvent, _tenant_id: &str) -> bool {
    // Todos los eventos son visibles para el tenant conectado.
    // En un sistema multi-tenant real se filtraría por project_id → tenant_id.
    true
}
