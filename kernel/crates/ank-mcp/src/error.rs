use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum McpError {
    #[error("La conexión con el servidor MCP se ha cerrado")]
    ConnectionClosed,
    #[error("La petición ha excedido el tiempo de espera (timeout)")]
    Timeout,
    #[error("Error en el transporte: {0}")]
    Transport(String),
    #[error("ID de mensaje inválido o duplicado")]
    InvalidId,
    #[error("Error interno del protocolo JSON-RPC: {0}")]
    Internal(String),
    #[error("Herramienta no encontrada: {0}")]
    ToolNotFound(String),
    #[error("Error de validación de argumentos: {0}")]
    ValidationError(String),
    #[error("Fallo al descubrir herramientas: {0}")]
    DiscoveryFailed(String),
}
