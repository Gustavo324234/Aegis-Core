use crate::agents::instructions::InstructionLoader;
use crate::agents::node::AgentRole;
use crate::agents::tool_registry::{ProviderKind, ToolRegistry};
use crate::pcb::PCB;
use crate::plugins::PluginManager;
use crate::router::{CognitiveRouter, RoutingDecision};
use crate::scheduler::{ModelPreference, SharedPCB};
use crate::vcm::swap::LanceSwapManager;
use crate::vcm::VirtualContextManager;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

pub mod drivers;
pub mod hardware;
#[async_trait]
pub trait EmbeddingDriver: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SystemError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, SystemError>;
}

/// --- SYSTEM PROMPT ---
/// CORE-128: System prompt base honesto y sin alucinaciones.
/// - No inventa capacidades que no tiene mediante herramientas activas.
/// - No inventa acciones que no ejecutó.
/// - Sin listas innecesarias en respuestas conversacionales.
/// - Identidad: "Aegis" por defecto; la Persona del tenant la sobreescribe (CORE-129).
pub const SYSTEM_PROMPT_MASTER: &str = "\
Sos un asistente personal inteligente y cercano. Respondés en el idioma del usuario.\n\
\n\
TONO Y ESTILO:\n\
- Conversá de forma natural y cálida, como un asistente de confianza.\n\
- Respondés con la extensión adecuada al contexto: corto para saludos, \
  más elaborado cuando la pregunta lo requiere.\n\
- Cuando no sabés algo o no podés hacer algo, lo decís de forma amigable \
  y ofrecés alternativas si las hay. Nunca respondés solo \"No sé.\" \
  — siempre agregás contexto útil.\n\
- No anunciés tus capacidades espontáneamente. Si el usuario pregunta \
  qué podés hacer, entonces sí explicás.\n\
\n\
PRECISIÓN:\n\
- Solo afirmás que hiciste algo si una herramienta te devolvió un resultado concreto.\n\
- No inventés datos, cifras ni hechos que no tenés.\n\
- Si ejecutaste una herramienta y no devolvió resultados útiles, lo decís claramente.\n\
";

/// CORE-150 / CORE-181: Instrucciones para la capacidad Maker (Scripting Autónomo).
pub const MAKER_INSTRUCTIONS: &str = "\
\n\n[CAPACIDAD: MAKER]\n\
Podés ejecutar scripts JavaScript para automatizar tareas complejas o procesar datos. \
El entorno es un sandbox seguro con acceso al /workspace del usuario.\n\
Sintaxis: [SYS_CALL_MAKER(\"js\", \"código aquí\", {\"param1\": \"valor\"})]\n\
Funciones disponibles en JS:\n\
- read_file(path): Lee un archivo del workspace.\n\
- write_file(path, content): Escribe un archivo en el workspace.\n\
- params: Objeto que contiene los parámetros pasados en el tercer argumento.\n\
Usa esta capacidad cuando necesites realizar operaciones repetitivas, \
procesamiento de archivos pesados o lógica que no podés expresar solo con texto.\n\
\n\
MAKER — ENTORNO DE EJECUCIÓN JS:\n\
- El entorno es Boa Engine embebido (NO Node.js, NO browser).\n\
- No existe `require`, `import`, `module`, `exports`, `process`, `__dirname`.\n\
- Solo están disponibles: read_file(path), write_file(path, content), params (objeto con los parámetros).\n\
- Podés usar `return` para retornar el resultado — el script se ejecuta como función.\n\
- El resultado del script es el valor retornado (o el último valor evaluado).\n\
- Usá solo JavaScript puro ES2020 sin módulos externos.\n";

/// EPIC 47: Las instrucciones multi-agente ya no se inyectan como texto.
/// El Agent Protocol v2 usa Tool Use nativo — el ToolRegistry registra las herramientas
/// en cada llamada de inferencia según el rol del agente (CORE-236).
/// Esta constante se mantiene vacía para no romper los sitios de uso en build_prompt.
pub const SPAWN_INSTRUCTIONS: &str = "";

/// Template para inyectar la Persona del tenant cuando está configurada (CORE-129).
/// `{persona}` se reemplaza con el texto libre del operador.
pub const PERSONA_SECTION_TEMPLATE: &str =
    "\n\n[IDENTIDAD CONFIGURADA POR EL OPERADOR]\n{persona}\n[FIN DE IDENTIDAD]\n";

/// --- CHAL ERROR SYSTEM ---
#[derive(Error, Debug, Clone)]
pub enum SystemError {
    #[error("VRAM Exhausted: cannot load model or process prompt")]
    VramExhausted,
    #[error("Driver Offline: the inference engine {0} is not responding")]
    DriverOffline(String),
    #[error("Model Not Found: {0}")]
    ModelNotFound(String),
    #[error("Hardware Failure: {0}")]
    HardwareFailure(String),
    #[error("Decision Error: {0}")]
    DecisionError(String),
}

#[derive(Error, Debug, Clone)]
pub enum ExecutionError {
    #[error("Stream Interrupted: {0}")]
    Interrupted(String),
    #[error("Safety Violation: Content blocked by filter")]
    SafetyViolation,
    #[error("Processing Timeout")]
    Timeout,
}

/// --- SUPPORT TYPES ---
#[derive(Debug, Clone, Default)]
pub struct DriverStatus {
    pub is_ready: bool,
    pub vram_usage_bytes: u64,
    pub active_models: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Grammar {
    Gbnf(String),
    JsonSchema(serde_json::Value),
}

use std::task::{Context, Poll};

pub struct SyncStream<S>(pub S);
unsafe impl<S: Send> Sync for SyncStream<S> {}
impl<S: Stream> Stream for SyncStream<S> {
    type Item = S::Item;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe { self.map_unchecked_mut(|s| &mut s.0).poll_next(cx) }
    }
}

pub type GenerateStreamResult =
    Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send + Sync>>, SystemError>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ChatMessage {
    pub role: ChatRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    #[default]
    User,
    System,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionCallRecord,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionCallRecord {
    pub name: String,
    pub arguments: String,
}

/// --- INFERENCE DRIVER INTERFACE ---
#[async_trait]
pub trait InferenceDriver: Send + Sync {
    async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        grammar: Option<Grammar>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> GenerateStreamResult;

    async fn get_health_status(&self) -> DriverStatus;

    async fn load_model(&mut self, model_id: &str) -> Result<(), SystemError>;
}

/// Formato del token `__TOOL_CALL__` emitido por el CloudProxyDriver (CORE-261).
/// Distinto del OpenAI `ToolCallRecord` — se convierte antes de inyectar al historial.
#[derive(serde::Deserialize)]
struct DriverToolCallPayload {
    id: String,
    name: String,
    arguments: serde_json::Value,
}

/// --- COGNITIVE HAL (Hardware Abstraction Layer) ---
pub struct CognitiveHAL {
    pub drivers: RwLock<HashMap<String, Box<dyn InferenceDriver + Send + Sync>>>,
    pub plugin_manager: Arc<RwLock<PluginManager>>,
    pub mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
    pub router: RwLock<Option<Arc<RwLock<CognitiveRouter>>>>,
    pub hardware: tokio::sync::Mutex<hardware::HardwareMonitor>,
    pub http_client: Arc<reqwest::Client>,
    pub vcm: VirtualContextManager,
    pub swap_manager: Arc<LanceSwapManager>,
    pub embedding_driver: Option<Arc<dyn EmbeddingDriver>>,
    /// CORE-226: InstructionLoader para inyectar chat_agent.md al Chat Agent principal.
    pub instruction_loader: Arc<RwLock<InstructionLoader>>,
    /// CORE-261: AgentOrchestrator para ejecutar tool calls dentro del bucle ReAct.
    pub agent_orchestrator: RwLock<Option<Arc<crate::agents::orchestrator::AgentOrchestrator>>>,
    /// CORE-267: Referencia al router para notificar rate-limits desde el driver.
    pub router_ref: RwLock<Option<Arc<RwLock<CognitiveRouter>>>>,
}

#[cfg(test)]
fn _assert_hal_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<CognitiveHAL>();
}

impl CognitiveHAL {
    pub fn new(plugin_manager: Arc<RwLock<PluginManager>>) -> Result<Self, SystemError> {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    SystemError::HardwareFailure(format!("reqwest::Client::builder failed: {}", e))
                })?,
        );

        let mut drivers: HashMap<String, Box<dyn InferenceDriver + Send + Sync>> = HashMap::new();

        if let Some(cloud_driver) =
            crate::chal::drivers::CloudProxyDriver::from_env(Arc::clone(&http_client))
        {
            drivers.insert("cloud-driver".to_string(), Box::new(cloud_driver));
            tracing::info!("CloudProxyDriver initialized via ENV vars and registered.");
        }

        let data_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let swap_manager = Arc::new(LanceSwapManager::new(&data_dir));

        let mut loader = InstructionLoader::default_from_workspace(std::path::Path::new(&data_dir));
        let _ = loader.preload();
        let instruction_loader = Arc::new(RwLock::new(loader));

        let embedding_driver: Option<Arc<dyn EmbeddingDriver>> = if let Some(cloud_driver) =
            crate::chal::drivers::CloudProxyDriver::from_env(Arc::clone(&http_client))
        {
            let model = std::env::var("AEGIS_CLOUD_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string());
            Some(Arc::new(
                crate::chal::drivers::embeddings::CloudEmbeddingDriver::new(
                    Arc::clone(&http_client),
                    cloud_driver.api_url.clone(),
                    cloud_driver.api_key.clone(),
                    model,
                ),
            ))
        } else {
            None
        };

        Ok(Self {
            drivers: RwLock::new(drivers),
            plugin_manager,
            mcp_registry: Arc::new(ank_mcp::registry::McpToolRegistry::new()),
            router: RwLock::new(None),
            hardware: tokio::sync::Mutex::new(hardware::HardwareMonitor::new()),
            http_client,
            vcm: VirtualContextManager::new(),
            swap_manager,
            embedding_driver,
            instruction_loader,
            agent_orchestrator: RwLock::new(None),
            router_ref: RwLock::new(None),
        })
    }

    pub async fn set_router(&self, router: Arc<RwLock<CognitiveRouter>>) {
        let mut r = self.router.write().await;
        *r = Some(router);
    }

    /// CORE-267: Registra el router para notificar rate-limits desde el CloudProxyDriver.
    pub async fn set_router_ref(&self, router: Arc<RwLock<CognitiveRouter>>) {
        let mut r = self.router_ref.write().await;
        *r = Some(router);
    }

    /// CORE-261: Registra el AgentOrchestrator para uso en el bucle ReAct.
    pub async fn set_orchestrator(
        &self,
        orchestrator: Arc<crate::agents::orchestrator::AgentOrchestrator>,
    ) {
        let mut a = self.agent_orchestrator.write().await;
        *a = Some(orchestrator);
    }

    pub async fn register_driver(&self, id: &str, driver: Box<dyn InferenceDriver + Send + Sync>) {
        let mut drivers = self.drivers.write().await;
        drivers.insert(id.to_string(), driver);
        tracing::info!(driver_id = %id, "New driver registered in HAL.");
    }

    pub async fn update_cloud_credentials(&self, api_url: String, model: String, api_key: String) {
        let cloud_driver = crate::chal::drivers::CloudProxyDriver::new(
            Arc::clone(&self.http_client),
            api_url,
            api_key,
            model.clone(),
        );
        let mut drivers = self.drivers.write().await;
        drivers.insert("cloud-driver".to_string(), Box::new(cloud_driver));
        tracing::info!(model = %model, "CloudProxyDriver credentials updated dynamically and driver re-registered in HAL.");
    }

    pub async fn route_and_execute(
        self: Arc<Self>,
        shared_pcb: SharedPCB,
        persona: Option<String>,
    ) -> GenerateStreamResult {
        let (instruction, priority, model_pref, pid) = {
            let pcb = shared_pcb.read().await;
            (
                pcb.memory_pointers.l1_instruction.clone(),
                pcb.priority,
                pcb.model_pref,
                pcb.pid.clone(),
            )
        };

        // Try CognitiveRouter first if available — bucle ReAct (CORE-261)
        let router_opt = self.router.read().await.clone();
        if let Some(router_rw) = router_opt {
            let router = router_rw.read().await;
            let pcb_snapshot = {
                let pcb = shared_pcb.read().await;
                pcb.clone()
            };
            match router.decide(&pcb_snapshot).await {
                Ok(decision) => {
                    let (text_tx, text_rx) =
                        tokio::sync::mpsc::unbounded_channel::<Result<String, ExecutionError>>();
                    let hal_arc = Arc::clone(&self);
                    let pid_str = pid.clone();
                    let persona_str = persona.clone();
                    let pcb_clone = pcb_snapshot.clone();
                    let err_tx = text_tx.clone();

                    tokio::spawn(async move {
                        if let Err(e) = hal_arc
                            .execute_with_decision(
                                decision,
                                &pcb_clone,
                                &pid_str,
                                persona_str.as_deref(),
                                text_tx,
                            )
                            .await
                        {
                            tracing::error!(pid = %pid_str, "ReAct loop error: {}", e);
                            let _ = err_tx.send(Err(ExecutionError::Interrupted(e.to_string())));
                        }
                    });

                    let stream = futures_util::stream::unfold(text_rx, |mut rx| async move {
                        rx.recv().await.map(|item| (item, rx))
                    });
                    return Ok(Box::pin(SyncStream(stream)));
                }
                Err(e) => {
                    warn!(
                        pid = %pid,
                        "CognitiveRouter failed ({}), falling back to legacy heuristic",
                        e
                    );
                }
            }
        }

        // Legacy heuristic fallback
        let driver_id = match model_pref {
            ModelPreference::LocalOnly => {
                #[cfg(not(feature = "local_llm"))]
                {
                    return Err(SystemError::HardwareFailure(
                        "Motor local no compilado. Reinicie con feature 'local_llm' o use Cloud."
                            .to_string(),
                    ));
                }
                #[cfg(feature = "local_llm")]
                {
                    info!(pid = %pid, "Policy: LOCAL_ONLY. Selecting local-driver.");
                    "local-driver"
                }
            }
            ModelPreference::CloudOnly => {
                info!(pid = %pid, "Policy: CLOUD_ONLY. Selecting cloud-driver.");
                "cloud-driver"
            }
            ModelPreference::HybridSmart => {
                let is_complex = priority > 8 || instruction.len() > 1000;
                let has_local_driver = self.drivers.read().await.contains_key("local-driver");
                if is_complex || !has_local_driver {
                    info!(
                        pid = %pid,
                        priority = priority,
                        has_local_driver = has_local_driver,
                        "HybridSmart: Routing to CLOUD (fallback or complex)."
                    );
                    "cloud-driver"
                } else {
                    info!(
                        pid = %pid,
                        priority = priority,
                        "HybridSmart: Low complexity and local driver available. Routing to LOCAL."
                    );
                    "local-driver"
                }
            }
        };

        let pcb_snapshot = shared_pcb.read().await.clone();

        let drivers = self.drivers.read().await;
        let driver = drivers.get(driver_id).ok_or_else(|| {
            if driver_id == "cloud-driver" {
                SystemError::HardwareFailure(
                    "Driver cloud no configurado o sin credenciales.".to_string(),
                )
            } else {
                SystemError::DriverOffline(driver_id.to_string())
            }
        })?;

        let messages = self.build_messages(&pcb_snapshot, persona.as_deref()).await;
        driver.generate_stream(messages, None, None).await
    }

    /// CORE-261: Bucle ReAct — tool call → resultado → LLM, hasta MAX_ITERATIONS.
    /// Envía tokens de texto al caller via `text_tx`; nunca reenvía `__TOOL_CALL__` tokens.
    async fn execute_with_decision(
        &self,
        decision: RoutingDecision,
        pcb: &PCB,
        pid: &str,
        persona: Option<&str>,
        text_tx: tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    ) -> Result<(), SystemError> {
        use crate::chal::drivers::CloudProxyDriver;

        tracing::info!(
            pid = %pid,
            model = %decision.model_id,
            provider = %decision.provider,
            "CognitiveRouter: routing to model (ReAct loop)"
        );

        let on_rate_limited = {
            let router_opt = self.router_ref.read().await.clone();
            let key_id = decision.key_id.clone();
            match (router_opt, key_id) {
                (Some(router), Some(kid)) => {
                    Some(Arc::new(move |until: chrono::DateTime<chrono::Utc>| {
                        let router = Arc::clone(&router);
                        let kid = kid.clone();
                        tokio::spawn(async move {
                            router.read().await.mark_key_rate_limited(&kid, until).await;
                        });
                    })
                        as Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>)
                }
                _ => None,
            }
        };

        let driver = CloudProxyDriver::new_with_callback(
            Arc::clone(&self.http_client),
            decision.api_url.clone(),
            decision.api_key.clone(),
            decision.model_id.clone(),
            decision.key_id.clone(),
            on_rate_limited,
        );

        let provider = ProviderKind::from_string(&decision.provider);
        let tools = {
            let defs = ToolRegistry::tools_for(&AgentRole::ChatAgent, &provider);
            if defs.is_empty() {
                None
            } else {
                Some(defs)
            }
        };

        let mut messages = self.build_messages(pcb, persona).await;

        const MAX_ITERATIONS: usize = 10;
        let mut finished = false;

        for _iteration in 0..MAX_ITERATIONS {
            let raw_stream = driver
                .generate_stream(messages.clone(), None, tools.clone())
                .await?;

            tokio::pin!(raw_stream);

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

            while let Some(token_result) = raw_stream.next().await {
                match token_result {
                    Ok(token) if token.starts_with("__TOOL_CALL__") => {
                        let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                        if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str) {
                            tool_calls.push(ToolCallRecord {
                                id: tc.id,
                                type_: "function".to_string(),
                                function: FunctionCallRecord {
                                    name: tc.name,
                                    arguments: tc.arguments.to_string(),
                                },
                            });
                        }
                    }
                    Ok(text) => {
                        assistant_text.push_str(&text);
                        let _ = text_tx.send(Ok(text));
                    }
                    Err(e) => {
                        let _ = text_tx.send(Err(e));
                    }
                }
            }

            if tool_calls.is_empty() {
                finished = true;
                break;
            }

            // Inyectar respuesta del asistente con tool calls al historial
            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: if assistant_text.is_empty() {
                    None
                } else {
                    Some(assistant_text.clone())
                },
                tool_calls: Some(tool_calls.clone()),
                ..Default::default()
            });

            // Ejecutar cada tool call e inyectar resultado
            for tc in &tool_calls {
                let result = self.execute_tool_call_internal(tc, pid, pcb).await;
                tracing::info!(
                    pid = %pid,
                    tool = %tc.function.name,
                    "ReAct: tool ejecutado, inyectando resultado al historial"
                );
                messages.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: Some(result),
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                    ..Default::default()
                });
            }
        }

        if !finished {
            let _ = text_tx.send(Ok(
                "Lo siento, alcancé el límite máximo de pasos internos al procesar tu solicitud. Por favor, reformulá tu pedido."
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// CORE-262: Bucle ReAct para agentes del árbol.
    /// Acepta mensajes ya construidos sin necesitar un PCB completo.
    pub async fn execute_agent_loop(
        self: Arc<Self>,
        decision: crate::router::RoutingDecision,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<serde_json::Value>>,
        text_tx: tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
        agent_id: uuid::Uuid,
    ) -> Result<(), SystemError> {
        use crate::chal::drivers::CloudProxyDriver;

        let on_rate_limited = {
            let router_opt = self.router_ref.read().await.clone();
            let key_id = decision.key_id.clone();
            match (router_opt, key_id) {
                (Some(router), Some(kid)) => {
                    Some(Arc::new(move |until: chrono::DateTime<chrono::Utc>| {
                        let router = Arc::clone(&router);
                        let kid = kid.clone();
                        tokio::spawn(async move {
                            router.read().await.mark_key_rate_limited(&kid, until).await;
                        });
                    })
                        as Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>)
                }
                _ => None,
            }
        };

        let driver = CloudProxyDriver::new_with_callback(
            Arc::clone(&self.http_client),
            decision.api_url.clone(),
            decision.api_key.clone(),
            decision.model_id.clone(),
            decision.key_id.clone(),
            on_rate_limited,
        );

        let mut messages = messages;
        const MAX_ITERATIONS: usize = 10;

        for _iteration in 0..MAX_ITERATIONS {
            let raw_stream = driver
                .generate_stream(messages.clone(), None, tools.clone())
                .await?;
            tokio::pin!(raw_stream);

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

            while let Some(token_result) = raw_stream.next().await {
                match token_result {
                    Ok(token) if token.starts_with("__TOOL_CALL__") => {
                        let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                        if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str) {
                            tool_calls.push(ToolCallRecord {
                                id: tc.id,
                                type_: "function".to_string(),
                                function: FunctionCallRecord {
                                    name: tc.name,
                                    arguments: tc.arguments.to_string(),
                                },
                            });
                        }
                    }
                    Ok(text) => {
                        assistant_text.push_str(&text);
                        let _ = text_tx.send(Ok(text));
                    }
                    Err(e) => {
                        let _ = text_tx.send(Err(e));
                    }
                }
            }

            if tool_calls.is_empty() {
                break;
            }

            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: if assistant_text.is_empty() {
                    None
                } else {
                    Some(assistant_text)
                },
                tool_calls: Some(tool_calls.clone()),
                ..Default::default()
            });

            let mut mock_pcb =
                crate::pcb::PCB::new(format!("agent_{}", agent_id), 5, String::new());
            mock_pcb.agent_id = Some(agent_id);

            for tc in &tool_calls {
                let result = self
                    .execute_tool_call_internal(tc, &agent_id.to_string(), &mock_pcb)
                    .await;
                messages.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: Some(result),
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                    ..Default::default()
                });
            }
        }

        Ok(())
    }

    // --- CORE-275: Filesystem helpers ---

    fn get_tenant_workspace(pcb: &PCB) -> std::path::PathBuf {
        let data_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
        std::path::Path::new(&data_dir)
            .join("users")
            .join(tenant)
            .join("workspace")
    }

    async fn get_approved_paths(pcb: &PCB) -> Vec<String> {
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let session_key = pcb.session_key.as_deref().unwrap_or("");
        match crate::enclave::TenantDB::open(tenant_id, session_key) {
            Ok(db) => db.get_approved_paths().unwrap_or_default(),
            Err(_) => vec![],
        }
    }

    fn resolve_path(
        workspace: &std::path::Path,
        input_path: &str,
        approved_paths: &[String],
    ) -> Result<std::path::PathBuf, String> {
        let candidate = if std::path::Path::new(input_path).is_absolute() {
            std::path::PathBuf::from(input_path)
        } else {
            workspace.join(input_path)
        };

        // Canonicalizar para resolver .. y symlinks.
        // Si el archivo no existe aún (write_file), canonicalizar el padre.
        let canonical = if candidate.exists() {
            candidate
                .canonicalize()
                .map_err(|e| format!("{{\"error\":\"invalid_path\",\"detail\":\"{}\"}}", e))?
        } else {
            let parent = candidate.parent().ok_or_else(|| {
                "{\"error\":\"invalid_path\",\"detail\":\"no parent\"}".to_string()
            })?;
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            canonical_parent.join(candidate.file_name().unwrap_or_default())
        };

        // Verificar confinamiento al workspace
        if canonical.starts_with(workspace) {
            return Ok(canonical);
        }

        // Path externo — verificar aprobaciones
        let canonical_str = canonical.to_string_lossy().to_string();
        let approved = approved_paths
            .iter()
            .any(|a| canonical_str.starts_with(a.as_str()));

        if approved {
            Ok(canonical)
        } else {
            Err(format!(
                "{{\"error\":\"path_requires_approval\",\"path\":\"{}\",\"message\":\"Este path está fuera del workspace. El usuario debe aprobarlo primero.\"}}",
                canonical_str
            ))
        }
    }

    /// CORE-261: Ejecuta un tool call interno y retorna el resultado como String.
    async fn execute_tool_call_internal(
        &self,
        tc: &ToolCallRecord,
        _pid: &str,
        pcb: &PCB,
    ) -> String {
        let args: serde_json::Value =
            serde_json::from_str(&tc.function.arguments).unwrap_or_default();

        match tc.function.name.as_str() {
            "spawn_agent" => {
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        if pcb.agent_id.is_some() {
                            return "{\"error\":\"spawn_agent solo aplica al Chat Agent (sin agent_id)\"}".to_string();
                        }
                        let name = args["name"].as_str().map(String::from);
                        let scope = args["scope"].as_str().unwrap_or("").to_string();
                        let project_name = name
                            .clone()
                            .unwrap_or_else(|| scope.chars().take(40).collect());
                        let task_type_str = args["task_type"].as_str().unwrap_or("planning");
                        let task_type = Some(crate::syscalls::parse_task_type(task_type_str));

                        match orchestrator
                            .create_project(
                                project_name.clone(),
                                scope,
                                task_type,
                                pcb.tenant_id.clone(),
                            )
                            .await
                        {
                            Ok(agent_id) => {
                                let task = pcb.memory_pointers.l1_instruction.clone();
                                if !task.is_empty() {
                                    if let Err(e) =
                                        orchestrator.dispatch(agent_id, task, vec![]).await
                                    {
                                        tracing::warn!(
                                            agent = %agent_id,
                                            "CORE-264: dispatch post-spawn falló: {}",
                                            e
                                        );
                                    } else {
                                        tracing::info!(
                                            agent = %agent_id,
                                            project = %project_name,
                                            "CORE-264: Dispatch automático enviado al supervisor recién creado."
                                        );
                                    }
                                }
                                format!(
                                    "{{\"status\":\"spawned\",\"project\":\"{}\",\"agent_id\":\"{}\"}}",
                                    project_name,
                                    agent_id
                                )
                            }
                            Err(e) => format!("{{\"error\":\"{}\"}}", e),
                        }
                    }
                }
            }

            "query_agent" => {
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        let caller_id = match pcb.agent_id {
                            Some(id) => id,
                            None => {
                                return "{\"error\":\"query_agent requiere agent_id en PCB\"}"
                                    .to_string()
                            }
                        };
                        let project = args["project"].as_str().unwrap_or("").to_string();
                        let question = args["question"].as_str().unwrap_or("").to_string();
                        let call =
                            crate::agents::message::AgentToolCall::Query { project, question };
                        match orchestrator.handle_tool_call(caller_id, call).await {
                            Ok(result) => result,
                            Err(e) => format!("{{\"error\":\"{}\"}}", e),
                        }
                    }
                }
            }

            "call_plugin" => {
                let plugin_name = args["plugin_name"].as_str().unwrap_or("").to_string();
                let plugin_args = args["args"]
                    .as_object()
                    .map(|o| serde_json::Value::Object(o.clone()).to_string())
                    .unwrap_or_else(|| "{}".to_string());
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                let pm = self.plugin_manager.read().await;
                match pm
                    .execute_plugin(
                        tenant_id,
                        &plugin_name,
                        &plugin_args,
                        pcb.session_key.as_deref(),
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            // CORE-263: supervisor pausa su ejecución esperando respuesta del usuario
            "ask_user" => {
                let question = args["question"].as_str().unwrap_or("").to_string();
                let context = args
                    .get("context")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let agent_uuid = match pcb.agent_id {
                    Some(id) => id,
                    None => return "{\"error\":\"ask_user requiere agent_id en PCB\"}".to_string(),
                };

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel::<String>();
                orchestrator.register_user_reply(agent_uuid, reply_tx).await;

                let project_name = {
                    let tree = orchestrator.tree.read().await;
                    tree.get(&agent_uuid)
                        .map(|n| n.project_id.clone())
                        .unwrap_or_else(|| "proyecto".to_string())
                };

                {
                    let mut tree = orchestrator.tree.write().await;
                    if let Some(node) = tree.get_mut(&agent_uuid) {
                        node.set_state(crate::agents::node::AgentState::WaitingUser);
                    }
                }

                tracing::info!(
                    agent = %agent_uuid,
                    question = %question,
                    "CORE-263: supervisor pausado esperando respuesta del usuario"
                );

                // CORE-268: notificar a la UI que el supervisor necesita respuesta
                orchestrator.emit_event(crate::agents::event::AgentEvent::SupervisorQuestion {
                    agent_id: agent_uuid,
                    project_name: project_name.clone(),
                    question: question.clone(),
                    context: context.clone(),
                    timestamp: chrono::Utc::now(),
                });

                match tokio::time::timeout(std::time::Duration::from_secs(600), reply_rx).await {
                    Ok(Ok(answer)) => {
                        {
                            let mut tree = orchestrator.tree.write().await;
                            if let Some(node) = tree.get_mut(&agent_uuid) {
                                node.set_state(crate::agents::node::AgentState::Running);
                            }
                        }
                        orchestrator.emit_event(
                            crate::agents::event::AgentEvent::SupervisorResumed {
                                agent_id: agent_uuid,
                            },
                        );
                        format!("{{\"user_answer\": {}}}", serde_json::json!(answer))
                    }
                    _ => {
                        {
                            let mut tree = orchestrator.tree.write().await;
                            if let Some(node) = tree.get_mut(&agent_uuid) {
                                node.set_state(crate::agents::node::AgentState::Running);
                            }
                        }
                        orchestrator.emit_event(
                            crate::agents::event::AgentEvent::SupervisorTimedOut {
                                agent_id: agent_uuid,
                                project_name,
                            },
                        );
                        "{\"user_answer\": null, \"reason\": \"timeout\"}".to_string()
                    }
                }
            }

            // CORE-276: supervisor aprueba un path externo tras recibir OK del usuario
            "approve_path" => {
                let path = args["path"].as_str().unwrap_or("").to_string();

                if path.is_empty() {
                    return "{\"error\":\"empty_path\"}".to_string();
                }

                if !std::path::Path::new(&path).exists() {
                    return format!("{{\"error\":\"path_not_found\",\"path\":\"{}\"}}", path);
                }

                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let session_key = pcb.session_key.as_deref().unwrap_or("");

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match db.add_approved_path(&path) {
                        Ok(_) => serde_json::json!({
                            "status": "approved",
                            "path": path
                        })
                        .to_string(),
                        Err(e) => {
                            format!("{{\"error\":\"persist_failed\",\"detail\":\"{}\"}}", e)
                        }
                    },
                    Err(e) => {
                        format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e)
                    }
                }
            }

            // CORE-277: búsqueda web para specialists
            "web_search" => {
                let query = args["query"].as_str().unwrap_or("").to_string();
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5)
                    .min(10) as usize;

                if query.is_empty() {
                    return "{\"error\":\"empty_query\"}".to_string();
                }

                let api_key = match std::env::var("BRAVE_SEARCH_API_KEY").ok() {
                    Some(k) if !k.is_empty() => k,
                    _ => {
                        return "{\"error\":\"web_search_not_configured\",\"detail\":\"BRAVE_SEARCH_API_KEY not set\"}".to_string();
                    }
                };

                match self
                    .http_client
                    .get("https://api.search.brave.com/res/v1/web/search")
                    .header("X-Subscription-Token", api_key)
                    .header("Accept", "application/json")
                    .query(&[("q", &query), ("count", &max_results.to_string())])
                    .send()
                    .await
                {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(data) => {
                            let empty = vec![];
                            let raw = data["web"]["results"].as_array().unwrap_or(&empty);
                            let results: Vec<serde_json::Value> = raw
                                .iter()
                                .take(max_results)
                                .map(|r| {
                                    serde_json::json!({
                                        "title": r["title"],
                                        "url": r["url"],
                                        "snippet": r["description"]
                                    })
                                })
                                .collect();
                            let count = results.len();
                            serde_json::json!({
                                "results": results,
                                "query": query,
                                "count": count
                            })
                            .to_string()
                        }
                        Err(e) => {
                            format!("{{\"error\":\"parse_failed\",\"detail\":\"{}\"}}", e)
                        }
                    },
                    Err(e) => format!("{{\"error\":\"search_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-263: Chat Agent entrega la respuesta del usuario al supervisor pausado
            "answer_supervisor" => {
                let agent_id_str = args["agent_id"].as_str().unwrap_or("");
                let answer = args["answer"].as_str().unwrap_or("").to_string();

                let agent_uuid = match agent_id_str.parse::<uuid::Uuid>() {
                    Ok(id) => id,
                    Err(_) => return "{\"error\":\"agent_id inválido\"}".to_string(),
                };

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        if orchestrator.answer_user_question(agent_uuid, answer).await {
                            "{\"status\":\"answer_delivered\"}".to_string()
                        } else {
                            "{\"status\":\"no_supervisor_waiting\"}".to_string()
                        }
                    }
                }
            }

            // CORE-275: Specialist filesystem tools
            "read_file" => {
                let input_path = args["path"].as_str().unwrap_or("").to_string();
                let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(200) as usize;

                let workspace = Self::get_tenant_workspace(pcb);
                let approved = Self::get_approved_paths(pcb).await;

                let resolved = match Self::resolve_path(&workspace, &input_path, &approved) {
                    Ok(p) => p,
                    Err(e) => return e,
                };

                match tokio::fs::read_to_string(&resolved).await {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().collect();
                        let total = lines.len();
                        let start = offset.min(total);
                        let end = (offset + length).min(total);
                        let slice = &lines[start..end];
                        serde_json::json!({
                            "content": slice.join("\n"),
                            "total_lines": total,
                            "offset": offset,
                            "returned_lines": slice.len()
                        })
                        .to_string()
                    }
                    Err(e) => format!("{{\"error\":\"read_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "write_file" => {
                use tokio::io::AsyncWriteExt;

                let input_path = args["path"].as_str().unwrap_or("").to_string();
                let content = args["content"].as_str().unwrap_or("").to_string();
                let mode = args
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rewrite");

                let workspace = Self::get_tenant_workspace(pcb);

                // write_file solo opera dentro del workspace — nunca en paths externos
                let resolved = match Self::resolve_path(&workspace, &input_path, &[]) {
                    Ok(p) => p,
                    Err(_) => {
                        return "{\"error\":\"write_outside_workspace\",\"message\":\"write_file solo puede escribir dentro del workspace del tenant.\"}".to_string();
                    }
                };

                // Crear directorios intermedios si es necesario
                if let Some(parent) = resolved.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return format!("{{\"error\":\"mkdir_failed\",\"detail\":\"{}\"}}", e);
                    }
                }

                let result = if mode == "append" {
                    match tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&resolved)
                        .await
                    {
                        Ok(mut f) => f.write_all(content.as_bytes()).await,
                        Err(e) => Err(e),
                    }
                } else {
                    tokio::fs::write(&resolved, content.as_bytes()).await
                };

                match result {
                    Ok(_) => serde_json::json!({
                        "status": "written",
                        "path": resolved.to_string_lossy()
                    })
                    .to_string(),
                    Err(e) => format!("{{\"error\":\"write_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "list_files" => {
                let input_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let depth = args
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2)
                    .min(4) as usize;

                let workspace = Self::get_tenant_workspace(pcb);
                let approved = Self::get_approved_paths(pcb).await;

                let resolved = match Self::resolve_path(&workspace, input_path, &approved) {
                    Ok(p) => p,
                    Err(e) => return e,
                };

                fn walk(dir: &std::path::Path, max_depth: usize, current: usize) -> Vec<String> {
                    if current > max_depth {
                        return vec![];
                    }
                    let mut entries = vec![];
                    if let Ok(rd) = std::fs::read_dir(dir) {
                        let mut items: Vec<_> = rd.flatten().collect();
                        items.sort_by_key(|e| e.file_name());
                        for entry in items {
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if name.starts_with('.') || name == "target" || name == "node_modules" {
                                continue;
                            }
                            if path.is_dir() {
                                entries.push(format!("[DIR]  {}", name));
                                for s in walk(&path, max_depth, current + 1) {
                                    entries.push(format!("  {}", s));
                                }
                            } else {
                                entries.push(format!("[FILE] {}", name));
                            }
                        }
                    }
                    entries
                }

                let listing = walk(&resolved, depth, 0);
                serde_json::json!({
                    "path": resolved.to_string_lossy(),
                    "entries": listing
                })
                .to_string()
            }

            // CORE-272: Chat Agent consulta el ledger de un proyecto por nombre
            "get_project_ledger" => {
                let project_name = args["project_name"].as_str().unwrap_or("").to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
                };

                let project_id = {
                    let tree = orchestrator.tree.read().await;
                    tree.all_nodes()
                        .iter()
                        .filter_map(|n| {
                            if let crate::agents::node::AgentRole::ProjectSupervisor {
                                name, ..
                            } = &n.role
                            {
                                if name.to_lowercase().contains(&project_name.to_lowercase()) {
                                    return Some(n.project_id.clone());
                                }
                            }
                            None
                        })
                        .next()
                };

                let project_id = match project_id {
                    Some(id) => id,
                    None => {
                        return format!(
                            "{{\"error\":\"project_not_found\",\"project\":\"{}\"}}",
                            project_name
                        )
                    }
                };

                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };

                let persistence = crate::agents::persistence::AgentPersistence::from_env();
                match persistence.load_ledger(&tenant_id, &project_id) {
                    Ok(Some(ledger)) => serde_json::to_string(&ledger)
                        .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string()),
                    Ok(None) => format!(
                        "{{\"error\":\"no_ledger\",\"project_id\":\"{}\"}}",
                        project_id
                    ),
                    Err(e) => format!("{{\"error\":\"load_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-273: Supervisores escriben al ledger del proyecto
            "add_ledger_entry" => {
                let content = args["content"].as_str().unwrap_or("").to_string();
                if content.is_empty() {
                    return "{\"error\":\"content is required\"}".to_string();
                }

                let agent_uuid = match pcb.agent_id {
                    Some(id) => id,
                    None => {
                        return "{\"error\":\"add_ledger_entry requiere agent_id en PCB\"}"
                            .to_string()
                    }
                };

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let tenant_id = pcb
                    .tenant_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string());

                match orchestrator
                    .add_project_ledger_entry(&tenant_id, agent_uuid, content)
                    .await
                {
                    Ok(_) => "{\"status\":\"recorded\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            other => format!("{{\"error\":\"Unknown tool: {}\"}}", other),
        }
    }

    /// Almacena un fragmento de texto en la base de datos neuronal (L3).
    pub async fn store_memory(&self, tenant_id: &str, text: &str) -> Result<String, SystemError> {
        let driver = self.embedding_driver.as_ref().ok_or_else(|| {
            SystemError::HardwareFailure("No embedding driver configured for memory storage".into())
        })?;

        let vector = driver.embed(text).await?;
        self.swap_manager
            .store_fragment(tenant_id, text, vector)
            .await
            .map_err(|e| SystemError::HardwareFailure(format!("Swap storage failed: {}", e)))
    }

    /// Detecta si un modelo Ollama soporta tool use y actualiza el catálogo (CORE-237).
    ///
    /// Lógica:
    /// - Si `tool_use_support == Unknown` → intentar con tools, observar resultado.
    /// - Si falla (error de driver) → marcar `Degraded`, reintentar sin tools.
    /// - Si ok → marcar `Supported`.
    ///
    /// Retorna `true` si el modelo soporta tools, `false` si está en modo degradado.
    pub async fn detect_ollama_tool_support(
        &self,
        model_id: &str,
        router: &crate::router::CognitiveRouter,
    ) -> bool {
        use crate::router::ToolUseSupport;

        // Verificar estado actual en catálogo
        let current_support = {
            let entry = router.catalog_find(model_id).await;
            entry
                .map(|e| e.tool_use_support)
                .unwrap_or(ToolUseSupport::Unknown)
        };

        match current_support {
            ToolUseSupport::Supported => true,
            ToolUseSupport::Degraded => false,
            ToolUseSupport::Unknown => {
                // Intentar con una llamada de prueba mínima
                let drivers = self.drivers.read().await;
                let driver_key = format!("ollama-{}", model_id);
                let driver = drivers
                    .get(&driver_key)
                    .or_else(|| drivers.get("local-driver"));

                if let Some(driver) = driver {
                    // Prompt de prueba — en producción la respuesta incluiría tool_calls
                    let test_messages = vec![ChatMessage {
                        role: ChatRole::User,
                        content: Some(
                            "[TOOL_USE_PROBE] Respond with a tool call if supported.".to_string(),
                        ),
                        ..Default::default()
                    }];
                    match driver.generate_stream(test_messages, None, None).await {
                        Ok(_) => {
                            // Respuesta exitosa → marcar Supported (en producción verificar tool_calls)
                            router
                                .update_tool_use_support(model_id, ToolUseSupport::Supported)
                                .await;
                            true
                        }
                        Err(_) => {
                            warn!(
                                model = %model_id,
                                "ollama model {} does not support tool use — degraded mode",
                                model_id
                            );
                            router
                                .update_tool_use_support(model_id, ToolUseSupport::Degraded)
                                .await;
                            false
                        }
                    }
                } else {
                    // No hay driver Ollama disponible → asumir Degraded
                    false
                }
            }
        }
    }

    /// Construye los mensajes para el LLM usando VCM para ensamblar contexto.
    pub async fn build_messages(&self, pcb: &PCB, persona: Option<&str>) -> Vec<ChatMessage> {
        // 1. Ensamblar contexto via VCM (L1 + L2 + L3)
        let assembled_context = self
            .vcm
            .assemble_context(
                pcb,
                &self.swap_manager,
                self.embedding_driver.as_deref(),
                4096,
            )
            .await
            .unwrap_or_else(|e| {
                warn!(
                    "VCM assembly failed: {}. Falling back to raw instruction.",
                    e
                );
                pcb.memory_pointers.l1_instruction.clone()
            });

        let tool_prompt = self
            .plugin_manager
            .read()
            .await
            .get_available_tools_prompt();
        let mcp_tool_prompt = self.mcp_registry.generate_system_prompt().await;

        let persona_section = match persona {
            Some(p) if !p.trim().is_empty() => PERSONA_SECTION_TEMPLATE.replace("{persona}", p),
            _ => String::new(),
        };

        // CORE-148: Music instructions are only injected if the music_search plugin is active.
        let has_music_plugin = self
            .plugin_manager
            .read()
            .await
            .is_plugin_active("music_search");

        let music_section = if has_music_plugin {
            "\n\nMÚSICA — INSTRUCCIONES:\
             \n- Para reproducir: [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista canción>\", \"max_results\": 1})] y luego [MUSIC_PLAY:youtube:<video_id>] (o [MUSIC_PLAY:spotify:<track_id>] si usas Spotify)\
             \n- Para pausar: responde brevemente y termina con [MUSIC_PAUSE]\
             \n- Para continuar: responde brevemente y termina con [MUSIC_RESUME]\
             \n- Para detener: responde brevemente y termina con [MUSIC_STOP]\
             \n- Para cambiar volumen: termina con [MUSIC_VOLUME:<0-100>]\
             \n- Para cambiar canción: haz una nueva búsqueda y usa [MUSIC_PLAY:youtube:<nuevo_video_id>]\
             \nNunca expliques estos tags al usuario. Solo úsalos.\n"
        } else {
            ""
        };

        // CORE-226: Si el PCB es del Chat Agent (sin agent_id asignado), cargar chat_agent.md.
        // Si tiene agent_id, es un agente del árbol — el AgentOrchestrator ya maneja sus instrucciones.
        let (role_instructions, instruction_source) = if pcb.agent_id.is_none() {
            let instructions = self
                .instruction_loader
                .write()
                .await
                .instructions_for(&AgentRole::ChatAgent);
            (instructions, "chat_agent.md")
        } else {
            (SYSTEM_PROMPT_MASTER.to_string(), "SYSTEM_PROMPT_MASTER")
        };

        info!(
            pid = %pcb.pid,
            source = instruction_source,
            chars = role_instructions.len(),
            "build_messages: role instructions loaded"
        );

        let has_maker_plugin = self.plugin_manager.read().await.is_plugin_active("maker");

        let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
            MAKER_INSTRUCTIONS
        } else {
            ""
        };

        let system_content = if tool_prompt.trim().is_empty() && mcp_tool_prompt.trim().is_empty() {
            format!(
                "{}{}{}{}{}",
                maker_section,
                SPAWN_INSTRUCTIONS,
                role_instructions,
                persona_section,
                music_section
            )
        } else {
            format!(
                "{}{}{}{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}\n{}",
                maker_section,
                SPAWN_INSTRUCTIONS,
                role_instructions,
                persona_section,
                music_section,
                tool_prompt,
                mcp_tool_prompt
            )
        };

        let mut messages = vec![ChatMessage {
            role: ChatRole::System,
            content: Some(system_content),
            ..Default::default()
        }];

        // CORE-260: Historial previo de la sesión
        messages.extend(pcb.message_history.clone());

        messages.push(ChatMessage {
            role: ChatRole::User,
            content: Some(assembled_context),
            ..Default::default()
        });

        messages
    }
}

/// --- DUMMY DRIVER FOR TESTING ---
pub struct DummyDriver {
    pub name: String,
}

#[async_trait]
impl InferenceDriver for DummyDriver {
    async fn generate_stream(
        &self,
        _messages: Vec<ChatMessage>,
        _grammar: Option<Grammar>,
        _tools: Option<Vec<serde_json::Value>>,
    ) -> GenerateStreamResult {
        let response = format!("[{}] OK", self.name);
        let stream = tokio_stream::iter(vec![Ok(response)]);
        Ok(Box::pin(stream))
    }

    async fn get_health_status(&self) -> DriverStatus {
        DriverStatus {
            is_ready: true,
            ..Default::default()
        }
    }

    async fn load_model(&mut self, _id: &str) -> Result<(), SystemError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use crate::scheduler::ModelPreference;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_hybrid_smart_routing_high_priority() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = Arc::new(CognitiveHAL::new(pm)?);
        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        )
        .await;
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        )
        .await;
        let mut pcb = PCB::mock("Complex Mission", 10);
        pcb.model_pref = ModelPreference::HybridSmart;
        let shared_pcb = Arc::new(RwLock::new(pcb));
        let stream_res = hal.route_and_execute(shared_pcb, None).await?;
        let tokens: Vec<Result<String, ExecutionError>> = stream_res.collect().await;
        assert_eq!(tokens.len(), 1);
        let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
        assert!(
            response.contains("[cloud]"),
            "Debe haber seleccionado el driver cloud por alta prioridad"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_smart_routing_low_priority() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = Arc::new(CognitiveHAL::new(pm)?);
        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        )
        .await;
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        )
        .await;
        let mut pcb = PCB::mock("Simple task", 5);
        pcb.model_pref = ModelPreference::HybridSmart;
        let shared_pcb = Arc::new(RwLock::new(pcb));
        let stream_res = hal.route_and_execute(shared_pcb, None).await?;
        let tokens: Vec<Result<String, ExecutionError>> = stream_res.collect().await;
        let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
        assert!(
            response.contains("[local]"),
            "Debe haber seleccionado el driver local por baja prioridad"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_build_messages_default_tools_presence() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test".into(), 5, "hola".into());
        let messages = hal.build_messages(&pcb, None).await;
        let system_msg = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .unwrap()
            .content
            .as_ref()
            .unwrap();
        let user_msg = messages
            .iter()
            .find(|m| m.role == ChatRole::User)
            .unwrap()
            .content
            .as_ref()
            .unwrap();

        assert!(
            !system_msg.contains("[USER_PROCESS_INSTRUCTION]"),
            "El prompt no debe contener el tag USER_PROCESS_INSTRUCTION"
        );
        assert!(
            system_msg.contains("HERRAMIENTAS (PLUGINS) DISPONIBLES:")
                || system_msg.contains("HERRAMIENTAS DISPONIBLES:"),
            "Deben aparecer los plugins de dominio por defecto"
        );
        assert!(
            system_msg.contains("ledger") && system_msg.contains("chronos"),
            "Debe contener las herramientas ledger y chronos"
        );
        assert!(
            user_msg.contains("hola"),
            "El mensaje de usuario debe contener la instrucción"
        );
        // CORE-148: Music instructions are only injected if plugin is active.
        assert!(
            !system_msg.contains("MÚSICA"),
            "Music instructions must NOT be present if plugin is not active (CORE-148)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_build_messages_with_persona() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test".into(), 5, "hola".into());
        let messages = hal
            .build_messages(&pcb, Some("Eres Eve, asistente de ACME Corp."))
            .await;
        let system_msg = messages
            .iter()
            .find(|m| m.role == ChatRole::System)
            .unwrap()
            .content
            .as_ref()
            .unwrap();
        let user_msg = messages
            .iter()
            .find(|m| m.role == ChatRole::User)
            .unwrap()
            .content
            .as_ref()
            .unwrap();

        assert!(
            system_msg.contains("Eve"),
            "El mensaje de sistema debe contener la persona"
        );
        assert!(
            user_msg.contains("hola"),
            "El mensaje de usuario debe contener la instrucción"
        );
        Ok(())
    }
}
