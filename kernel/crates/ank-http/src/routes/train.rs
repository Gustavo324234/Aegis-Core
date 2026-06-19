use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::trainer::{TrainingConfig, TrainingProgress};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
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
        .route("/progress", get(ws_progress_handler))
}

// ── POST /api/train/start ─────────────────────────────────────────────────────
async fn start_train(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(config): Json<TrainingConfig>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    info!(
        "Iniciando petición de entrenamiento para: {}",
        config.model_id
    );

    state
        .training_manager
        .start_training(config)
        .await
        .map_err(|e| AegisHttpError::BadRequest(e))?;

    Ok(Json(json!({
        "success": true,
        "message": "Entrenamiento iniciado en segundo plano"
    })))
}

// ── POST /api/train/cancel ────────────────────────────────────────────────────
async fn cancel_train(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    info!("Petición para cancelar el entrenamiento activo...");

    state
        .training_manager
        .cancel_training()
        .await
        .map_err(|e| AegisHttpError::BadRequest(e))?;

    Ok(Json(json!({
        "success": true,
        "message": "Entrenamiento cancelado correctamente"
    })))
}

// ── GET /api/train/status ─────────────────────────────────────────────────────
async fn get_train_status(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
) -> Result<Json<TrainingProgress>, AegisHttpError> {
    let progress = state.training_manager.get_progress().await;
    Ok(Json(progress))
}

// ── GET /api/train/progress (WebSocket) ──────────────────────────────────────────
async fn ws_progress_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_progress_stream(socket, state))
}

async fn handle_progress_stream(mut socket: WebSocket, state: AppState) {
    info!("[TrainWS] Cliente conectado al stream de progreso...");

    // Enviar estado inicial del progreso
    let initial = state.training_manager.get_progress().await;
    if let Ok(json) = serde_json::to_string(&initial) {
        if socket.send(Message::Text(json)).await.is_err() {
            return;
        }
    }

    // Suscribirse a los eventos del TrainingManager
    let mut rx = state.training_manager.subscribe();

    loop {
        tokio::select! {
            // Recibir métricas del canal de broadcast del manager y enviarlas a la UI
            progress_event = rx.recv() => {
                match progress_event {
                    Ok(progress) => {
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

    info!("[TrainWS] Cliente desconectado del stream de progreso.");
}
