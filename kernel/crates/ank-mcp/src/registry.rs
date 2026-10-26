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
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta campo 'name'")?
            .to_string();

        let description = val
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("External MCP tool")
            .to_string();

        let input_schema = val
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "object" }));

        Ok(McpTool {
            name,
            description,
            input_schema,
            session: Some(session),
        })
    }

    /// Elimina herramientas asociadas a sesiones MCP desconectadas.
    pub async fn prune_closed_sessions(&self) -> usize {
        let mut tools = self.tools.write().await;
        let before = tools.len();
        tools.retain(|_, tool| {
            tool.session
                .as_ref()
                .map(|s| !s.is_closed())
                .unwrap_or(false)
        });
        before.saturating_sub(tools.len())
    }

    /// Genera un segmento de System Prompt describiendo las herramientas activas disponibles.
    pub async fn generate_system_prompt(&self) -> String {
        let tools = self.tools.read().await;
        let active_tools: Vec<&McpTool> = tools
            .values()
            .filter(|t| t.session.as_ref().map(|s| !s.is_closed()).unwrap_or(true))
            .collect();

        if active_tools.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n## AVAILABLE MCP TOOLS\n");
        prompt.push_str("You can invoke external tools using the syscall [SYS_MCP_EXEC(tool_name, {args})].\n\n");

        for tool in active_tools {
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

    /// Carga e inicializa servidores MCP definidos en un archivo JSON (`mcpServers`),
    /// ejecutando el handshake `initialize` + `discover_tools` por cada servidor habilitado.
    pub async fn load_from_config_file(&self, config_path: &std::path::Path) -> usize {
        if !config_path.exists() {
            return 0;
        }

        let raw = match std::fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "ANK-MCP: No se pudo leer configuración MCP {:?}: {}",
                    config_path, e
                );
                return 0;
            }
        };

        let parsed: Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                warn!("ANK-MCP: JSON inválido en {:?}: {}", config_path, e);
                return 0;
            }
        };

        let Some(servers) = parsed
            .get("mcpServers")
            .or_else(|| parsed.get("servers"))
            .and_then(|v| v.as_object())
        else {
            warn!(
                "ANK-MCP: {:?} no contiene objeto 'mcpServers' ni 'servers'",
                config_path
            );
            return 0;
        };

        let default_cwd = config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let mut total_tools = 0usize;

        for (server_name, cfg) in servers {
            if cfg
                .get("disabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                debug!(
                    "ANK-MCP: Servidor '{}' deshabilitado; omitiendo.",
                    server_name
                );
                continue;
            }

            let session_opt: Option<Arc<McpClientSession>> =
                if let Some(cmd) = cfg.get("command").and_then(|v| v.as_str()) {
                    let args: Vec<String> = cfg
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();

                    let envs: HashMap<String, String> = cfg
                        .get("env")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                .collect()
                        })
                        .unwrap_or_default();

                    let cwd = cfg
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|| default_cwd.clone());

                    match crate::stdio::StdioTransport::new(cmd, args, envs, cwd) {
                        Ok(t) => Some(McpClientSession::new(t)),
                        Err(e) => {
                            warn!(
                                "ANK-MCP: Falló arranque StdIO para '{}': {}",
                                server_name, e
                            );
                            None
                        }
                    }
                } else if let Some(sse_str) = cfg
                    .get("sseUrl")
                    .or_else(|| cfg.get("sse_url"))
                    .and_then(|v| v.as_str())
                {
                    let post_str = cfg
                        .get("postUrl")
                        .or_else(|| cfg.get("post_url"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(sse_str);

                    match (reqwest::Url::parse(sse_str), reqwest::Url::parse(post_str)) {
                        (Ok(sse_url), Ok(post_url)) => match crate::sse::SseTransport::new(
                            sse_url,
                            post_url,
                            std::time::Duration::from_secs(15),
                        ) {
                            Ok(t) => Some(McpClientSession::new(t)),
                            Err(e) => {
                                warn!("ANK-MCP: Falló creación SSE para '{}': {}", server_name, e);
                                None
                            }
                        },
                        _ => {
                            warn!("ANK-MCP: URLs SSE inválidas en servidor '{}'", server_name);
                            None
                        }
                    }
                } else {
                    warn!(
                        "ANK-MCP: Servidor '{}' no define 'command' ni 'sseUrl'",
                        server_name
                    );
                    None
                };

            let Some(session) = session_opt else {
                continue;
            };

            if let Err(e) = session
                .initialize("aegis-ank-mcp", env!("CARGO_PKG_VERSION"))
                .await
            {
                warn!(
                    "ANK-MCP: Handshake initialize falló en '{}' (intentando tools/list igual): {}",
                    server_name, e
                );
            }

            match self.discover_tools(session).await {
                Ok(count) => {
                    info!(
                        "ANK-MCP: Servidor '{}' conectado ({} herramientas registradas)",
                        server_name, count
                    );
                    total_tools += count;
                }
                Err(e) => {
                    warn!(
                        "ANK-MCP: Falló descubrimiento de herramientas en '{}': {}",
                        server_name, e
                    );
                }
            }
        }

        total_tools
    }
}

/// Dispatcher que enruta la ejecución de herramientas validadas.
pub struct McpToolDispatcher;

impl McpToolDispatcher {
    /// Valida `type` y propiedades `required` del `input_schema` antes de despachar por red/IPC.
    fn validate_args(tool_name: &str, schema: &Value, args: &Value) -> Result<(), McpError> {
        if let Some(schema_type) = schema.get("type").and_then(|v| v.as_str()) {
            if schema_type == "object" && !args.is_object() {
                return Err(McpError::ValidationError(format!(
                    "Se esperaba un objeto JSON para '{}', se recibió {}",
                    tool_name, args
                )));
            }
        }

        if let (Some(required), Some(args_obj)) = (
            schema.get("required").and_then(|v| v.as_array()),
            args.as_object(),
        ) {
            for req_key in required.iter().filter_map(|v| v.as_str()) {
                match args_obj.get(req_key) {
                    None | Some(Value::Null) => {
                        return Err(McpError::ValidationError(format!(
                            "Falta argumento obligatorio '{}' en herramienta MCP '{}'",
                            req_key, tool_name
                        )));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Ejecuta una herramienta MCP si está registrada, su sesión sigue activa y los argumentos son válidos.
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

        if session.is_closed() {
            let _ = registry.prune_closed_sessions().await;
            return Err(McpError::ConnectionClosed);
        }

        Self::validate_args(tool_name, &tool.input_schema, &args)?;

        debug!(
            "ANK-MCP: Ejecutando herramienta '{}' con args: {:?}",
            tool_name, args
        );

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
                                    "required": ["text"],
                                    "properties": {
                                        "text": { "type": "string" }
                                    }
                                }
                            }
                        ]
                    })),
                    error: None,
                };
                futures_util::future::pending::<()>().await;
            };
            Box::pin(stream)
        }
    }

    #[tokio::test]
    async fn test_discovery_prompt_and_required_arg_validation() {
        let registry = McpToolRegistry::new();
        let transport = MockToolServer;
        let session = McpClientSession::new(transport);

        let tool = McpTool {
            name: "test_tool".into(),
            description: "A test tool".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["query"],
                "properties": { "query": { "type": "string" } }
            }),
            session: Some(session),
        };

        {
            let mut tools = registry.tools.write().await;
            tools.insert(tool.name.clone(), tool);
        }

        let prompt = registry.generate_system_prompt().await;
        assert!(prompt.contains("## AVAILABLE MCP TOOLS"));
        assert!(prompt.contains("test_tool"));

        // Missing required field 'query' must be rejected immediately with ValidationError
        let err = McpToolDispatcher::execute(&registry, "test_tool", serde_json::json!({})).await;
        assert!(matches!(err, Err(McpError::ValidationError(_))));
    }
}
