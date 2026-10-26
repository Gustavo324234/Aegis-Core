use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{handshake::client::Request, Message},
};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Concurrencia máxima de peticiones HTTP locales simultáneas despachadas por el túnel.
const MAX_CONCURRENT_LOCAL_REQUESTS: usize = 64;
/// Capacidad de la cola de salida WS del cliente de túnel.
const CLIENT_WS_QUEUE_CAPACITY: usize = 256;
/// Tamaño máximo de respuesta local proxificada de vuelta al relay (16 MiB).
const MAX_LOCAL_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

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

/// Construye la URI WebSocket (`ws://` o `wss://`) y el header `Host` preservando TLS
/// y sin exponer el token secreto en el query string.
pub(crate) fn build_ws_handshake_target(
    relay_url: &str,
    tenant: &str,
) -> Result<(String, String), String> {
    let parsed = reqwest::Url::parse(relay_url).map_err(|e| format!("Invalid relay URL: {}", e))?;

    let ws_scheme = match parsed.scheme() {
        "wss" | "https" => "wss",
        "ws" | "http" => "ws",
        other => return Err(format!("Unsupported relay URL scheme '{}'", other)),
    };

    let host = parsed
        .host_str()
        .ok_or_else(|| "Missing host in relay URL".to_string())?;

    let host_header = match parsed.port() {
        Some(port) => format!("{}:{}", host, port),
        None => host.to_string(),
    };

    let path = if parsed.path().is_empty() {
        "/ws/connect"
    } else {
        parsed.path()
    };

    let mut query_pairs = Vec::new();
    if let Some(existing_q) = parsed.query() {
        if !existing_q.is_empty() {
            query_pairs.push(existing_q.to_string());
        }
    }
    query_pairs.push(format!("tenant={}", urlencoding::encode(tenant)));

    let uri_str = format!(
        "{}://{}{}?{}",
        ws_scheme,
        host_header,
        path,
        query_pairs.join("&")
    );
    Ok((uri_str, host_header))
}

/// Valida y resuelve `path` contra `local_url`, garantizando que jamás escape hacia otro host/origen.
pub(crate) fn resolve_safe_local_url(
    local_base_url: &str,
    rel_path: &str,
) -> Result<reqwest::Url, String> {
    if !rel_path.starts_with('/') || rel_path.starts_with("//") || rel_path.contains('\0') {
        return Err(format!(
            "Rejected unsafe tunnel request path '{}'",
            rel_path
        ));
    }

    let base = reqwest::Url::parse(local_base_url)
        .map_err(|e| format!("Invalid local_url '{}': {}", local_base_url, e))?;
    let joined = base
        .join(rel_path)
        .map_err(|e| format!("Invalid request path '{}': {}", rel_path, e))?;

    if joined.scheme() != base.scheme()
        || joined.host_str() != base.host_str()
        || joined.port_or_known_default() != base.port_or_known_default()
    {
        return Err("Blocked cross-origin local proxy attempt".to_string());
    }

    Ok(joined)
}

pub struct TunnelClient {
    relay_url: String,
    tenant: String,
    token: String,
    local_url: String,
}

impl TunnelClient {
    pub fn new(relay_url: String, tenant: String, token: String, local_url: String) -> Self {
        Self {
            relay_url,
            tenant,
            token,
            local_url,
        }
    }

    pub async fn run(&self) {
        info!(
            "Starting Aegis Connect TunnelClient for tenant '{}' pointing to Relay '{}'",
            self.tenant, self.relay_url
        );

        let local_client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(28))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to build local proxy reqwest client: {}", e);
                return;
            }
        };

        let concurrency_limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_LOCAL_REQUESTS));
        let mut backoff = Duration::from_secs(2);
        let max_backoff = Duration::from_secs(30);

        loop {
            let (uri_str, host_header) =
                match build_ws_handshake_target(&self.relay_url, &self.tenant) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Invalid relay configuration '{}': {}", self.relay_url, e);
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        continue;
                    }
                };

            let request_builder = Request::builder()
                .uri(&uri_str)
                .header("Host", &host_header)
                .header("Connection", "Upgrade")
                .header("Upgrade", "websocket")
                .header("Sec-WebSocket-Version", "13")
                .header(
                    "Sec-WebSocket-Key",
                    tokio_tungstenite::tungstenite::handshake::client::generate_key(),
                )
                .header("x-citadel-tenant", &self.tenant)
                .header("Authorization", format!("Bearer {}", self.token));

            let request = match request_builder.body(()) {
                Ok(req) => req,
                Err(e) => {
                    error!("Failed to build WebSocket handshake request: {}", e);
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                    continue;
                }
            };

            info!(
                "Connecting to Aegis Connect Relay WebSocket ({})...",
                uri_str
            );
            match connect_async(request).await {
                Ok((ws_stream, _)) => {
                    info!("Successfully connected to Relay WebSocket tunnel!");
                    backoff = Duration::from_secs(2);

                    let (mut ws_tx, mut ws_rx) = ws_stream.split();
                    let (tx, mut rx) = mpsc::channel::<Message>(CLIENT_WS_QUEUE_CAPACITY);

                    let writer_task = tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            if ws_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                    });

                    let ping_tx = tx.clone();
                    let heartbeat_task = tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(20)).await;
                            if ping_tx.send(Message::Ping(vec![])).await.is_err() {
                                break;
                            }
                        }
                    });

                    while let Some(msg_result) = ws_rx.next().await {
                        match msg_result {
                            Ok(Message::Binary(bytes)) => {
                                if let Ok(TunnelFrame::Request {
                                    id,
                                    method,
                                    path,
                                    headers,
                                    body,
                                }) = serde_json::from_slice::<TunnelFrame>(&bytes)
                                {
                                    let tx_clone = tx.clone();
                                    let client_clone = local_client.clone();
                                    let local_url_clone = self.local_url.clone();
                                    let sem_clone = Arc::clone(&concurrency_limiter);

                                    tokio::spawn(async move {
                                        let Ok(_permit) = sem_clone.acquire_owned().await else {
                                            return;
                                        };

                                        let target_url =
                                            match resolve_safe_local_url(&local_url_clone, &path) {
                                                Ok(u) => u,
                                                Err(reason) => {
                                                    warn!(
                                                        "TunnelClient blocked request: {}",
                                                        reason
                                                    );
                                                    let denied = TunnelFrame::Response {
                                                        id,
                                                        status: 400,
                                                        headers: HashMap::new(),
                                                        body: Some(reason.into_bytes()),
                                                    };
                                                    if let Ok(b) = serde_json::to_vec(&denied) {
                                                        let _ =
                                                            tx_clone.send(Message::Binary(b)).await;
                                                    }
                                                    return;
                                                }
                                            };

                                        let mut req_builder = client_clone.request(
                                            reqwest::Method::from_bytes(method.as_bytes())
                                                .unwrap_or(reqwest::Method::GET),
                                            target_url.clone(),
                                        );

                                        for (k, v) in headers.iter() {
                                            let kl = k.to_ascii_lowercase();
                                            if kl != "host"
                                                && kl != "connection"
                                                && kl != "content-length"
                                                && kl != "transfer-encoding"
                                            {
                                                req_builder = req_builder.header(k, v);
                                            }
                                        }

                                        if let Some(body_bytes) = body {
                                            req_builder = req_builder.body(body_bytes);
                                        }

                                        let response_frame = match req_builder.send().await {
                                            Ok(resp) => {
                                                let status = resp.status().as_u16();
                                                let mut resp_headers = HashMap::new();
                                                for (k, v) in resp.headers().iter() {
                                                    if let Ok(val) = v.to_str() {
                                                        resp_headers
                                                            .insert(k.to_string(), val.to_string());
                                                    }
                                                }
                                                let mut buf = Vec::new();
                                                let mut stream = resp.bytes_stream();
                                                let mut overflowed = false;
                                                while let Some(chunk_res) = stream.next().await {
                                                    match chunk_res {
                                                        Ok(chunk) => {
                                                            if buf.len().saturating_add(chunk.len())
                                                                > MAX_LOCAL_RESPONSE_BYTES
                                                            {
                                                                overflowed = true;
                                                                break;
                                                            }
                                                            buf.extend_from_slice(&chunk);
                                                        }
                                                        Err(_) => break,
                                                    }
                                                }

                                                if overflowed {
                                                    TunnelFrame::Response {
                                                        id,
                                                        status: 502,
                                                        headers: HashMap::new(),
                                                        body: Some(
                                                            b"Local response exceeded 16 MiB tunnel limit"
                                                                .to_vec(),
                                                        ),
                                                    }
                                                } else {
                                                    TunnelFrame::Response {
                                                        id,
                                                        status,
                                                        headers: resp_headers,
                                                        body: Some(buf),
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "Failed proxying request locally to {}: {}",
                                                    target_url, e
                                                );
                                                TunnelFrame::Response {
                                                    id,
                                                    status: 502,
                                                    headers: HashMap::new(),
                                                    body: Some(
                                                        format!("Local proxy error: {}", e)
                                                            .into_bytes(),
                                                    ),
                                                }
                                            }
                                        };

                                        if let Ok(resp_bytes) = serde_json::to_vec(&response_frame)
                                        {
                                            let _ =
                                                tx_clone.send(Message::Binary(resp_bytes)).await;
                                        }
                                    });
                                }
                            }
                            Ok(Message::Ping(payload)) => {
                                let _ = tx.try_send(Message::Pong(payload));
                            }
                            Ok(Message::Close(_)) => {
                                warn!("WebSocket connection closed by relay server.");
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket read error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    writer_task.abort();
                    heartbeat_task.abort();
                }
                Err(e) => {
                    error!(
                        "WebSocket connection error: {}. Retrying in {:?}...",
                        e, backoff
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ws_handshake_preserves_wss_and_hides_token() -> Result<(), String> {
        let (uri, host) =
            build_ws_handshake_target("wss://connect.aegis-os.com/ws/connect", "tavo")?;
        assert_eq!(uri, "wss://connect.aegis-os.com/ws/connect?tenant=tavo");
        assert_eq!(host, "connect.aegis-os.com");
        assert!(!uri.contains("token="));

        let (uri_local, host_local) =
            build_ws_handshake_target("http://127.0.0.1:8083/ws/connect", "root")?;
        assert_eq!(uri_local, "ws://127.0.0.1:8083/ws/connect?tenant=root");
        assert_eq!(host_local, "127.0.0.1:8083");
        Ok(())
    }

    #[test]
    fn test_resolve_safe_local_url_blocks_origin_escape() -> Result<(), String> {
        let ok = resolve_safe_local_url("http://127.0.0.1:8000", "/api/status?full=1")?;
        assert_eq!(ok.as_str(), "http://127.0.0.1:8000/api/status?full=1");

        // Protocol-relative open redirect / SSRF attempt
        assert!(
            resolve_safe_local_url("http://127.0.0.1:8000", "//169.254.169.254/latest").is_err()
        );
        // Full URL override attempt
        assert!(
            resolve_safe_local_url("http://127.0.0.1:8000", "http://evil.example/steal").is_err()
        );
        Ok(())
    }
}
