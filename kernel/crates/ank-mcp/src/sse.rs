use crate::transport::{JsonRpcMessage, McpTransport};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::{Client, Url};
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error};

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
}

impl SseTransport {
    /// Crea una nueva instancia de SseTransport.
    /// El `post_url` suele ser la URL donde se envían las peticiones JSON-RPC.
    /// El `sse_url` es el endpoint de Server-Sent Events.
    pub fn new(sse_url: Url, post_url: Url, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .context("Error al inicializar reqwest::Client para MCP")?;

        Ok(Self {
            client,
            sse_url,
            post_url,
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
            let response = client.get(url)
                .send()
                .await
                .map_err(|e| anyhow!(SseError::Network(e)))?;

            if !response.status().is_success() {
                yield Err(anyhow!(SseError::Protocol(format!("SSE Handshake failed: {}", response.status()))))?;
            }

            let mut byte_stream = response.bytes_stream();
            let mut buffer = Vec::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.map_err(|e| anyhow!(SseError::Network(e)))?;
                buffer.extend_from_slice(&chunk);

                // Procesamos el buffer buscando delimitadores de línea (\n o \r\n)
                while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes = buffer.drain(..pos + 1).collect::<Vec<u8>>();
                    let line = String::from_utf8_lossy(&line_bytes);
                    let line = line.trim();

                    if line.starts_with("data:") {
                        let json_str = line.strip_prefix("data: ")
                            .or_else(|| line.strip_prefix("data:"))
                            .unwrap_or(line);

                        if !json_str.is_empty() {
                            let msg: JsonRpcMessage = serde_json::from_str(json_str)
                                .map_err(|e| anyhow!(SseError::Serialization(e)))?;
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
    async fn test_sse_transport_e2e() -> Result<()> {
        // 1. Mock Server setup
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        // POST handler (recibe requests JSON-RPC)
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

        // SSE handler (envía eventos)
        let sse_route = warp::get().and(warp::path("sse")).map(|| {
            let (tx_sse, rx_sse) = tokio::sync::mpsc::unbounded_channel();

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

            // Mantenemos el stream vivo enviando un ping o simplemente no cerrando el canal
            let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx_sse);
            warp::sse::reply(warp::sse::keep_alive().stream(stream))
        });

        let routes = post_route.or(sse_route);
        let addr = ([127, 0, 0, 1], 0); // Port 0 assigns random available port
        let (addr, server) = warp::serve(routes).bind_ephemeral(addr);
        let port = addr.port();

        let server_hnd = tokio::spawn(server);
        sleep(Duration::from_millis(100)).await; // Wait for server to boot

        // 2. Client setup
        let sse_url = Url::parse(&format!("http://127.0.0.1:{}/sse", port))?;
        let post_url = Url::parse(&format!("http://127.0.0.1:{}/rpc", port))?;
        let transport = SseTransport::new(sse_url, post_url, Duration::from_secs(5))?;

        // 3. Test sending
        let req = JsonRpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "ping".to_string(),
            params: None,
        };
        transport.send_message(req.clone()).await?;

        // Verify server received it
        let received_by_server = rx.recv().await.context("Server didn't receive message")?;
        if let JsonRpcMessage::Request { method, .. } = received_by_server {
            assert_eq!(method, "ping");
        } else {
            anyhow::bail!("Wrong message type received by server");
        }

        // 4. Test receiving SSE
        let mut stream = transport.receive_messages();
        // Wait a bit for the event to propagate
        sleep(Duration::from_millis(500)).await;
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

        // Cleanup
        server_hnd.abort();
        Ok(())
    }
}
