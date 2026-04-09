use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Representación atómica de un mensaje JSON-RPC 2.0 según la especificación MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request {
        jsonrpc: String,
        id: serde_json::Value,
        method: String,
        params: Option<serde_json::Value>,
    },
    Response {
        jsonrpc: String,
        id: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<JsonRpcError>,
    },
    Notification {
        jsonrpc: String,
        method: String,
        params: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Trait fundamental para el transporte de mensajes MCP.
/// Permite un desacoplamiento total entre la lógica del protocolo y el medio físico (SSE, StdIO, etc).
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Envía un mensaje JSON-RPC al servidor MCP.
    async fn send_message(&self, msg: JsonRpcMessage) -> Result<()>;

    /// Suscribe a un flujo continuo de mensajes provenientes del servidor.
    /// Utiliza Pin<Box<...>> para manejar el stream asíncrono de forma genérica.
    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<JsonRpcMessage>> + Send>>;
}
