use crate::transport::{JsonRpcMessage, McpTransport};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::{Client, Url};
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};

/// Tamaño máximo de buffer acumulado por evento SSE para prevenir agotamiento de RAM (4 MiB).
const MAX_SSE_BUFFER_BYTES: usize = 4 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum SseError {
    #[error("Error de red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Error de protocolo SSE: {0}")]
    Protocol(String),
    #[error("Error de serialización JSON: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Timeout alcanzado al contactar con el Sidecar")]
    Timeout,
}

pub struct SseTransport {
    client: Client,
    sse_url: Url,
    post_url: Url,
    post_timeout: Duration,
}

impl SseTransport {
    /// Crea una nueva instancia de SseTransport.
    ///
    /// CORE-364: `timeout` se aplica únicamente al handshake TCP (`connect_timeout`) y a las
    /// peticiones `POST` individuales, NUNCA como timeout total del stream `GET /sse` de larga vida.
    pub fn new(sse_url: Url, post_url: Url, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(timeout)
            .build()
            .context("Error al inicializar reqwest::Client para MCP")?;

        Ok(Self {
            client,
            sse_url,
            post_url,
            post_timeout: timeout,
        })
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn send_message(&self, msg: JsonRpcMessage) -> Result<()> {
        debug!(
            "Enviando mensaje MCP via POST a {}: {:?}",
            self.post_url, msg
        );

        let response = self
            .client
            .post(self.post_url.clone())
            .timeout(self.post_timeout)
            .json(&msg)
            .send()
            .await
            .map_err(|e| anyhow!(SseError::Network(e)))?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Error al enviar mensaje MCP: HTTP {}", status);
            return Err(anyhow!(SseError::Protocol(format!(
                "HTTP Error: {}",
                status
            ))));
        }

        Ok(())
    }

    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<JsonRpcMessage>> + Send>> {
        let client = self.client.clone();
        let url = self.sse_url.clone();

        let stream = async_stream::try_stream! {
            let response = client
                .get(url)
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .send()
                .await
                .map_err(|e| anyhow!(SseError::Network(e)))?;

            if !response.status().is_success() {
                yield Err(anyhow!(SseError::Protocol(format!(
                    "SSE Handshake failed: {}",
                    response.status()
                ))))?;
            }

            let mut byte_stream = response.bytes_stream();
            let mut buffer: Vec<u8> = Vec::new();
            let mut event_data = String::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.map_err(|e| anyhow!(SseError::Network(e)))?;
                if buffer.len().saturating_add(chunk.len()) > MAX_SSE_BUFFER_BYTES {
                    buffer.clear();
                    event_data.clear();
                    yield Err(anyhow!(SseError::Protocol(
                        "SSE frame exceeded 4 MiB safety cap".into()
                    )))?;
                }
                buffer.extend_from_slice(&chunk);

                // Procesamos el buffer buscando delimitadores de línea (\n o \r\n)
                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                    let raw_line = String::from_utf8_lossy(&line_bytes);
                    let line = raw_line.trim_end_matches(['\r', '\n']);

                    if line.is_empty() {
                        // Fin de bloque SSE: emitir cualquier payload multi-línea pendiente
                        let payload = event_data.trim();
                        if !payload.is_empty() {
                            match serde_json::from_str::<JsonRpcMessage>(payload) {
                                Ok(msg) => {
                                    event_data.clear();
                                    yield msg;
                                }
                                Err(e) => {
                                    warn!("ANK-MCP SSE: Ignorando evento no-JSON-RPC ({}): {}", e, payload);
                                    event_data.clear();
                                }
                            }
                        }
                        continue;
                    }

                    if line.starts_with(':') {
                        // Comentario / keep-alive SSE
                        continue;
                    }

                    if let Some(rest) = line.strip_prefix("data:") {
                        let part = rest.strip_prefix(' ').unwrap_or(rest);
                        if event_data.len().saturating_add(part.len()).saturating_add(1) > MAX_SSE_BUFFER_BYTES {
                            event_data.clear();
                            yield Err(anyhow!(SseError::Protocol(
                                "Multi-line SSE data exceeded 4 MiB safety cap".into()
                            )))?;
                        }
                        if !event_data.is_empty() {
                            event_data.push('\n');
                        }
                        event_data.push_str(part);

                        // Fast-path: si la línea ya conforma un mensaje JSON-RPC completo, emitirlo de inmediato
                        if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(event_data.trim()) {
                            event_data.clear();
                            yield msg;
                        }
                    }
                }
            }
        };

        Box::pin(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use tokio::time::sleep;
    use warp::Filter;

    #[tokio::test]
    async fn test_sse_transport_e2e_and_survives_past_post_timeout() -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let post_route = warp::post()
            .and(warp::path("rpc"))
            .and(warp::body::json())
            .map(move |msg: JsonRpcMessage| {
                let _ = tx.try_send(msg);
                warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({"status": "ok"})),
                    warp::http::StatusCode::OK,
                )
            });

        // SSE handler: emite un evento después de 350ms mientras post_timeout es solo 200ms!
        // Antes de CORE-364, el stream moría a los 200ms por el timeout global del Client.
        let sse_route = warp::get().and(warp::path("sse")).map(|| {
            let (tx_sse, rx_sse) = tokio::sync::mpsc::unbounded_channel();

            tokio::spawn(async move {
                sleep(Duration::from_millis(350)).await;
                let msg = JsonRpcMessage::Notification {
                    jsonrpc: "2.0".to_string(),
                    method: "test/event".to_string(),
                    params: Some(serde_json::json!({"data": "hello"})),
                };
                let json_msg = serde_json::to_string(&msg)
                    .unwrap_or_else(|_| r#"{"error": "mock_serialization_failed"}"#.to_string());
                let _ = tx_sse.send(Ok::<_, Infallible>(
                    warp::sse::Event::default().data(json_msg),
                ));
                // Keep channel alive briefly
                sleep(Duration::from_millis(200)).await;
            });

            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx_sse);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

        let routes = post_route.or(sse_route);
        let addr = ([127, 0, 0, 1], 0);
        let (addr, server) = warp::serve(routes).bind_ephemeral(addr);
        let port = addr.port();

        let server_hnd = tokio::spawn(server);
        sleep(Duration::from_millis(50)).await;

        let sse_url = Url::parse(&format!("http://127.0.0.1:{}/sse", port))?;
        let post_url = Url::parse(&format!("http://127.0.0.1:{}/rpc", port))?;
        let transport = SseTransport::new(sse_url, post_url, Duration::from_millis(200))?;

        let req = JsonRpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "ping".to_string(),
            params: None,
        };
        transport.send_message(req).await?;

        let received_by_server = rx.recv().await.context("Server didn't receive message")?;
        if let JsonRpcMessage::Request { method, .. } = received_by_server {
            assert_eq!(method, "ping");
        } else {
            anyhow::bail!("Wrong message type received by server");
        }

        let mut stream = transport.receive_messages();
        if let Some(res) = stream.next().await {
            let msg = res?;
            if let JsonRpcMessage::Notification { method, .. } = msg {
                assert_eq!(method, "test/event");
            } else {
                anyhow::bail!("Wrong message type received via SSE");
            }
        } else {
            anyhow::bail!("SSE stream closed prematurely");
        }

        server_hnd.abort();
        Ok(())
    }
}
