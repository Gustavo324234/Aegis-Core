use crate::error::McpError;
use crate::transport::{JsonRpcMessage, McpTransport};
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Alias para el mapa de peticiones pendientes ruteadas por ID.
type PendingRequests =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<JsonRpcMessage, McpError>>>>>;

/// Sesión de cliente MCP que multiplexa peticiones JSON-RPC sobre un transporte asíncrono.
/// Implementa el Patrón Actor para gestionar el estado de las peticiones en vuelo.
pub struct McpClientSession {
    transport: Arc<dyn McpTransport>,
    pending_requests: PendingRequests,
}

impl std::fmt::Debug for McpClientSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClientSession")
            .field("pending_requests_count", &"...") // Simplificado para evitar bloqueos
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
        });

        let session_clone = session.clone();
        tokio::spawn(async move {
            session_clone.background_loop().await;
        });

        session
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
                    warn!("ANK-MCP: Servidor intentó invocar el método '{}'. Bidireccionalidad no implementada en cliente.", method);
                }
                Err(e) => {
                    error!("ANK-MCP: Error en el stream de transporte: {}", e);
                    break;
                }
            }
        }

        // Escalación SRE: Si el transporte muere, abortar todas las promesas pendientes.
        warn!(
            "ANK-MCP: Transporte cerrado. Limpiando {} peticiones pendientes.",
            self.pending_requests.lock().await.len()
        );

        let mut pending = self.pending_requests.lock().await;
        for (id, tx) in pending.drain() {
            debug!("ANK-MCP: Notificando ConnectionClosed a ID: {}", id);
            let _ = tx.send(Err(McpError::ConnectionClosed));
        }
    }

    /// API Pública: "Fire and Await".
    /// Envía una petición y espera la respuesta asíncrona con un timeout de 30s.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        // 1. Registro de la promesa
        {
            let mut pending = self.pending_requests.lock().await;
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

        // 3. Await con Resiliencia (Timeout de 30 segundos según especificación)
        match timeout(Duration::from_secs(30), rx).await {
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
            Ok(Ok(Err(e))) => {
                // Error propagado desde el loop (ej ConnectionClosed)
                Err(e)
            }
            Ok(Err(_)) => {
                // RecvError: El sender fue droppeado sin enviar nada
                error!(
                    "ANK-MCP: El canal de respuesta se cerró prematuramente para ID: {}",
                    id
                );
                Err(McpError::ConnectionClosed)
            }
            Err(_) => {
                // Timeout
                warn!(
                    "ANK-MCP: Timeout excedido (30s) esperando respuesta para ID: {}",
                    id
                );
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(McpError::Timeout)
            }
            Ok(Ok(_)) => {
                error!("ANK-MCP: Flujo de control inválido. Se recibió un mensaje no-Response para ID: {}", id);
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
                // Yielding nothing is fine, but we need to satisfy the type inference
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

    #[tokio::test]
    async fn test_client_session_timeout() -> anyhow::Result<()> {
        let transport = MockTransport {
            delay: Duration::from_secs(40),
        }; // Mayor que el timeout de 30s
        let session = McpClientSession::new(transport);

        let result = session.call("test/method", Value::Null).await;

        match result {
            Err(McpError::Timeout) => info!("Test pasado: Timeout detectado correctamente"),
            other => anyhow::bail!("Se esperaba McpError::Timeout, se obtuvo: {:?}", other),
        }
        Ok(())
    }
}
