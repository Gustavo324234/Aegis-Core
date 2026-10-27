use crate::{
    citadel::{
        extract_ws_session_key_and_protocol, verify_ws_tenant, CitadelAuthenticated, CitadelError,
    },
    error::AegisHttpError,
    state::AppState,
};
use ank_core::trainer::{TrainingConfig, TrainingProgress};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{info, warn};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/start", post(start_train))
        .route("/cancel", post(cancel_train))
        .route("/status", get(get_train_status))
        .route("/progress", get(ws_progress_legacy_handler))
        .route("/progress/:tenant_id", get(ws_progress_handler))
}

// ── POST /api/train/start ─────────────────────────────────────────────────────
async fn start_train(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(mut config): Json<TrainingConfig>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    config.tenant_id = Some(auth.tenant_id.clone());
    info!(
        tenant_id = %auth.tenant_id,
        model_id = %config.model_id,
        "Iniciando petición de entrenamiento aislada por tenant"
    );

    state
        .training_manager
        .start_training(config)
        .await
        .map_err(AegisHttpError::BadRequest)?;

    Ok(Json(json!({
        "success": true,
        "message": "Entrenamiento iniciado en segundo plano"
    })))
}

// ── POST /api/train/cancel ────────────────────────────────────────────────────
async fn cancel_train(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    if !state
        .training_manager
        .owner_matches(&auth.tenant_id, auth.is_master)
        .await
    {
        return Err(AegisHttpError::Forbidden(
            "No tienes permisos para cancelar el entrenamiento de otro tenant".to_string(),
        ));
    }

    info!(
        tenant_id = %auth.tenant_id,
        "Petición para cancelar el entrenamiento activo..."
    );

    state
        .training_manager
        .cancel_training()
        .await
        .map_err(AegisHttpError::BadRequest)?;

    Ok(Json(json!({
        "success": true,
        "message": "Entrenamiento cancelado correctamente"
    })))
}

// ── GET /api/train/status ─────────────────────────────────────────────────────
async fn get_train_status(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<TrainingProgress>, AegisHttpError> {
    let progress = state
        .training_manager
        .get_progress_for_tenant(&auth.tenant_id, auth.is_master)
        .await;
    Ok(Json(progress))
}

// ── GET /api/train/progress (legacy header-authenticated WS) ──────────────────
async fn ws_progress_legacy_handler(
    ws: WebSocketUpgrade,
    auth: CitadelAuthenticated,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| {
        handle_progress_stream(socket, state, auth.tenant_id, auth.is_master)
    })
}

// ── GET /api/train/progress/:tenant_id (subprotocol-authenticated WS) ─────────
async fn ws_progress_handler(
    ws: WebSocketUpgrade,
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (Some(session_key), protocol_header) = extract_ws_session_key_and_protocol(&headers) else {
        warn!(
            tenant = %tenant_id,
            "[TrainWS] Rejected unauthenticated WebSocket upgrade"
        );
        return CitadelError::MissingKey.into_response();
    };

    let is_master = match verify_ws_tenant(&state, &tenant_id, &session_key).await {
        Ok(master) => master,
        Err(err) => {
            warn!(
                tenant = %tenant_id,
                error = %err,
                "[TrainWS] Rejected unauthorized WebSocket upgrade"
            );
            return err.into_response();
        }
    };

    let ws = if let Some(proto) = protocol_header {
        ws.protocols([proto])
    } else {
        ws
    };

    ws.on_upgrade(move |socket| handle_progress_stream(socket, state, tenant_id, is_master))
}

async fn handle_progress_stream(
    mut socket: WebSocket,
    state: AppState,
    tenant_id: String,
    is_master: bool,
) {
    info!(
        tenant = %tenant_id,
        "[TrainWS] Cliente conectado al stream de progreso..."
    );

    // Enviar estado inicial del progreso filtrado por tenant (CORE-368)
    let initial = state
        .training_manager
        .get_progress_for_tenant(&tenant_id, is_master)
        .await;
    if let Ok(json) = serde_json::to_string(&initial) {
        if socket.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Suscribirse a los eventos del TrainingManager
    let mut rx = state.training_manager.subscribe();

    loop {
        tokio::select! {
            // Recibir métricas del canal de broadcast del manager y enviarlas a la UI si pertenecen al tenant
            progress_event = rx.recv() => {
                match progress_event {
                    Ok(progress) => {
                        if !state.training_manager.owner_matches(&tenant_id, is_master).await {
                            continue;
                        }
                        match serde_json::to_string(&progress) {
                            Ok(json) => {
                                if socket.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => warn!("[TrainWS] Error de serialización del progreso: {}", e),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[TrainWS] Cliente de progreso retrasado por {} mensajes.", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Escuchar desconexiones de socket
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

    info!(
        tenant = %tenant_id,
        "[TrainWS] Cliente desconectado del stream de progreso."
    );
}
