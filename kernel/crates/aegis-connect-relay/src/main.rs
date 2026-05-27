use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// Shared Tunnel Frame Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelFrame {
    Request {
        id: Uuid,
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    },
    Response {
        id: Uuid,
        status: u16,
        headers: HashMap<String, String>,
        body: Option<Vec<u8>>,
    },
}

// Struct to track pending browser requests waiting for responses from the tunnel
struct PendingRequest {
    tx: oneshot::Sender<TunnelFrame>,
}

// Global server state
struct RelayState {
    // Maps: tenant_slug -> sender channel to the WebSocket tunnel client
    tunnels: RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>,
    // Maps: request_id -> oneshot channel to return responses back to the browser threads
    pending_responses: RwLock<HashMap<Uuid, PendingRequest>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Failed to set global subscriber: {}", e);
    }

    let state = Arc::new(RelayState {
        tunnels: RwLock::new(HashMap::new()),
        pending_responses: RwLock::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/ws/connect", get(handle_tunnel_websocket))
        .route("/u/:tenant", any(proxy_request))
        .route("/u/:tenant/*path", any(proxy_request))
        .with_state(state);

    let port = std::env::var("AEGIS_RELAY_PORT").unwrap_or_else(|_| "8083".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Aegis Connect Relay starting on http://{}", addr);
    axum::serve(listener, app).await?;
    
    Ok(())
}

// WebSocket Tunnel Connection endpoint (Local Aegis connects here)
async fn handle_tunnel_websocket(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    // Token extraction and validation
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .or_else(|| params.get("token").map(|s| s.as_str()));

    let tenant = headers
        .get("x-citadel-tenant")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| params.get("tenant").cloned());

    let _token = match token {
        Some(t) if t.starts_with("orion_id_tok_live_aegis_") && t.len() >= 30 => t,
        _ => {
            warn!("Rejecting tunnel: missing or invalid Orion ID Token");
            return (
                StatusCode::UNAUTHORIZED,
                "Unauthorized Orion ID Token required",
            )
                .into_response();
        }
    };

    let tenant_id = match tenant {
        Some(t) if !t.is_empty() => t
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>(),
        _ => {
            warn!("Rejecting tunnel: missing tenant name header");
            return (StatusCode::BAD_REQUEST, "x-citadel-tenant header required").into_response();
        }
    };

    info!(
        "Orion ID validated successfully for tunnel tenant '{}' (Token prefix matches)",
        tenant_id
    );

    ws.on_upgrade(move |socket| register_tunnel(socket, tenant_id, state))
}

// Registers the WebSocket client into active tunnels map and routes traffic
async fn register_tunnel(socket: WebSocket, tenant: String, state: Arc<RelayState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Register active tunnel
    {
        let mut tunnels = state.tunnels.write().await;
        tunnels.insert(tenant.clone(), tx.clone());
        info!(
            "Aegis Connect: Tunnel established and registered for tenant '{}'",
            tenant
        );
    }

    // Task to receive frames from the queue and send them down the WebSocket
    let tenant_name = tenant.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
        info!(
            "Aegis Connect: Tunnel sender loop closed for tenant '{}'",
            tenant_name
        );
    });

    // Main read loop from WebSocket
    let relay_state = state.clone();
    let _tenant_slug = tenant.clone();
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(Message::Binary(bytes)) => {
                if let Ok(frame) = serde_json::from_slice::<TunnelFrame>(&bytes) {
                    if let TunnelFrame::Response { id, .. } = &frame {
                        // Dispatch response back to the pending HTTP request thread
                        let mut pending = relay_state.pending_responses.write().await;
                        if let Some(req) = pending.remove(id) {
                            let _ = req.tx.send(frame);
                        }
                    }
                }
            }
            Ok(Message::Close(_)) | Err(_) => {
                break;
            }
            _ => {}
        }
    }

    // Clean up tunnel registration
    {
        let mut tunnels = state.tunnels.write().await;
        if tunnels.remove(&tenant).is_some() {
            info!(
                "Aegis Connect: Tunnel disconnected and unregistered for tenant '{}'",
                tenant
            );
        }
    }
    send_task.abort();
}

// Proxies incoming public browser HTTP request down the WebSocket tunnel
async fn proxy_request(
    Path(params): Path<HashMap<String, String>>,
    method: Method,
    headers: HeaderMap,
    State(state): State<Arc<RelayState>>,
    body: Bytes,
) -> Response {
    let tenant = match params.get("tenant") {
        Some(t) => t.to_lowercase(),
        None => return (StatusCode::BAD_REQUEST, "Missing tenant path parameter").into_response(),
    };

    // Find active WebSocket tunnel for this tenant
    let ws_sender = {
        let tunnels = state.tunnels.read().await;
        match tunnels.get(&tenant) {
            Some(sender) => sender.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    format!(
                        "Aegis instance for tenant '{}' is offline (Tunnel not found)",
                        tenant
                    ),
                )
                    .into_response()
            }
        }
    };

    let request_id = Uuid::new_v4();
    let mut headers_map = HashMap::new();
    for (k, v) in headers.iter() {
        if let Ok(val) = v.to_str() {
            headers_map.insert(k.to_string(), val.to_string());
        }
    }

    // Extract path after "/u/:tenant"
    let path = params
        .get("path")
        .map(|p| format!("/{}", p))
        .unwrap_or_else(|| "/".to_string());

    let frame = TunnelFrame::Request {
        id: request_id,
        method: method.to_string(),
        path,
        headers: headers_map,
        body: if body.is_empty() {
            None
        } else {
            Some(body.to_vec())
        },
    };

    // Serialize frame
    let frame_bytes = match serde_json::to_vec(&frame) {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to serialize request",
            )
                .into_response()
        }
    };

    // Register oneshot response waiter
    let (tx, rx) = oneshot::channel::<TunnelFrame>();
    {
        let mut pending = state.pending_responses.write().await;
        pending.insert(request_id, PendingRequest { tx });
    }

    // Send request frame over WebSocket
    if ws_sender.send(Message::Binary(frame_bytes)).is_err() {
        let mut pending = state.pending_responses.write().await;
        pending.remove(&request_id);
        return (StatusCode::BAD_GATEWAY, "Tunnel connection lost").into_response();
    }

    // Wait for response frame with 30s timeout
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(TunnelFrame::Response {
            status,
            headers,
            body,
            ..
        })) => {
            let mut response_builder = Response::builder().status(status);
            for (k, v) in headers {
                response_builder = response_builder.header(k, v);
            }
            let body_bytes = body.unwrap_or_default();
            match response_builder.body(axum::body::Body::from(body_bytes)) {
                Ok(res) => res,
                Err(e) => {
                    warn!("Failed to construct proxy response: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(axum::body::Body::from("Internal Server Error"))
                        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()))
                }
            }
        }
        _ => {
            // Clean up waiter
            let mut pending = state.pending_responses.write().await;
            pending.remove(&request_id);
            (StatusCode::GATEWAY_TIMEOUT, "Aegis instance timed out").into_response()
        }
    }
}
