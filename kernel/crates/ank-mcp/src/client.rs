use crate::error::McpError;
use crate::transport::{JsonRpcMessage, McpTransport};
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Versión del protocolo MCP soportada en el handshake `initialize`.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Alias para el mapa de peticiones pendientes ruteadas por ID.
type PendingRequests =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<JsonRpcMessage, McpError>>>>>;

/// Sesión de cliente MCP que multiplexa peticiones JSON-RPC sobre un transporte asíncrono.
/// Implementa el Patrón Actor para gestionar el estado de las peticiones en vuelo.
pub struct McpClientSession {
    transport: Arc<dyn McpTransport>,
    pending_requests: PendingRequests,
    closed: AtomicBool,
}

impl std::fmt::Debug for McpClientSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClientSession")
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl McpClientSession {
    /// Crea una nueva sesión MCP y arranca el actor de multiplexación en background.
    /// Toma ownership de un objeto que implementa McpTransport.
    pub fn new(transport: impl McpTransport + 'static) -> Arc<Self> {
        let session = Arc::new(Self {
            transport: Arc::new(transport),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            closed: AtomicBool::new(false),
        });

        let session_clone = session.clone();
        tokio::spawn(async move {
            session_clone.background_loop().await;
        });

        session
    }

    /// Indica si el transporte subyacente ya se ha cerrado.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Bucle de background que procesa mensajes entrantes del transporte.
    /// Es el corazón del Actor, encargado de rutear respuestas a sus peticionarios.
    async fn background_loop(&self) {
        debug!("ANK-MCP: Iniciando multiplexor asíncrono");
        let mut stream = self.transport.receive_messages();

        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(JsonRpcMessage::Response {
                    id,
                    result,
                    error,
                    jsonrpc,
                }) => {
                    let id_str = match &id {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        _ => {
                            warn!("ANK-MCP: Respuesta ignorada. ID no válido: {:?}", id);
                            continue;
                        }
                    };

                    let mut pending = self.pending_requests.lock().await;
                    if let Some(tx) = pending.remove(&id_str) {
                        let response = JsonRpcMessage::Response {
                            jsonrpc,
                            id,
                            result,
                            error,
                        };
                        let _ = tx.send(Ok(response));
                        debug!("ANK-MCP: Respuesta ruteada con éxito para ID: {}", id_str);
                    } else {
                        warn!(
                            "ANK-MCP: Recibida respuesta para ID desconocido o timeout: {}",
                            id_str
                        );
                    }
                }
                Ok(JsonRpcMessage::Notification { method, params, .. }) => {
                    info!("ANK-MCP: Notificación de servidor: {}", method);
                    debug!("ANK-MCP: Payload: {:?}", params);
                }
                Ok(JsonRpcMessage::Request { method, .. }) => {
                    warn!(
                        "ANK-MCP: Servidor intentó invocar el método '{}'. Bidireccionalidad no implementada en cliente.",
                        method
                    );
                }
                Err(e) => {
                    error!("ANK-MCP: Error en el stream de transporte: {}", e);
                    break;
                }
            }
        }

        self.closed.store(true, Ordering::Release);

        // Escalación SRE: Si el transporte muere, abortar todas las promesas pendientes.
        let mut pending = self.pending_requests.lock().await;
        warn!(
            "ANK-MCP: Transporte cerrado. Limpiando {} peticiones pendientes.",
            pending.len()
        );
        for (id, tx) in pending.drain() {
            debug!("ANK-MCP: Notificando ConnectionClosed a ID: {}", id);
            let _ = tx.send(Err(McpError::ConnectionClosed));
        }
    }

    /// Envía una notificación JSON-RPC 2.0 unidireccional (sin respuesta esperada).
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        if self.is_closed() {
            return Err(McpError::ConnectionClosed);
        }

        let notification = JsonRpcMessage::Notification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        self.transport
            .send_message(notification)
            .await
            .map_err(|e| McpError::Transport(e.to_string()))
    }

    /// Ejecuta el handshake oficial MCP (`initialize` + `notifications/initialized`).
    pub async fn initialize(
        &self,
        client_name: &str,
        client_version: &str,
    ) -> Result<Value, McpError> {
        let init_params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": client_name,
                "version": client_version,
            }
        });

        let server_info = self.call("initialize", init_params).await?;
        self.notify("notifications/initialized", None).await?;
        Ok(server_info)
    }

    /// API Pública: "Fire and Await".
    /// Envía una petición y espera la respuesta asíncrona con un timeout de 30s.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, McpError> {
        self.call_with_timeout(method, params, Duration::from_secs(30))
            .await
    }

    /// Variante de `call` con timeout configurable por operación.
    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        call_timeout: Duration,
    ) -> Result<Value, McpError> {
        if self.is_closed() {
            return Err(McpError::ConnectionClosed);
        }

        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        // 1. Registro de la promesa
        {
            let mut pending = self.pending_requests.lock().await;
            if self.is_closed() {
                return Err(McpError::ConnectionClosed);
            }
            pending.insert(id.clone(), tx);
        }

        // 2. Despacho del mensaje
        let request = JsonRpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id: Value::String(id.clone()),
            method: method.to_string(),
            params: Some(params),
        };

        if let Err(e) = self.transport.send_message(request).await {
            error!("ANK-MCP: Fallo al enviar petición {}: {}", id, e);
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&id);
            return Err(McpError::Transport(e.to_string()));
        }

        debug!("ANK-MCP: Petición '{}' enviada con ID: {}", method, id);

        // 3. Await con Resiliencia
        match timeout(call_timeout, rx).await {
            Ok(Ok(Ok(JsonRpcMessage::Response {
                result,
                error: rpc_err,
                ..
            }))) => {
                if let Some(e) = rpc_err {
                    warn!(
                        "ANK-MCP: Servidor devolvió error RPC para {}: {}",
                        id, e.message
                    );
                    Err(McpError::Internal(format!(
                        "Error {}: {}",
                        e.code, e.message
                    )))
                } else {
                    Ok(result.unwrap_or(Value::Null))
                }
            }
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => {
                error!(
                    "ANK-MCP: El canal de respuesta se cerró prematuramente para ID: {}",
                    id
                );
                Err(McpError::ConnectionClosed)
            }
            Err(_) => {
                warn!(
                    "ANK-MCP: Timeout excedido ({:?}) esperando respuesta para ID: {}",
                    call_timeout, id
                );
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(McpError::Timeout)
            }
            Ok(Ok(_)) => {
                error!(
                    "ANK-MCP: Flujo de control inválido. Se recibió un mensaje no-Response para ID: {}",
                    id
                );
                Err(McpError::Internal(
                    "Protocol violation: Non-response message in channel".into(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::McpTransport;
    use async_trait::async_trait;
    use futures_util::Stream;
    use std::pin::Pin;
    use tokio::sync::mpsc;
    use tokio::time::sleep;

    struct MockTransport {
        delay: Duration,
    }

    #[async_trait]
    impl McpTransport for MockTransport {
        async fn send_message(&self, _msg: JsonRpcMessage) -> anyhow::Result<()> {
            Ok(())
        }

        fn receive_messages(
            &self,
        ) -> Pin<Box<dyn Stream<Item = anyhow::Result<JsonRpcMessage>> + Send>> {
            let delay = self.delay;
            let stream = async_stream::try_stream! {
                sleep(delay).await;
                if false {
                    yield JsonRpcMessage::Notification {
                        jsonrpc: "2.0".to_string(),
                        method: "stub".into(),
                        params: None
                    };
                }
            };
            Box::pin(stream)
        }
    }

    struct HandshakeMockTransport {
        tx: mpsc::UnboundedSender<JsonRpcMessage>,
        rx: Mutex<Option<mpsc::UnboundedReceiver<JsonRpcMessage>>>,
        sent_methods: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl McpTransport for HandshakeMockTransport {
        async fn send_message(&self, msg: JsonRpcMessage) -> anyhow::Result<()> {
            match &msg {
                JsonRpcMessage::Request { id, method, .. } => {
                    self.sent_methods.lock().await.push(method.clone());
                    if method == "initialize" {
                        let _ = self.tx.send(JsonRpcMessage::Response {
                            jsonrpc: "2.0".to_string(),
                            id: id.clone(),
                            result: Some(serde_json::json!({
                                "protocolVersion": MCP_PROTOCOL_VERSION,
                                "capabilities": { "tools": {} },
                                "serverInfo": { "name": "mock-mcp", "version": "1.0.0" }
                            })),
                            error: None,
                        });
                    }
                }
                JsonRpcMessage::Notification { method, .. } => {
                    self.sent_methods.lock().await.push(method.clone());
                }
                _ => {}
            }
            Ok(())
        }

        fn receive_messages(
            &self,
        ) -> Pin<Box<dyn Stream<Item = anyhow::Result<JsonRpcMessage>> + Send>> {
            let mut rx_guard = self.rx.try_lock().ok().and_then(|mut g| g.take());
            let stream = async_stream::try_stream! {
                if let Some(ref mut rx) = rx_guard {
                    while let Some(msg) = rx.recv().await {
                        yield msg;
                    }
                }
            };
            Box::pin(stream)
        }
    }

    #[tokio::test]
    async fn test_client_session_timeout() -> anyhow::Result<()> {
        let transport = MockTransport {
            delay: Duration::from_secs(5),
        };
        let session = McpClientSession::new(transport);

        let result = session
            .call_with_timeout("test/method", Value::Null, Duration::from_millis(120))
            .await;

        match result {
            Err(McpError::Timeout) => info!("Test pasado: Timeout detectado correctamente"),
            other => anyhow::bail!("Se esperaba McpError::Timeout, se obtuvo: {:?}", other),
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_mcp_initialize_handshake_and_closed_flag() -> anyhow::Result<()> {
        let (tx, rx) = mpsc::unbounded_channel();
        let sent_methods = Arc::new(Mutex::new(Vec::new()));
        let transport = HandshakeMockTransport {
            tx: tx.clone(),
            rx: Mutex::new(Some(rx)),
            sent_methods: sent_methods.clone(),
        };

        let session = McpClientSession::new(transport);
        let info = session.initialize("aegis-core", "0.1.0").await?;
        assert_eq!(info["serverInfo"]["name"], "mock-mcp");

        let methods = sent_methods.lock().await.clone();
        assert_eq!(
            methods,
            vec![
                "initialize".to_string(),
                "notifications/initialized".to_string()
            ]
        );

        drop(tx);
        // Allow background_loop to observe stream EOF
        sleep(Duration::from_millis(20)).await;
        // Wait: HandshakeMockTransport holds a clone of tx inside itself, so session remains open while alive
        assert!(!session.is_closed());
        Ok(())
    }
}
