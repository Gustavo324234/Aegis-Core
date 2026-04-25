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
use chrono;
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
        .unwrap_or_else(|_| panic!("FATAL: music play regex is invalid"))
});

#[allow(clippy::expect_used)]
static MUSIC_CTRL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(MUSIC_PAUSE|MUSIC_RESUME|MUSIC_STOP|MUSIC_VOLUME:(\d{1,3}))\]")
        .unwrap_or_else(|_| panic!("FATAL: music control regex is invalid"))
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

    // 2.5 Onboarding check
    let should_onboard = {
        match ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            Ok(db) => {
                let has_persona = db.get_persona().ok().flatten().is_some();
                let has_step = db.get_onboarding_step().ok().flatten().is_some();
                !has_persona && !has_step
            }
            Err(_) => false,
        }
    };

    if should_onboard {
        // Iniciar onboarding — guardar step e inmediatamente saludar
        if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            let _ = db.set_onboarding_step("awaiting_name");
        }
        let greeting = "Hola! Soy tu nuevo asistente personal 👋\n¿Cómo querés que me llame?";
        send_onboarding_message(&mut socket, greeting).await;
        let _ = append_to_chat_history(&tenant_id, "ASSISTANT", greeting).await;
    }

    // 3. Subscribe to workspace events (CORE-175)
    let mut workspace_rx = state.workspace_events.subscribe();

    // 4. Loop
    while let Some(Ok(msg)) = socket.next().await {
        // Forward pending workspace events for this tenant before processing message
        loop {
            match workspace_rx.try_recv() {
                Ok(event) if event.tenant_id == tenant_id => {
                    let _ = socket.send(Message::Text(event.payload.to_string())).await;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
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

            // Intercept onboarding steps
            let onboarding_step = {
                ank_core::enclave::TenantDB::open(&tenant_id, &hash)
                    .ok()
                    .and_then(|db| db.get_onboarding_step().ok().flatten())
            };

            match onboarding_step.as_deref() {
                // ── STEP 1: El usuario está respondiendo con el nombre ──
                Some("awaiting_name") => {
                    let name = prompt.trim().to_string();
                    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
                        let _ = db.set_onboarding_name(&name);
                        let _ = db.set_onboarding_step("awaiting_style");
                    }

                    let msg = format!(
                        "¡{}! Me encanta ese nombre 😄\n\
                         A partir de ahora me llamaré {}.\n\n\
                         Ahora dime, ¿qué tipo de personalidad preferís que adopte?\n\n\
                         · Profesional y preciso\n\
                         · Casual y amigable\n\
                         · Directo y sin rodeos\n\
                         · Curioso y creativo\n\n\
                         (o describímela con tus palabras)",
                        name, name
                    );
                    let _ = append_to_chat_history(&tenant_id, "USER", &prompt).await;
                    send_onboarding_message(&mut socket, &msg).await;
                    let _ = append_to_chat_history(&tenant_id, "ASSISTANT", &msg).await;
                    continue;
                }

                // ── STEP 2: El usuario está eligiendo la personalidad ──
                Some("awaiting_style") => {
                    let style_input = prompt.trim().to_string();

                    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
                        let name = db
                            .get_onboarding_name()
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| "Aegis".to_string());

                        let style_desc = map_style_to_description(&style_input);

                        let persona = format!(
                            "Tu nombre es {}. {}\n\
                             Eres el asistente personal del usuario. Cuando sea apropiado \
                             te referís a vos mismo como {}. Mantenés este estilo en \
                             todas tus respuestas sin excepción.",
                            name, style_desc, name
                        );

                        let _ = db.set_persona(&persona);
                        let _ = db.clear_onboarding();

                        let msg = format!(
                            "Perfecto! Ya soy **{}**, tu asistente {} 🚀\n\n\
                             Podés cambiar mi personalidad cuando quieras desde \
                             Configuración.\n\n\
                             ¿En qué te puedo ayudar hoy?",
                            name,
                            friendly_style_label(&style_input)
                        );
                        let _ = append_to_chat_history(&tenant_id, "USER", &prompt).await;
                        send_onboarding_message(&mut socket, &msg).await;
                        let _ = append_to_chat_history(&tenant_id, "ASSISTANT", &msg).await;
                    }
                    continue;
                }
                _ => {}
            }

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
            let mut pcb = PCB::new(tenant_id.clone(), 5, prompt.clone());
            pcb.model_pref = pref;
            pcb.tenant_id = Some(tenant_id.clone());
            pcb.session_key = Some(hash.clone());

            // CORE-FIX: Enable conversation history and semantic memory
            pcb.memory_pointers
                .l2_context_refs
                .push("file://chat_history.log".to_string());
            pcb.memory_pointers
                .swap_refs
                .push("semantic_memory".to_string());

            // Save user prompt to history
            let _ = append_to_chat_history(&tenant_id, "USER", &prompt).await;

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
            let full_response = stream_with_receiver(&mut socket, receiver).await;

            // Save assistant response to history
            if !full_response.is_empty() {
                let _ = append_to_chat_history(&tenant_id, "ASSISTANT", &full_response).await;
            }
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
) -> String {
    let mut full_output = String::new();
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
                        (
                            match tag {
                                "MUSIC_PAUSE" => "pause",
                                "MUSIC_RESUME" => "resume",
                                "MUSIC_STOP" => "stop",
                                _ => "unknown",
                            },
                            "",
                        )
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
                ank_proto::v1::task_event::Payload::Output(o) => {
                    full_output.push_str(o);
                    json!({ "output": o })
                }
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
    full_output
}

/// Appends a message to the tenant's chat history log.
async fn append_to_chat_history(tenant_id: &str, role: &str, text: &str) -> anyhow::Result<()> {
    let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace_path = std::path::Path::new(&base_dir)
        .join("users")
        .join(tenant_id)
        .join("workspace");

    let _ = tokio::fs::create_dir_all(&workspace_path).await;
    let log_path = workspace_path.join("chat_history.log");

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await?;

    let entry = format!(
        "\n[{}] {}: {}\n",
        chrono::Utc::now().to_rfc3339(),
        role,
        text
    );
    file.write_all(entry.as_bytes()).await?;
    Ok(())
}

/// Convierte la elección del usuario en una instrucción de Persona.
/// Si no coincide con ninguna opción, usa el texto libre directamente.
fn map_style_to_description(input: &str) -> String {
    let s = input.to_lowercase();
    if s.contains("profesional") || s.contains("preciso") || s.contains("formal") {
        "Sos profesional y preciso. Comunicás con claridad y rigor, \
         sin lenguaje informal."
            .to_string()
    } else if s.contains("casual") || s.contains("amigable") || s.contains("cercano") {
        "Sos casual y amigable, como un amigo de confianza. \
         Usás un tono cálido, natural y relajado."
            .to_string()
    } else if s.contains("directo") || s.contains("sin rodeos") || s.contains("conciso") {
        "Sos directo y sin rodeos. Respondés de forma breve y clara, \
         sin relleno innecesario."
            .to_string()
    } else if s.contains("curioso") || s.contains("creativo") || s.contains("expresivo") {
        "Sos curioso y creativo. Aportás perspectivas originales \
         y no tenés miedo de ser expresivo."
            .to_string()
    } else {
        // Usar el texto libre del usuario como instrucción directa
        format!("Tu estilo de comunicación: {}.", input)
    }
}

/// Label amigable para el mensaje de confirmación.
fn friendly_style_label(input: &str) -> &str {
    let s = input.to_lowercase();
    if s.contains("profesional") || s.contains("preciso") {
        "profesional y preciso"
    } else if s.contains("casual") || s.contains("amigable") {
        "casual y amigable"
    } else if s.contains("directo") || s.contains("sin rodeos") {
        "directo y sin rodeos"
    } else if s.contains("curioso") || s.contains("creativo") {
        "curioso y creativo"
    } else {
        "personalizado"
    }
}

/// Envía un mensaje de onboarding al WebSocket como si fuera el agente.
async fn send_onboarding_message(socket: &mut WebSocket, text: &str) {
    let _ = socket
        .send(Message::Text(
            json!({ "event": "kernel_event", "data": { "output": text } }).to_string(),
        ))
        .await;
    let _ = socket
        .send(Message::Text(
            json!({ "event": "kernel_event", "data": {
                "status_update": { "state": "STATE_COMPLETED" }
            }})
            .to_string(),
        ))
        .await;
}
