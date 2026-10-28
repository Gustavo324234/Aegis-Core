use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket},
        DefaultBodyLimit, OriginalUri, Path, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Capacidad máxima de mensajes en cola por túnel activo (evita OOM ante clientes lentos).
const TUNNEL_QUEUE_CAPACITY: usize = 256;
/// Tamaño máximo permitido de cuerpo HTTP proxificado (16 MiB).
const MAX_TUNNEL_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Tamaño máximo permitido de frame binario WS entrante (18 MiB incluyendo cabeceras JSON).
const MAX_WS_FRAME_BYTES: usize = 18 * 1024 * 1024;

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

struct TunnelRegistration {
    connection_id: Uuid,
    token_fingerprint: String,
    tx: mpsc::Sender<Message>,
}

struct PendingRequest {
    connection_id: Uuid,
    tx: oneshot::Sender<TunnelFrame>,
}

struct RelayAuthConfig {
    hmac_secret: Option<String>,
    static_tokens: HashMap<String, String>,
    allow_dev_tokens: bool,
}

impl RelayAuthConfig {
    fn from_env() -> Self {
        let hmac_secret = std::env::var("AEGIS_RELAY_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let static_tokens = std::env::var("AEGIS_RELAY_TOKENS_JSON")
            .ok()
            .and_then(|raw| serde_json::from_str::<HashMap<String, String>>(&raw).ok())
            .unwrap_or_default();

        let allow_dev_tokens = std::env::var("AEGIS_RELAY_ALLOW_DEV_TOKENS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(cfg!(debug_assertions));

        Self {
            hmac_secret,
            static_tokens,
            allow_dev_tokens,
        }
    }
}

struct RelayState {
    tunnels: RwLock<HashMap<String, TunnelRegistration>>,
    pending_responses: RwLock<HashMap<Uuid, PendingRequest>>,
    /// En modo dev, fija la primera huella de token vista por tenant para impedir hijacking cruzado.
    claimed_dev_tenants: RwLock<HashMap<String, String>>,
    auth: RelayAuthConfig,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn token_fingerprint_hex(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_tenant_slug(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn compute_expected_hmac_hex(secret: &str, tenant_id: &str) -> Option<String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(format!("aegis-relay:{}", tenant_id).as_bytes());
    Some(hex::encode(mac.finalize().into_bytes()))
}

fn verify_token_against_config(auth: &RelayAuthConfig, tenant_id: &str, token: &str) -> bool {
    if !token.starts_with("orion_id_tok_live_aegis_") || token.len() < 30 {
        return false;
    }

    if !auth.static_tokens.is_empty() {
        if let Some(expected) = auth.static_tokens.get(tenant_id) {
            return constant_time_eq(expected.as_bytes(), token.as_bytes());
        }
        if auth.hmac_secret.is_none() {
            return false;
        }
    }

    if let Some(ref secret) = auth.hmac_secret {
        let Some(expected_mac) = compute_expected_hmac_hex(secret, tenant_id) else {
            return false;
        };
        let suffix = token.rsplit('_').next().unwrap_or("");
        return constant_time_eq(expected_mac.as_bytes(), suffix.as_bytes());
    }

    auth.allow_dev_tokens
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Failed to set global subscriber: {}", e);
    }

    let auth = RelayAuthConfig::from_env();
    if auth.hmac_secret.is_none() && auth.static_tokens.is_empty() {
        if auth.allow_dev_tokens {
            warn!(
                "SEC-WARN: AEGIS_RELAY_SECRET / AEGIS_RELAY_TOKENS_JSON not set. \
                 Running in dev-pinned token mode (first token claims tenant slug)."
            );
        } else {
            warn!(
                "SEC-ALERT: AEGIS_RELAY_SECRET and AEGIS_RELAY_TOKENS_JSON are unset in release mode — \
                 all tunnel handshakes will be rejected until configured."
            );
        }
    }

    let state = Arc::new(RelayState {
        tunnels: RwLock::new(HashMap::new()),
        pending_responses: RwLock::new(HashMap::new()),
        claimed_dev_tenants: RwLock::new(HashMap::new()),
        auth,
    });

    let app = build_router(state);

    let port = std::env::var("AEGIS_RELAY_PORT").unwrap_or_else(|_| "8083".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Aegis Connect Relay starting on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(state: Arc<RelayState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ws/connect", get(handle_tunnel_websocket))
        .route("/u/:tenant", any(proxy_request))
        .route("/u/:tenant/*path", any(proxy_request))
        .layer(DefaultBodyLimit::max(MAX_TUNNEL_BODY_BYTES))
        .with_state(state)
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// WebSocket Tunnel Connection endpoint (Local Aegis connects here)
async fn handle_tunnel_websocket(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
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

    let Some(tenant_raw) = tenant else {
        warn!("Rejecting tunnel: missing tenant name header");
        return (StatusCode::BAD_REQUEST, "x-citadel-tenant header required").into_response();
    };

    let tenant_id = normalize_tenant_slug(&tenant_raw);
    if tenant_id.is_empty() || tenant_id.len() > 64 {
        return (StatusCode::BAD_REQUEST, "Invalid tenant slug").into_response();
    }

    let Some(raw_token) = token else {
        warn!(
            "Rejecting tunnel for '{}': missing Orion ID Token",
            tenant_id
        );
        return (
            StatusCode::UNAUTHORIZED,
            "Unauthorized Orion ID Token required",
        )
            .into_response();
    };

    if !verify_token_against_config(&state.auth, &tenant_id, raw_token) {
        warn!(
            "Rejecting tunnel for '{}': invalid or unauthorized Orion ID Token",
            tenant_id
        );
        return (
            StatusCode::UNAUTHORIZED,
            "Unauthorized Orion ID Token required",
        )
            .into_response();
    }

    let fingerprint = token_fingerprint_hex(raw_token);

    // Pin token fingerprint per tenant in dev mode and block cross-token hijacking of active tunnels
    {
        if state.auth.hmac_secret.is_none() && state.auth.static_tokens.is_empty() {
            let mut claimed = state.claimed_dev_tenants.write().await;
            match claimed.get(&tenant_id) {
                Some(existing_fp)
                    if !constant_time_eq(existing_fp.as_bytes(), fingerprint.as_bytes()) =>
                {
                    warn!(
                        "Rejecting tunnel hijack attempt on dev-pinned tenant '{}'",
                        tenant_id
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        "Tenant slug already claimed by a different token",
                    )
                        .into_response();
                }
                None => {
                    claimed.insert(tenant_id.clone(), fingerprint.clone());
                }
                _ => {}
            }
        }

        let tunnels = state.tunnels.read().await;
        if let Some(active) = tunnels.get(&tenant_id) {
            if !constant_time_eq(active.token_fingerprint.as_bytes(), fingerprint.as_bytes()) {
                warn!(
                    "Rejecting conflicting token for active tunnel tenant '{}'",
                    tenant_id
                );
                return (
                    StatusCode::FORBIDDEN,
                    "Active tunnel belongs to a different token",
                )
                    .into_response();
            }
        }
    }

    info!(
        "Orion ID validated successfully for tunnel tenant '{}'",
        tenant_id
    );

    ws.max_message_size(MAX_WS_FRAME_BYTES)
        .on_upgrade(move |socket| register_tunnel(socket, tenant_id, fingerprint, state))
}

// Registers the WebSocket client into active tunnels map and routes traffic
async fn register_tunnel(
    socket: WebSocket,
    tenant: String,
    token_fingerprint: String,
    state: Arc<RelayState>,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Message>(TUNNEL_QUEUE_CAPACITY);
    let connection_id = Uuid::new_v4();

    {
        let mut tunnels = state.tunnels.write().await;
        tunnels.insert(
            tenant.clone(),
            TunnelRegistration {
                connection_id,
                token_fingerprint,
                tx: tx.clone(),
            },
        );
        info!(
            "Aegis Connect: Tunnel established for tenant '{}' (conn={})",
            tenant, connection_id
        );
    }

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

    let relay_state = state.clone();
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(Message::Binary(bytes)) => {
                if bytes.len() > MAX_WS_FRAME_BYTES {
                    warn!("Tunnel frame exceeded MAX_WS_FRAME_BYTES; closing.");
                    break;
                }
                if let Ok(frame) = serde_json::from_slice::<TunnelFrame>(&bytes) {
                    if let TunnelFrame::Response { id, .. } = &frame {
                        let mut pending = relay_state.pending_responses.write().await;
                        if let Some(req) = pending.remove(id) {
                            let _ = req.tx.send(frame);
                        }
                    }
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = tx.try_send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) | Err(_) => {
                break;
            }
            _ => {}
        }
    }

    // CORE-366: Only unregister if `connection_id` still matches (prevents stale socket
    // disconnect from evicting a newer reconnected tunnel for the same tenant).
    {
        let mut tunnels = state.tunnels.write().await;
        let should_remove = tunnels
            .get(&tenant)
            .map(|reg| reg.connection_id == connection_id)
            .unwrap_or(false);
        if should_remove {
            tunnels.remove(&tenant);
            info!(
                "Aegis Connect: Tunnel disconnected and unregistered for tenant '{}' (conn={})",
                tenant, connection_id
            );
        }
    }

    // Fail-fast all in-flight browser requests tied to this closed connection_id
    {
        let mut pending = state.pending_responses.write().await;
        pending.retain(|_, waiter| waiter.connection_id != connection_id);
    }

    send_task.abort();
}

// Proxies incoming public browser HTTP request down the WebSocket tunnel
async fn proxy_request(
    OriginalUri(original_uri): OriginalUri,
    Path(params): Path<HashMap<String, String>>,
    method: Method,
    headers: HeaderMap,
    State(state): State<Arc<RelayState>>,
    body: Bytes,
) -> Response {
    if body.len() > MAX_TUNNEL_BODY_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Request body exceeds 16 MiB").into_response();
    }

    let tenant = match params.get("tenant") {
        Some(t) => normalize_tenant_slug(t),
        None => return (StatusCode::BAD_REQUEST, "Missing tenant path parameter").into_response(),
    };

    let (connection_id, ws_sender) = {
        let tunnels = state.tunnels.read().await;
        match tunnels.get(&tenant) {
            Some(reg) => (reg.connection_id, reg.tx.clone()),
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
        if !is_hop_by_hop_header(k.as_str()) {
            if let Ok(val) = v.to_str() {
                headers_map.insert(k.to_string(), val.to_string());
            }
        }
    }

    let base_path = params
        .get("path")
        .map(|p| format!("/{}", p.trim_start_matches('/')))
        .unwrap_or_else(|| "/".to_string());
    let path = match original_uri.query() {
        Some(q) if !q.is_empty() => format!("{}?{}", base_path, q),
        _ => base_path,
    };

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

    let (tx, rx) = oneshot::channel::<TunnelFrame>();
    {
        let mut pending = state.pending_responses.write().await;
        pending.insert(request_id, PendingRequest { connection_id, tx });
    }

    if tokio::time::timeout(
        Duration::from_secs(5),
        ws_sender.send(Message::Binary(frame_bytes)),
    )
    .await
    .map(|r| r.is_err())
    .unwrap_or(true)
    {
        let mut pending = state.pending_responses.write().await;
        pending.remove(&request_id);
        return (
            StatusCode::BAD_GATEWAY,
            "Tunnel connection congested or lost",
        )
            .into_response();
    }

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(TunnelFrame::Response {
            status,
            headers,
            body,
            ..
        })) => {
            let mut response_builder = Response::builder().status(status);
            for (k, v) in headers {
                if !is_hop_by_hop_header(&k) {
                    response_builder = response_builder.header(k, v);
                }
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
        Ok(Ok(_)) => (
            StatusCode::BAD_GATEWAY,
            "Unexpected tunnel frame received from Aegis instance",
        )
            .into_response(),
        Ok(Err(_)) => (
            StatusCode::BAD_GATEWAY,
            "Aegis instance disconnected while processing request",
        )
            .into_response(),
        Err(_) => {
            let mut pending = state.pending_responses.write().await;
            pending.remove(&request_id);
            (StatusCode::GATEWAY_TIMEOUT, "Aegis instance timed out").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_token_verification_and_tenant_binding() -> anyhow::Result<()> {
        let secret = "top_secret_relay_master_key_2026";
        let auth = RelayAuthConfig {
            hmac_secret: Some(secret.to_string()),
            static_tokens: HashMap::new(),
            allow_dev_tokens: false,
        };

        let mac_alice = compute_expected_hmac_hex(secret, "alice")
            .ok_or_else(|| anyhow::anyhow!("hmac failed"))?;
        let valid_alice_token = format!("orion_id_tok_live_aegis_alice_{}", mac_alice);

        assert!(verify_token_against_config(
            &auth,
            "alice",
            &valid_alice_token
        ));
        // Token valid for alice MUST NOT authenticate for bob!
        assert!(!verify_token_against_config(
            &auth,
            "bob",
            &valid_alice_token
        ));
        // Forged prefix-only token MUST be rejected!
        assert!(!verify_token_against_config(
            &auth,
            "alice",
            "orion_id_tok_live_aegis_forged_1234567890"
        ));
        Ok(())
    }

    #[test]
    fn test_normalize_tenant_slug_strips_dangerous_chars() {
        assert_eq!(normalize_tenant_slug(" ../Root_User! "), "root-user");
    }
}
