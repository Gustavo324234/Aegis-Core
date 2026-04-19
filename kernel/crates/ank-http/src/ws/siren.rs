use crate::{citadel::hash_passphrase, state::AppState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::stream::StreamExt;
use serde_json::json;
use tracing::{error, info, warn};

pub fn router() -> Router<AppState> {
    Router::new().route("/:tenant_id", get(ws_siren_handler))
}

pub async fn ws_siren_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let (session_key, protocol_header) = extract_session_key_and_protocol(&headers);

    let ws = if let Some(proto) = protocol_header {
        ws.protocols([proto])
    } else {
        ws.protocols(["session-key"])
    };

    ws.on_upgrade(move |socket| handle_siren(socket, tenant_id, session_key, state))
}

fn extract_session_key_and_protocol(headers: &HeaderMap) -> (Option<String>, Option<String>) {
    let header_val = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok());

    let Some(val) = header_val else {
        return (None, None);
    };

    let proto = val
        .split(',')
        .find(|p| p.trim().starts_with("session-key."))
        .map(|p| p.trim().to_string());

    let session_key = proto.as_ref().map(|p| p.replace("session-key.", ""));

    (session_key, proto)
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
        let is_auth = citadel
            .enclave
            .authenticate_tenant(&tenant_id, &hash)
            .await
            .unwrap_or(false);

        if !is_auth {
            let _ = socket
                .send(Message::Text(
                    json!({
                        "event": "error",
                        "data": "Siren Auth Failed: Access Denied."
                    })
                    .to_string(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    }

    info!("Siren Stream: WebSocket established for {}", tenant_id);

    // 2. Loop
    let mut audio_buffer: Vec<u8> = Vec::new();

    while let Some(msg_res) = socket.next().await {
        match msg_res {
            Ok(Message::Binary(data)) => {
                audio_buffer.extend_from_slice(&data);
                if audio_buffer.len() > 10 * 1024 * 1024 {
                    warn!("Siren: Audio buffer exceeded safety cap, clearing.");
                    audio_buffer.clear();
                }
            }
            Ok(Message::Text(t)) => {
                if t.contains("VAD_END_SIGNAL") {
                    let _ = socket
                        .send(Message::Text(
                            json!({
                                "event": "siren_event",
                                "data": { "event_type": "VAD_START" }
                            })
                            .to_string(),
                        ))
                        .await;

                    let _ = socket
                        .send(Message::Text(
                            json!({
                                "event": "siren_event",
                                "data": { "event_type": "STT_START" }
                            })
                            .to_string(),
                        ))
                        .await;

                    let pcm_data = std::mem::take(&mut audio_buffer);
                    let transcript =
                        match state.siren_router.process_audio(&tenant_id, pcm_data).await {
                            Ok(t) => t,
                            Err(e) => {
                                error!("Siren: STT Processing failed: {}", e);
                                let _ = socket.send(Message::Text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "STT_ERROR", "message": e.to_string() }
                                }).to_string()
                            )).await;
                                continue;
                            }
                        };

                    let mut pcb =
                        ank_core::PCB::new("Voice Task".to_string(), 5, transcript.clone());
                    pcb.tenant_id = Some(tenant_id.clone());
                    pcb.session_key = Some(session_key.clone());
                    pcb.task_type = ank_core::pcb::TaskType::Chat;
                    let pid = pcb.pid.clone();

                    if let Err(e) = state
                        .scheduler_tx
                        .send(ank_core::SchedulerEvent::ScheduleTask(Box::new(pcb)))
                        .await
                    {
                        error!("Failed to schedule STT transcript: {}", e);
                    }

                    let payload = json!({ "transcript": transcript, "pid": pid }).to_string();
                    let _ = socket
                        .send(Message::Text(
                            json!({
                                "event": "siren_event",
                                "data": { "event_type": "STT_DONE", "message": payload }
                            })
                            .to_string(),
                        ))
                        .await;
                } else {
                    warn!("Received unexpected text message on siren WS: {}", t);
                }
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
