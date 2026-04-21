use crate::{citadel::hash_passphrase, state::AppState};
use ank_core::{pcb::PCB, scheduler::SchedulerEvent};
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
use futures::StreamExt;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::sync::LazyLock;
use tokio::sync::oneshot;
use tokio_stream::wrappers::BroadcastStream;

#[allow(clippy::expect_used)]
static MUSIC_PLAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[MUSIC_PLAY:(spotify|youtube):([A-Za-z0-9_:%-]{5,50})\]")
        .expect("FATAL: music play regex is invalid")
});

#[allow(clippy::expect_used)]
static MUSIC_CTRL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(MUSIC_PAUSE|MUSIC_RESUME|MUSIC_STOP|MUSIC_VOLUME:(\d{1,3}))\]")
        .expect("FATAL: music control regex is invalid")
});

pub fn router() -> Router<AppState> {
    Router::new().route("/:tenant_id", get(ws_chat_handler))
}

pub async fn ws_chat_handler(
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

    ws.on_upgrade(move |socket| handle_chat(socket, tenant_id, session_key, state))
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

#[derive(Deserialize)]
struct ChatAction {
    #[serde(default = "default_action")]
    action: String,
    prompt: Option<String>,
    pid: Option<String>,
}

fn default_action() -> String {
    "submit".to_string()
}

async fn handle_chat(
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
                        "data": "Citadel AUTH_FAILURE: Access Denied."
                    })
                    .to_string(),
                ))
                .await;
            let _ = socket.close().await;
            return;
        }
    }

    // 2. Welcome Syslog
    let _ = socket
        .send(Message::Text(
            json!({
                "event": "syslog",
                "data": format!("Aegis Shell established secure bridge for tenant: {}", tenant_id)
            })
            .to_string(),
        ))
        .await;

    // 3. Loop
    while let Some(Ok(msg)) = socket.next().await {
        let msg_text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let action: ChatAction = match serde_json::from_str(&msg_text) {
            Ok(a) => a,
            Err(_) => {
                let _ = socket
                    .send(Message::Text(
                        json!({"event": "error", "data": "Invalid JSON"}).to_string(),
                    ))
                    .await;
                continue;
            }
        };

        if action.action == "watch" {
            if let Some(pid) = action.pid {
                let _ = socket
                    .send(Message::Text(
                        json!({
                            "event": "status",
                            "data": format!("Watching Task PID: {}", pid),
                            "pid": pid
                        })
                        .to_string(),
                    ))
                    .await;
                stream_task_events(&mut socket, &pid, &state).await;
            } else {
                let _ = socket
                    .send(Message::Text(
                        json!({"event": "error", "data": "Missing pid for watch action"})
                            .to_string(),
                    ))
                    .await;
            }
        } else {
            // submit
            let prompt = match action.prompt {
                Some(p) => p,
                None => {
                    let _ = socket
                        .send(Message::Text(
                            json!({"event": "error", "data": "Empty prompt received"}).to_string(),
                        ))
                        .await;
                    continue;
                }
            };

            let _ = socket
                .send(Message::Text(
                    json!({ "event": "status", "data": "Submitting task to ANK..." }).to_string(),
                ))
                .await;

            let pref =
                if let Ok(Some(profile)) = state.persistence.get_voice_profile(&tenant_id).await {
                    use std::str::FromStr;
                    ank_core::scheduler::ModelPreference::from_str(&profile.model_pref)
                        .unwrap_or(ank_core::scheduler::ModelPreference::HybridSmart)
                } else {
                    // Fallback a variable de entorno configurada por el instalador
                    std::env::var("DEFAULT_MODEL_PREF")
                        .ok()
                        .and_then(|s| {
                            use std::str::FromStr;
                            ank_core::scheduler::ModelPreference::from_str(&s).ok()
                        })
                        .unwrap_or(ank_core::scheduler::ModelPreference::HybridSmart)
                };
            let mut pcb = PCB::new(tenant_id.clone(), 5, prompt);
            pcb.model_pref = pref;
            pcb.tenant_id = Some(tenant_id.clone());
            pcb.session_key = Some(hash.clone());
            let pid = pcb.pid.clone();

            // CORE-FIX: Suscribirse al broadcast channel ANTES de enviar la tarea
            // al scheduler para evitar el race condition donde el HAL Runner termina
            // la inferencia antes de que el WebSocket esté escuchando.
            let receiver = {
                let mut broker = state.event_broker.write().await;
                let sender = broker.entry(pid.clone()).or_insert_with(|| {
                    let (tx, _) = tokio::sync::broadcast::channel(512);
                    tx
                });
                sender.subscribe()
            };

            let (tx, rx) = oneshot::channel();

            if let Err(e) = state
                .scheduler_tx
                .send(SchedulerEvent::ScheduleTaskConfirmed(Box::new(pcb), tx))
                .await
            {
                let _ = socket
                    .send(Message::Text(
                        json!({ "event": "error", "data": format!("Scheduler down: {}", e) })
                            .to_string(),
                    ))
                    .await;
                continue;
            }

            // Esperar confirmación del PID (best-effort)
            let confirmed_pid = rx.await.unwrap_or(pid);

            let _ = socket
                .send(Message::Text(
                    json!({
                        "event": "status",
                        "data": format!("Task accepted. PID: {}", confirmed_pid),
                        "pid": confirmed_pid
                    })
                    .to_string(),
                ))
                .await;

            // Streamear con el receiver ya suscrito antes del dispatch
            stream_with_receiver(&mut socket, receiver).await;
        }
    }
}

async fn stream_task_events(socket: &mut WebSocket, pid: &str, state: &AppState) {
    let receiver = {
        let mut broker = state.event_broker.write().await;
        let sender = broker.entry(pid.to_string()).or_insert_with(|| {
            let (tx, _) = tokio::sync::broadcast::channel(512);
            tx
        });
        sender.subscribe()
    };
    stream_with_receiver(socket, receiver).await;
}

async fn stream_with_receiver(
    socket: &mut WebSocket,
    receiver: tokio::sync::broadcast::Receiver<ank_proto::v1::TaskEvent>,
) {
    let mut stream = BroadcastStream::new(receiver);

    while let Some(Ok(proto_event)) = stream.next().await {
        if let Some(ref payload) = proto_event.payload {
            if let ank_proto::v1::task_event::Payload::Output(ref text) = payload {
                if let Some(caps) = MUSIC_PLAY_RE.captures(text) {
                    let provider = caps[1].to_string();
                    let track_id = caps[2].to_string();

                    let data = if provider == "spotify" {
                        json!({
                            "provider": provider,
                            "track_uri": track_id,
                            "track_id": track_id
                        })
                    } else {
                        json!({
                            "provider": provider,
                            "video_id": track_id
                        })
                    };

                    let _ = socket
                        .send(Message::Text(
                            json!({
                                "event": "music_play",
                                "data": data
                            })
                            .to_string(),
                        ))
                        .await;
                    continue;
                }
                if let Some(caps) = MUSIC_CTRL_RE.captures(text) {
                    let tag = &caps[1];
                    let (action, value) = if tag.starts_with("MUSIC_VOLUME:") {
                        ("volume", caps.get(2).map(|m| m.as_str()).unwrap_or("70"))
                    } else {
                        (match tag {
                            "MUSIC_PAUSE" => "pause",
                            "MUSIC_RESUME" => "resume",
                            "MUSIC_STOP" => "stop",
                            _ => "unknown",
                        }, "")
                    };
                    let _ = socket
                        .send(Message::Text(
                            json!({
                                "event": "music_control",
                                "data": { "action": action, "value": value }
                            })
                            .to_string(),
                        ))
                        .await;
                    continue;
                }
            }

            let data = match payload {
                ank_proto::v1::task_event::Payload::Thought(t) => json!({ "thought": t }),
                ank_proto::v1::task_event::Payload::Output(o) => json!({ "output": o }),
                ank_proto::v1::task_event::Payload::StatusUpdate(s) => {
                    let state_str = match s.state {
                        0 => "STATE_NEW",
                        1 => "STATE_READY",
                        2 => "STATE_RUNNING",
                        3 => "STATE_WAITING_SYSCALL",
                        4 => "STATE_COMPLETED",
                        5 => "STATE_FAILED",
                        _ => "UNKNOWN",
                    };
                    json!({ "status_update": { "state": state_str } })
                }
                ank_proto::v1::task_event::Payload::Error(e) => json!({ "error": e }),
                ank_proto::v1::task_event::Payload::Syscall(s) => json!({ "syscall": s.name }),
            };

            let _ = socket
                .send(Message::Text(
                    json!({ "event": "kernel_event", "data": data }).to_string(),
                ))
                .await;

            if let ank_proto::v1::task_event::Payload::StatusUpdate(ref s) = payload {
                if s.state == 4 || s.state == 5 {
                    break;
                }
            }
        }
    }
}
