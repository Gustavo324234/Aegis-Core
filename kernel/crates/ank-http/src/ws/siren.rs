use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::stream::StreamExt;
use serde_json::json;
use crate::{
    citadel::hash_passphrase,
    state::AppState,
};
use tracing::{info, error, warn};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:tenant_id", get(ws_siren_handler))
}

pub async fn ws_siren_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let session_key = extract_session_key(&headers);

    ws.protocols(["session-key"])
        .on_upgrade(move |socket| handle_siren(socket, tenant_id, session_key, state))
}

fn extract_session_key(headers: &HeaderMap) -> Option<String> {
    headers.get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').find(|p| p.trim().starts_with("session-key.")))
        .map(|p| p.trim().replace("session-key.", ""))
}

async fn handle_siren(
    mut socket: WebSocket,
    tenant_id: String,
    raw_session_key: Option<String>,
    state: AppState,
) {
    let session_key = match raw_session_key {
        Some(k) => k,
        None => {
            let _ = socket.close().await;
            return;
        }
    };

    let hash = hash_passphrase(&session_key);

    // 1. Authenticate
    {
        let citadel = state.citadel.lock().await;
        if citadel.enclave.authenticate_tenant(&tenant_id, &hash).await.is_err() {
            let _ = socket.send(Message::Text(json!({
                "event": "error",
                "data": "Siren Auth Failed: Access Denied."
            }).to_string())).await;
            let _ = socket.close().await;
            return;
        }
    }

    info!("Siren Stream: WebSocket established for {}", tenant_id);

    // 2. Loop
    let mut sequence_number = 0u64;

    while let Some(msg_res) = socket.next().await {
        match msg_res {
            Ok(Message::Binary(_data)) => {
                sequence_number += 1;
                // En un binario unificado, aquí llamaríamos al componente de procesamiento de audio
                // Para ahora, logeamos y devolvemos un evento de procesamiento mock
                // (Referencia CORE-015: Construir AudioChunk proto y enviar al SirenService)
                
                // Mock response para debug en UI
                if sequence_number % 50 == 0 {
                    let event = json!({
                        "event": "siren_event",
                        "data": {
                           "event_type": "AUDIO_PROCESSED",
                           "message": format!("Processed chunk {}", sequence_number),
                           "processed_sequence_number": sequence_number
                        }
                    });
                    if let Err(e) = socket.send(Message::Text(event.to_string())).await {
                        error!("Failed to send siren event: {}", e);
                        break;
                    }
                }
            }
            Ok(Message::Text(t)) => {
                // Posible comando de configuración o stop
                warn!("Received unexpected text message on siren WS: {}", t);
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                error!("Siren WS error: {}", e);
                break;
            }
            _ => continue,
        }
    }

    info!("Siren Stream: WebSocket closed for {}", tenant_id);
}
