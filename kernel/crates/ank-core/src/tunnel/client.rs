use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{handshake::client::Request, Message},
};
use tracing::{error, info, warn};
use uuid::Uuid;

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

        let local_client = reqwest::Client::new();

        loop {
            // Build the connect request with headers
            let connect_url = match reqwest::Url::parse(&self.relay_url) {
                Ok(url) => url,
                Err(e) => {
                    error!("Invalid relay URL '{}': {}", self.relay_url, e);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };

            let host = match connect_url.host_str() {
                Some(h) => h,
                None => {
                    error!("Invalid host in relay URL '{}'", self.relay_url);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };

            let port_str = match connect_url.port() {
                Some(p) => format!(":{}", p),
                None => "".to_string(),
            };

            let path_and_query = format!(
                "{}?tenant={}&token={}",
                connect_url.path(),
                urlencoding::encode(&self.tenant),
                urlencoding::encode(&self.token)
            );

            let uri_str = format!("ws://{}{}{}", host, port_str, path_and_query);

            let request_builder = Request::builder()
                .uri(uri_str)
                .header("Host", host)
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
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            info!("Connecting to Aegis Connect Relay WebSocket...");
            match connect_async(request).await {
                Ok((ws_stream, _)) => {
                    info!("Successfully connected to Relay WebSocket tunnel!");
                    let (mut ws_tx, mut ws_rx) = ws_stream.split();
                    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                    // Writer task to forward outgoing channel messages to WebSocket
                    let writer_task = tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            if ws_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                    });

                    // Heartbeat/Ping loop
                    let ping_tx = tx.clone();
                    let heartbeat_task = tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(20)).await;
                            if ping_tx.send(Message::Ping(vec![])).is_err() {
                                break;
                            }
                        }
                    });

                    // Read loop
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

                                    // Spawn HTTP dispatch task so we don't block the WebSocket reading loop
                                    tokio::spawn(async move {
                                        let local_req_url = format!(
                                            "{}{}",
                                            local_url_clone.trim_end_matches('/'),
                                            path
                                        );
                                        let mut req_builder = client_clone.request(
                                            reqwest::Method::from_bytes(method.as_bytes())
                                                .unwrap_or(reqwest::Method::GET),
                                            &local_req_url,
                                        );

                                        // Forward headers (excluding Host/Connection headers to prevent conflicts)
                                        for (k, v) in headers.iter() {
                                            let kl = k.to_lowercase();
                                            if kl != "host" && kl != "connection" {
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
                                                let body_bytes =
                                                    resp.bytes().await.ok().map(|b| b.to_vec());
                                                TunnelFrame::Response {
                                                    id,
                                                    status,
                                                    headers: resp_headers,
                                                    body: body_bytes,
                                                }
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "Failed proxying request locally to {}: {}",
                                                    local_req_url, e
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
                                            let _ = tx_clone.send(Message::Binary(resp_bytes));
                                        }
                                    });
                                }
                            }
                            Ok(Message::Ping(payload)) => {
                                let _ = tx.send(Message::Pong(payload));
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

                    // Clean up tasks
                    writer_task.abort();
                    heartbeat_task.abort();
                }
                Err(e) => {
                    error!(
                        "WebSocket connection error: {}. Retrying in 5 seconds...",
                        e
                    );
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}
