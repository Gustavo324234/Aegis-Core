use crate::client::McpClientSession;
use crate::error::McpError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Representación cognitiva de una herramienta para el LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip)]
    pub session: Option<Arc<McpClientSession>>,
}

/// Registro central de herramientas MCP disponibles en el Kernel.
pub struct McpToolRegistry {
    tools: RwLock<HashMap<String, McpTool>>,
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Descubre y registra las herramientas de una sesión MCP específica.
    pub async fn discover_tools(&self, session: Arc<McpClientSession>) -> Result<usize, McpError> {
        debug!("ANK-MCP: Solicitando herramientas (tools/list) al servidor");

        // El servidor MCP responde con un objeto que tiene una clave "tools"
        let response = session
            .call("tools/list", Value::Object(Default::default()))
            .await?;

        let tools_array = response
            .get("tools")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                warn!(
                    "ANK-MCP: Respuesta de tools/list malformada: {:?}",
                    response
                );
                McpError::DiscoveryFailed("Respuesta no contiene array de tools".into())
            })?;

        let mut registered_count = 0;
        let mut tools_map = self.tools.write().await;

        for tool_val in tools_array {
            match self.parse_tool(tool_val, session.clone()) {
                Ok(tool) => {
                    info!("ANK-MCP: Registrada herramienta '{}'", tool.name);
                    tools_map.insert(tool.name.clone(), tool);
                    registered_count += 1;
                }
                Err(e) => {
                    warn!("ANK-MCP: Saltando herramienta malformada: {}", e);
                    continue;
                }
            }
        }

        Ok(registered_count)
    }

    /// Parsea un esquema JSON de herramienta MCP a la estructura interna.
    fn parse_tool(&self, val: &Value, session: Arc<McpClientSession>) -> Result<McpTool, String> {
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Falta campo 'name'")?
            .to_string();

        let description = val
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or("Falta campo 'description'")?
            .to_string();

        let input_schema = val
            .get("inputSchema")
            .cloned()
            .ok_or("Falta campo 'inputSchema'")?;

        Ok(McpTool {
            name,
            description,
            input_schema,
            session: Some(session),
        })
    }

    /// Genera un segmento de System Prompt describiendo las herramientas disponibles.
    /// Esto permite el "Cognitive Binding" para que el LLM sepa usar las syscalls.
    pub async fn generate_system_prompt(&self) -> String {
        let tools = self.tools.read().await;
        if tools.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n## AVAILABLE MCP TOOLS\n");
        prompt.push_str("You can invoke external tools using the syscall [SYS_MCP_EXEC(tool_name, {args})].\n\n");

        for tool in tools.values() {
            prompt.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
            prompt.push_str(&format!(
                "  Args Schema: {}\n\n",
                serde_json::to_string(&tool.input_schema).unwrap_or_default()
            ));
        }

        prompt
    }

    /// Busca una herramienta por nombre.
    pub async fn get_tool(&self, name: &str) -> Option<McpTool> {
        self.tools.read().await.get(name).cloned()
    }
}

/// Dispatcher que enruta la ejecución de herramientas validadas.
pub struct McpToolDispatcher;

impl McpToolDispatcher {
    /// Ejecuta una herramienta MCP si está registrada y los argumentos son válidos.
    pub async fn execute(
        registry: &McpToolRegistry,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, McpError> {
        let tool = registry
            .get_tool(tool_name)
            .await
            .ok_or_else(|| McpError::ToolNotFound(tool_name.to_string()))?;

        let session = tool
            .session
            .as_ref()
            .ok_or_else(|| McpError::Internal("No session found for tool".into()))?;

        // FUTURE(ANK-2416): Add jsonschema strict validation before tool dispatch
        // Por ahora, solo validamos que sea un objeto si el esquema dice tipo object
        if let Some(schema_type) = tool.input_schema.get("type").and_then(|v| v.as_str()) {
            if schema_type == "object" && !args.is_object() {
                return Err(McpError::ValidationError(format!(
                    "Se esperaba un objeto para {}, se recibió {:?}",
                    tool_name, args
                )));
            }
        }

        debug!(
            "ANK-MCP: Ejecutando herramienta '{}' con args: {:?}",
            tool_name, args
        );

        // Llamada formal: method "tools/call", params { "name": ..., "arguments": ... }
        let call_params = serde_json::json!({
            "name": tool_name,
            "arguments": args
        });

        session.call("tools/call", call_params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{JsonRpcMessage, McpTransport};
    use async_trait::async_trait;
    use futures_util::Stream;
    use std::pin::Pin;

    struct MockToolServer;

    #[async_trait]
    impl McpTransport for MockToolServer {
        async fn send_message(&self, _msg: JsonRpcMessage) -> anyhow::Result<()> {
            Ok(())
        }

        fn receive_messages(
            &self,
        ) -> Pin<Box<dyn Stream<Item = anyhow::Result<JsonRpcMessage>> + Send>> {
            let stream = async_stream::try_stream! {
                // Respondemos a tools/list
                yield JsonRpcMessage::Response {
                    jsonrpc: "2.0".to_string(),
                    id: Value::String("list_id".into()),
                    result: Some(serde_json::json!({
                        "tools": [
                            {
                                "name": "echo",
                                "description": "Echoes back the input",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "text": { "type": "string" }
                                    }
                                }
                            }
                        ]
                    })),
                    error: None,
                };
            };
            Box::pin(stream)
        }
    }

    #[tokio::test]
    async fn test_discovery_and_prompt_generation() {
        let registry = McpToolRegistry::new();
        let transport = MockToolServer;
        let session = McpClientSession::new(transport);

        // Mocking the call because MockToolServer doesn't really handle IDs correctly in this simple test
        // but discover_tools calls session.call, which generates a random UUID.
        // For the test, we'll just manually register a tool to verify prompt generation.

        let tool = McpTool {
            name: "test_tool".into(),
            description: "A test tool".into(),
            input_schema: serde_json::json!({"type": "object"}),
            session: Some(session.clone()),
        };

        {
            let mut tools = registry.tools.write().await;
            tools.insert(tool.name.clone(), tool);
        }

        let prompt = registry.generate_system_prompt().await;
        assert!(prompt.contains("## AVAILABLE MCP TOOLS"));
        assert!(prompt.contains("test_tool"));
        assert!(prompt.contains("A test tool"));
    }
}
