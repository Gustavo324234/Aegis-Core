use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::HeaderMap,
    response::Response,
    routing::get,
    Router,
};
use base64::Engine as _;
use futures::stream::StreamExt;
use serde_json::json;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

pub fn router() -> Router<AppState> {
    Router::new().route("/:tenant_id", get(ws_siren_handler))
}

pub async fn ws_siren_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (session_key, protocol_header) = match crate::citadel::authenticate_ws_upgrade(
        &state, &tenant_id, &headers, "SirenWS",
    )
    .await
    {
        Ok(res) => res,
        Err(resp) => return resp,
    };

    let ws = if let Some(proto) = protocol_header {
        ws.protocols([proto])
    } else {
        ws
    };

    ws.on_upgrade(move |socket| handle_siren(socket, tenant_id, session_key, state))
}

async fn handle_siren(
    mut socket: WebSocket,
    tenant_id: String,
    session_key_hash: String,
    state: AppState,
) {
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
                    if pcm_data.is_empty() {
                        let _ = socket
                            .send(Message::Text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "STT_EMPTY" }
                                })
                                .to_string(),
                            ))
                            .await;
                        continue;
                    }

                    let transcript = match state
                        .siren_router
                        .process_audio(&tenant_id, pcm_data)
                        .await
                    {
                        Ok(t) => t,
                        Err(e) => {
                            error!("Siren: STT Processing failed: {}", e);
                            let _ = socket
                                    .send(Message::Text(
                                        json!({
                                            "event": "siren_event",
                                            "data": { "event_type": "STT_ERROR", "message": e.to_string() }
                                        })
                                        .to_string(),
                                    ))
                                    .await;
                            continue;
                        }
                    };

                    if transcript.trim().is_empty() {
                        info!("Siren WS: Empty STT transcript; skipping LLM task.");
                        let _ = socket
                            .send(Message::Text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "STT_EMPTY" }
                                })
                                .to_string(),
                            ))
                            .await;
                        continue;
                    }

                    let mut pcb =
                        ank_core::PCB::new("Voice Task".to_string(), 5, transcript.clone());
                    pcb.tenant_id = Some(tenant_id.clone());
                    pcb.session_key = Some(session_key_hash.clone());
                    pcb.task_type = ank_core::pcb::TaskType::Chat;
                    let pid = pcb.pid.clone();

                    let (output_tx, output_rx) = oneshot::channel::<String>();

                    if let Err(e) = state
                        .scheduler_tx
                        .send(ank_core::SchedulerEvent::ScheduleTaskConfirmed(
                            Box::new(pcb),
                            output_tx,
                        ))
                        .await
                    {
                        error!("Failed to schedule STT transcript: {}", e);
                        continue;
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

                    // CORE-185: Wait for LLM output then synthesize and stream TTS chunks.
                    let llm_output =
                        match tokio::time::timeout(std::time::Duration::from_secs(60), output_rx)
                            .await
                        {
                            Ok(Ok(output)) => output,
                            Ok(Err(_)) => {
                                warn!("Siren: LLM output channel closed without sending.");
                                continue;
                            }
                            Err(_) => {
                                warn!("Siren: LLM output timeout after 60s for pid={}", pid);
                                continue;
                            }
                        };

                    let engine = match state.siren_router.resolve(&tenant_id).await {
                        Ok(e) => e,
                        Err(e) => {
                            warn!("Siren: No TTS engine available: {}", e);
                            continue;
                        }
                    };

                    let sample_rate = engine.sample_rate();
                    match engine.synthesize(llm_output).await {
                        Ok(pcm_bytes) => {
                            const CHUNK_SIZE: usize = 8 * 1024;

                            for chunk in pcm_bytes.chunks(CHUNK_SIZE) {
                                let b64 = base64::engine::general_purpose::STANDARD.encode(chunk);
                                let msg = json!({
                                    "event": "siren_event",
                                    "data": {
                                        "tts_audio_chunk": b64,
                                        "sample_rate": sample_rate
                                    }
                                })
                                .to_string();

                                if socket.send(Message::Text(msg)).await.is_err() {
                                    break;
                                }
                            }

                            let _ = socket
                                .send(Message::Text(
                                    json!({
                                        "event": "siren_event",
                                        "data": { "event_type": "TTS_DONE" }
                                    })
                                    .to_string(),
                                ))
                                .await;
                        }
                        Err(e) => {
                            warn!("Siren: TTS synthesis failed: {}", e);
                        }
                    }
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
