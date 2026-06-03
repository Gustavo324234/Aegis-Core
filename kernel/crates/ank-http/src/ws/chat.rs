use crate::{citadel::hash_passphrase, state::AppState};
use ank_core::{
    agents::orchestrator::AgentOrchestrator,
    chal::{ChatMessage, ChatRole},
    pcb::{infer_task_type, PCB},
    scheduler::SchedulerEvent,
};
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
use tokio::sync::{broadcast, oneshot};
use tokio_stream::wrappers::BroadcastStream;
use tracing::warn;

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
    #[serde(default)]
    model_override: Option<String>,
}

fn default_action() -> String {
    "submit".to_string()
}

/// CORE-FIX (A4): structured error event for the client. `code` is a stable
/// machine-readable identifier the UI can match on; `message` is in Spanish
/// and meant for end-users; `detail` is an optional technical detail kept
/// out of the user-facing message but available for debugging panels.
fn error_event(code: &str, message: &str, detail: Option<&str>) -> String {
    json!({
        "event": "error",
        "data": {
            "code": code,
            "message": message,
            "detail": detail,
        }
    })
    .to_string()
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
                .send(Message::Text(error_event(
                    "auth_failed",
                    "Sesión inválida o expirada. Reconectate para continuar.",
                    Some("Citadel: tenant authentication rejected"),
                )))
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

    // CORE-260: Cache de historial de mensajes de la sesión WebSocket (máx. 20).
    let session_history: std::sync::Arc<
        tokio::sync::Mutex<std::collections::VecDeque<ChatMessage>>,
    > = std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::VecDeque::new()));

    // 3. Subscribe to workspace events (CORE-175)
    let mut workspace_rx = state.workspace_events.subscribe();

    // CORE-268: Registrar canal de AgentEvents en el orchestrator y suscribirse.
    state
        .agent_orchestrator
        .set_event_channel((*state.agent_event_tx).clone());
    let mut agent_event_rx = state.agent_event_tx.subscribe();

    // Re-attach: a reconnecting client missed every AgentEvent broadcast while it
    // was away. Replay the current tree snapshot and any unanswered supervisor
    // questions so the dashboard repopulates and the question modal re-appears,
    // instead of leaving the user blind to work that's still running.
    {
        let nodes = state
            .agent_orchestrator
            .tree_snapshot_for_tenant(&tenant_id)
            .await;
        if !nodes.is_empty() {
            let snapshot = ank_core::agents::event::AgentEvent::TreeSnapshot { nodes };
            if let Ok(data) = serde_json::to_value(&snapshot) {
                let frame = json!({ "event": "agent_event", "data": data });
                let _ = socket.send(Message::Text(frame.to_string())).await;
            }
        }
        for question in state
            .agent_orchestrator
            .pending_questions_for_tenant(&tenant_id)
            .await
        {
            if let Ok(data) = serde_json::to_value(&question) {
                let frame = json!({ "event": "agent_event", "data": data });
                let _ = socket.send(Message::Text(frame.to_string())).await;
            }
        }
    }

    // CORE-279: Ticker de keepalive — ping cada 30s para mantener la conexión viva
    // a través de proxies (Cloudflare, nginx, etc.) que cierran conexiones idle.
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // 4. Loop
    loop {
        // Forward pending workspace events for this tenant before waiting
        loop {
            match workspace_rx.try_recv() {
                Ok(event) if event.tenant_id == tenant_id => {
                    let _ = socket.send(Message::Text(event.payload.to_string())).await;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        let msg = tokio::select! {
            ws_next = socket.recv() => {
                match ws_next {
                    Some(Ok(m)) => m,
                    _ => break,
                }
            }
            event_result = agent_event_rx.recv() => {
                // CORE-274: envelope estándar { "event": "agent_event", "data": {...} }
                match event_result {
                    Ok(event) => {
                        // CORE-FIX (security): the AgentEvent broadcast is global —
                        // only forward events that belong to THIS tenant.
                        if state
                            .agent_orchestrator
                            .agent_event_belongs_to_tenant(&event, &tenant_id)
                            .await
                        {
                            let _ = append_to_agent_traces(&tenant_id, &event).await;

                            if let Ok(data) = serde_json::to_value(&event) {
                                let frame = json!({ "event": "agent_event", "data": data });
                                let _ = socket.send(Message::Text(frame.to_string())).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[ChatWS] tenant={} agent_event_rx lagged by {} events", tenant_id, n);
                    }
                    Err(_) => {}
                }
                continue;
            }
            // CORE-279: ping keepalive
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
                continue;
            }
        };

        let msg_text = match msg {
            Message::Text(t) => t,
            Message::Pong(_) => continue, // CORE-279: el browser responde automáticamente
            Message::Close(_) => break,
            _ => continue,
        };

        let action: ChatAction = match serde_json::from_str(&msg_text) {
            Ok(a) => a,
            Err(e) => {
                let _ = socket
                    .send(Message::Text(error_event(
                        "invalid_payload",
                        "El mensaje no tiene el formato esperado. Reintentá.",
                        Some(&e.to_string()),
                    )))
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
                stream_task_events(&mut socket, &pid, &state, &mut agent_event_rx, &tenant_id)
                    .await;
            } else {
                let _ = socket
                    .send(Message::Text(error_event(
                        "missing_pid",
                        "Falta el ID de la tarea a observar.",
                        None,
                    )))
                    .await;
            }
        } else {
            // submit
            let prompt = match action.prompt {
                Some(p) => p,
                None => {
                    let _ = socket
                        .send(Message::Text(error_event(
                            "empty_prompt",
                            "Mandaste un mensaje vacío.",
                            None,
                        )))
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
                    // CORE-FIX: parse the user's reply instead of dumping the
                    // whole phrase into the name field. Before this, saying
                    // "hola! te llamare Aegis" stored "hola! te llamare Aegis"
                    // as the assistant's name, producing "Ya soy hola! te
                    // llamare Aegis, tu asistente..." in step 2.
                    let parsed = extract_name_from_phrase(&prompt);
                    let (name, needs_clarification) = match parsed {
                        Some(n) => (n, false),
                        None => (prompt.trim().to_string(), true),
                    };

                    if needs_clarification {
                        // Couldn't extract a clear name — ask again instead of
                        // persisting garbage. Don't advance the step.
                        let _ = append_to_chat_history(&tenant_id, "USER", &prompt).await;
                        let clarify = "No te entendí bien 🙂 ¿Podés decirme \
                                       solo el nombre que querés que use? \
                                       Por ejemplo: \"Aegis\" o \"llamame Lucía\".";
                        send_onboarding_message(&mut socket, clarify).await;
                        let _ = append_to_chat_history(&tenant_id, "ASSISTANT", clarify).await;
                        continue;
                    }

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
            // CORE-FIX: Si el cliente pidió un model_override, verificar que exista en el
            // catálogo antes de aceptarlo. Antes el router lo ignoraba en silencio y caía
            // al CMR — el usuario seleccionaba un modelo y el sistema usaba otro sin avisar.
            let validated_override = match action.model_override {
                Some(model_id) if !model_id.trim().is_empty() => {
                    let exists = state
                        .router
                        .read()
                        .await
                        .catalog_find(&model_id)
                        .await
                        .is_some();
                    if exists {
                        Some(model_id)
                    } else {
                        let _ = socket
                            .send(Message::Text(error_event(
                                "model_override_unknown",
                                &format!(
                                    "El modelo '{}' no está en el catálogo. Sigo con la selección automática.",
                                    model_id
                                ),
                                Some(&format!("unknown model_id: {}", model_id)),
                            )))
                            .await;
                        None
                    }
                }
                _ => None,
            };

            let user_message_text = prompt.clone();
            let mut pcb = PCB::new(tenant_id.clone(), 5, prompt.clone());
            pcb.model_pref = pref;
            pcb.tenant_id = Some(tenant_id.clone());
            pcb.session_key = Some(hash.clone());
            pcb.model_override = validated_override;
            // CORE-FIX: Inferir TaskType del prompt para que el CMR puntúe por el tipo
            // de tarea real (Code/Planning/Analysis/Creative) y no siempre por Chat.
            // Sin esto, un pedido de coding eligía gemini-flash-lite en lugar de claude-sonnet.
            pcb.task_type = infer_task_type(&prompt);

            // CORE-260: Inyectar historial de la sesión en el PCB
            {
                let history = session_history.lock().await;
                pcb.message_history = history.iter().cloned().collect();
            }

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

            // CORE-FIX (A4 + C5): timeout on scheduler send so a stuck scheduler
            // doesn't hang the WS handler indefinitely. 5s is generous —
            // scheduler is in-process via mpsc, normal latency is sub-millisecond.
            let send_result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                state
                    .scheduler_tx
                    .send(SchedulerEvent::ScheduleTaskConfirmed(Box::new(pcb), tx)),
            )
            .await;

            match send_result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let _ = socket
                        .send(Message::Text(error_event(
                            "scheduler_unavailable",
                            "El núcleo no está aceptando tareas. Probá de nuevo en unos segundos.",
                            Some(&e.to_string()),
                        )))
                        .await;
                    continue;
                }
                Err(_) => {
                    let _ = socket
                        .send(Message::Text(error_event(
                            "scheduler_timeout",
                            "El núcleo está saturado y no respondió a tiempo. Probá de nuevo.",
                            Some("scheduler_tx.send timed out after 5s"),
                        )))
                        .await;
                    continue;
                }
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

            // Streamear con el receiver ya suscrito antes del dispatch.
            // CORE-FIX (C): pasamos el receiver de AgentEvents para que las preguntas
            // del supervisor (y demás eventos) sigan llegando a la UI mientras el turno
            // streamea, no solo cuando el handler queda ocioso entre turnos.
            let full_response = stream_with_receiver(
                &mut socket,
                receiver,
                &mut agent_event_rx,
                &tenant_id,
                &state.agent_orchestrator,
            )
            .await;

            // CORE-FIX (C): el turno pudo terminar de forma anómala (429 / output vacío)
            // justo después de que un supervisor preguntara algo. Re-entregamos las
            // preguntas que sigan pendientes para que el modal aparezca aunque el turno
            // padre no haya producido salida. El inbox store deduplica por
            // (agent_id + question), así que una pregunta ya enviada en vivo durante el
            // stream no se muestra dos veces.
            for question in state
                .agent_orchestrator
                .pending_questions_for_tenant(&tenant_id)
                .await
            {
                if let Ok(data) = serde_json::to_value(&question) {
                    let frame = json!({ "event": "agent_event", "data": data });
                    let _ = socket.send(Message::Text(frame.to_string())).await;
                }
            }

            // Save assistant response to history
            if !full_response.is_empty() {
                let _ = append_to_chat_history(&tenant_id, "ASSISTANT", &full_response).await;
            }

            // CORE-260: Actualizar SessionHistoryCache con el par user+assistant
            if !full_response.is_empty() {
                let mut history = session_history.lock().await;
                history.push_back(ChatMessage {
                    role: ChatRole::User,
                    content: Some(user_message_text.clone()),
                    ..Default::default()
                });
                history.push_back(ChatMessage {
                    role: ChatRole::Assistant,
                    content: Some(full_response.clone()),
                    ..Default::default()
                });
                while history.len() > 20 {
                    history.pop_front();
                }
            }
        }
    }

    // Detach, don't cancel. Supervisors are long-running background work that
    // must survive a socket drop (reconnect, navigation, network blip) so they
    // can finish and deliver their result to the chat on reconnect. Killing the
    // whole tenant tree on every disconnect was exactly why supervisors never
    // showed up in the dashboard and never reported back ("te aviso" → silence).
    // The AGENT_IDLE_TIMEOUT (now > ask_user's 600s) is the safety net against
    // genuinely abandoned work: anything that goes idle is reaped there instead.
    tracing::info!(
        tenant = %tenant_id,
        "ws/chat: client disconnected — detaching tenant agents (they keep running)"
    );
}

async fn stream_task_events(
    socket: &mut WebSocket,
    pid: &str,
    state: &AppState,
    agent_event_rx: &mut tokio::sync::broadcast::Receiver<ank_core::agents::event::AgentEvent>,
    tenant_id: &str,
) {
    let receiver = {
        let mut broker = state.event_broker.write().await;
        let sender = broker.entry(pid.to_string()).or_insert_with(|| {
            let (tx, _) = tokio::sync::broadcast::channel(512);
            tx
        });
        sender.subscribe()
    };
    stream_with_receiver(
        socket,
        receiver,
        agent_event_rx,
        tenant_id,
        &state.agent_orchestrator,
    )
    .await;
}

async fn stream_with_receiver(
    socket: &mut WebSocket,
    receiver: tokio::sync::broadcast::Receiver<ank_proto::v1::TaskEvent>,
    agent_event_rx: &mut tokio::sync::broadcast::Receiver<ank_core::agents::event::AgentEvent>,
    tenant_id: &str,
    orchestrator: &AgentOrchestrator,
) -> String {
    let mut full_output = String::new();
    let mut stream = BroadcastStream::new(receiver);

    loop {
        let proto_event = tokio::select! {
            task_next = stream.next() => match task_next {
                Some(Ok(ev)) => ev,
                // None (stream closed) or Err (broadcast lag): end streaming,
                // same semantics as the old `while let Some(Ok(_))`.
                _ => break,
            },
            // CORE-FIX (C): keep forwarding AgentEvents (SupervisorQuestion, tree
            // updates, …) WHILE a chat turn streams. Before this, agent events were
            // only drained by the idle select! in handle_chat, so a supervisor that
            // asked mid-turn stayed invisible until the turn ended — and if the turn
            // died (429 / empty output), the question was never surfaced to the user.
            agent_ev = agent_event_rx.recv() => {
                match agent_ev {
                    Ok(event) => {
                        // CORE-FIX (security): filter by tenant — global broadcast.
                        if orchestrator
                            .agent_event_belongs_to_tenant(&event, tenant_id)
                            .await
                        {
                            if let Ok(data) = serde_json::to_value(&event) {
                                let frame = json!({ "event": "agent_event", "data": data });
                                let _ = socket.send(Message::Text(frame.to_string())).await;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(
                            "[ChatWS] tenant={} agent_event_rx lagged by {} during stream",
                            tenant_id, n
                        );
                    }
                    Err(_) => {}
                }
                continue;
            }
        };
        if let Some(ref payload) = proto_event.payload {
            if let ank_proto::v1::task_event::Payload::Output(ref text) = payload {
                // CORE-FIX (A2): meta-tokens emitted by the HAL — never reach the user.
                if text.starts_with("__TOOL_CALL__") {
                    continue;
                }
                if let Some(json_str) = text.strip_prefix("__MODEL_SELECTED__") {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let _ = socket
                            .send(Message::Text(
                                json!({ "event": "model_selected", "data": meta }).to_string(),
                            ))
                            .await;
                    }
                    continue;
                }
                // CORE-FIX (A3): VCM / build-time warnings forwarded as a non-blocking event.
                if let Some(json_str) = text.strip_prefix("__WARNING__") {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let _ = socket
                            .send(Message::Text(
                                json!({ "event": "warning", "data": meta }).to_string(),
                            ))
                            .await;
                    }
                    continue;
                }
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

/// Appends an agent event trace to the tenant's private agent traces log.
async fn append_to_agent_traces(
    tenant_id: &str,
    event: &ank_core::agents::event::AgentEvent,
) -> anyhow::Result<()> {
    let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace_path = std::path::Path::new(&base_dir)
        .join("users")
        .join(tenant_id)
        .join("workspace");

    let _ = tokio::fs::create_dir_all(&workspace_path).await;
    let log_path = workspace_path.join("agent_traces.log");

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await?;

    let timestamp = chrono::Utc::now().to_rfc3339();
    let text = match event {
        ank_core::agents::event::AgentEvent::Spawned {
            agent_id,
            role,
            model,
            task_type,
            ..
        } => {
            format!(
                "🤖 [SPAWNED] Agent '{}' with role '{}' on model '{}' for task '{}'",
                agent_id,
                role.display_name(),
                model,
                task_type
            )
        }
        ank_core::agents::event::AgentEvent::StateChanged { agent_id, state } => {
            format!(
                "⚙️ [STATE] Agent '{}' state changed to '{}'",
                agent_id, state
            )
        }
        ank_core::agents::event::AgentEvent::Activity {
            agent_id,
            description,
        } => {
            format!("🧠 [ACTIVITY] Agent '{}': {}", agent_id, description)
        }
        ank_core::agents::event::AgentEvent::Reported { agent_id, summary } => {
            format!("📋 [REPORT] Agent '{}' reported: {}", agent_id, summary)
        }
        ank_core::agents::event::AgentEvent::SupervisorQuestion {
            agent_id, question, ..
        } => {
            format!(
                "❓ [QUESTION] Supervisor Agent '{}' asked: {}",
                agent_id, question
            )
        }
        ank_core::agents::event::AgentEvent::SupervisorCompleted {
            agent_id, summary, ..
        } => {
            format!(
                "✅ [COMPLETED] Supervisor Agent '{}' completed: {}",
                agent_id, summary
            )
        }
        ank_core::agents::event::AgentEvent::SupervisorTimedOut { agent_id, .. } => {
            format!(
                "⏳ [TIMEOUT] Supervisor Agent '{}' timed out waiting for input",
                agent_id
            )
        }
        _ => return Ok(()),
    };

    let entry = format!("[{}] {}\n", timestamp, text);
    file.write_all(entry.as_bytes()).await?;
    Ok(())
}

/// CORE-FIX: extract the assistant's name out of a free-form reply during
/// the onboarding "awaiting_name" step. The user often writes a phrase like
/// "hola! te llamare Aegis" instead of just "Aegis", and the previous code
/// stored the whole phrase as the name.
///
/// Strategy (in order of precedence):
/// 1. Common pattern: "te llamare X", "te llamo X", "tu nombre es X",
///    "llamate X", "llamame X" → extract X (one or two tokens after).
/// 2. If the reply is a single token (1-30 chars, mostly letters) → that IS the name.
/// 3. If the reply is 2-3 short tokens looking like a name (each ≤ 20 chars,
///    alphabetic + accents) → join them.
/// 4. Otherwise → None (caller asks for clarification).
fn extract_name_from_phrase(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Pattern-match common "tell me your name" phrasings.
    let lower = trimmed.to_lowercase();
    let patterns = [
        "mi asistente se llamará ",
        "mi asistente se llamara ",
        "mi asistente se llama ",
        "me gustaría que te llames ",
        "me gustaria que te llames ",
        "quiero que te llames ",
        "quiero que seas ",
        "te puedes llamar ",
        "te podes llamar ",
        "puedes llamarte ",
        "podes llamarte ",
        "ponete de nombre ",
        "ponte de nombre ",
        "que te llames ",
        "se llamará ",
        "se llamara ",
        "se llama ",
        "te llamare ",
        "te llamaré ",
        "te llamo ",
        "tu nombre es ",
        "tu nombre será ",
        "tu nombre va a ser ",
        "llamate ",
        "llámate ",
        "llamame ",
        "llámame ",
        "te voy a llamar ",
        "vas a ser ",
        "te llamaras ",
        "te llamarás ",
        "me llamas ",
        "llamarte ",
        "ponete ",
        "ponte ",
        "serás ",
        "seras ",
        "your name is ",
        "call you ",
        "i'll call you ",
        "name is ",
    ];
    for pat in &patterns {
        if let Some(idx) = lower.find(pat) {
            // Take what's after the pattern, trim, and grab up to the first
            // sentence-ending punctuation (so we don't pull in "Aegis. Es muy lindo").
            let after = &trimmed[idx + pat.len()..];
            let candidate = after
                .split(['.', ',', '!', '?', '\n', ';', ':'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(|c: char| !c.is_alphanumeric());
            if let Some(name) = sanitise_name_token(candidate) {
                return Some(name);
            }
        }
    }

    // 2. Single token → that's the name.
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() == 1 {
        return sanitise_name_token(tokens[0]);
    }

    // 3. 2-3 short alphabetic tokens → join as "First Middle Last".
    if tokens.len() <= 3
        && tokens.iter().all(|t| {
            t.len() <= 20
                && t.chars()
                    .all(|c| c.is_alphabetic() || matches!(c, '\'' | '-' | '.'))
        })
    {
        return sanitise_name_token(trimmed);
    }

    None
}

/// Trims and lightly validates a candidate name. Returns the title-cased
/// version when it looks reasonable, None otherwise. Refuses obvious garbage
/// like punctuation-only strings or things longer than 30 chars.
fn sanitise_name_token(raw: &str) -> Option<String> {
    let clean: String = raw
        .trim()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string();
    if clean.is_empty() || clean.len() > 30 {
        return None;
    }
    // Must contain at least one alphabetic character.
    if !clean.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    // Title-case each whitespace-separated word.
    let titled: String = clean
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(|c| c.to_lowercase()))
                    .collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(titled)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_name_single_token() {
        assert_eq!(extract_name_from_phrase("Aegis"), Some("Aegis".to_string()));
        assert_eq!(
            extract_name_from_phrase("  aegis  "),
            Some("Aegis".to_string())
        );
        assert_eq!(extract_name_from_phrase("MARIA"), Some("Maria".to_string()));
    }

    #[test]
    fn extract_name_from_te_llamare_phrase() {
        // The exact failure from the smoke test: "hola! te llamare Aegis"
        // used to persist as the entire string. Now must extract "Aegis".
        assert_eq!(
            extract_name_from_phrase("hola! te llamare Aegis"),
            Some("Aegis".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("Te llamaré Lucía"),
            Some("Lucía".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("llamame Eve"),
            Some("Eve".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("tu nombre es Pepe"),
            Some("Pepe".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("quiero que te llames Aegis"),
            Some("Aegis".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("me gustaría que te llames Aegis"),
            Some("Aegis".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("mi asistente se llamará Aegis"),
            Some("Aegis".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("te podes llamar Aegis"),
            Some("Aegis".to_string())
        );
    }

    #[test]
    fn extract_name_strips_trailing_punctuation() {
        assert_eq!(
            extract_name_from_phrase("te llamo Aegis."),
            Some("Aegis".to_string())
        );
        assert_eq!(
            extract_name_from_phrase("Tu nombre es Lucía, espero que te guste"),
            Some("Lucía".to_string())
        );
    }

    #[test]
    fn extract_name_two_word_name() {
        assert_eq!(
            extract_name_from_phrase("María José"),
            Some("María José".to_string())
        );
    }

    #[test]
    fn extract_name_rejects_garbage() {
        // Long rambling input with no clear name pattern → caller asks again
        assert_eq!(extract_name_from_phrase("ehh no se decime vos"), None);
        assert_eq!(extract_name_from_phrase(""), None);
        assert_eq!(extract_name_from_phrase("..."), None);
        assert_eq!(extract_name_from_phrase("123"), None);
    }

    #[test]
    fn sanitise_name_drops_too_long() {
        // 31 chars → reject
        let long = "AbcdefghijklmnopqrstuvwxyzAbcde";
        assert_eq!(sanitise_name_token(long), None);
    }
}
