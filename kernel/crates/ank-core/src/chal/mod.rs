#![allow(unused_assignments)]
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
use regex::Regex;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

pub mod autocorrect;
pub mod drivers;
mod failover;
pub mod hardware;
mod prompt_helpers;
mod sanitizer;
#[cfg(test)]
mod tests;
mod tool_handlers;

pub(crate) use failover::summarize_configured_providers;
use failover::{
    collect_rendered_video_card_ids, consume_recovery_stream, format_model_error_meta_token,
    is_casual_chat_turn, is_model_scoped_failure, is_permanent_model_failure,
    summarise_provider_failure,
};
use sanitizer::{emit_stream_text_chunk, StreamTextSanitizer};
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

// CORE-282: Interceptar token legacy [SYS_AGENT_SPAWN(...)] emitido por modelos sin tool use
// (e.g. openrouter/free). Captura: 1=role, 2=name, 3=scope.
static LEGACY_SPAWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_AGENT_SPAWN\(role="([^"]+)",\s*name="([^"]+)",\s*scope="([^"]+)"\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: legacy spawn regex invalid"))
});

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
    /// Gestor de WhatsApp Web (Baileys sidecar + rate limiter + confirmación).
    pub whatsapp: RwLock<Option<Arc<crate::whatsapp::WhatsAppManager>>>,
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
            // CORE-378: Fallback nativo a LocalEmbeddingDriver determinista y local
            Some(Arc::new(
                crate::chal::drivers::embeddings::LocalEmbeddingDriver::default(),
            ))
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
            whatsapp: RwLock::new(None),
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

    pub async fn set_whatsapp(&self, manager: Arc<crate::whatsapp::WhatsAppManager>) {
        let mut w = self.whatsapp.write().await;
        *w = Some(manager);
    }

    /// CORE-325: single point for feeding a model failure into the router's
    /// tracker. Every branch of the fallback walk (primary call, key
    /// rotation, fallback chain — HTTP-error and retry variants) used to
    /// inline this block with small inconsistencies; one helper keeps the
    /// signals uniform: scoring window + provider circuit always, and the
    /// per-model circuit on deterministic failures (401 / tier-0 quota,
    /// see `is_permanent_model_failure`).
    async fn note_model_failure(
        &self,
        model_id: &str,
        provider: &str,
        task: crate::pcb::TaskType,
        err: Option<&SystemError>,
    ) {
        if let Some(r) = self.router_ref.read().await.clone() {
            let router = r.read().await;
            let tracker = router.tracker_ref();
            // Record 402/404/subscription/limit:0 at model scope so healthy sibling
            // models and keys on the same provider stay routable.
            if err.is_some_and(is_model_scoped_failure) {
                tracker.record_model_failure_for_task(model_id, task).await;
            } else {
                tracker
                    .record_failure_for_task(model_id, provider, task)
                    .await;
            }
            if let Some(e) = err {
                if is_permanent_model_failure(e) {
                    tracker.record_model_unavailable(model_id).await;
                }
            }
        }
    }

    /// CORE-325: a 200-OK stream with zero content tokens is an implicit
    /// failure — feed both the per-model empty-response circuit and the
    /// model-scoped task failure window without poisoning sibling models on the provider.
    async fn note_empty_response(
        &self,
        model_id: &str,
        _provider: &str,
        task: crate::pcb::TaskType,
    ) {
        if let Some(r) = self.router_ref.read().await.clone() {
            let router = r.read().await;
            let tracker = router.tracker_ref();
            tracker.record_empty_response(model_id).await;
            tracker.record_model_failure_for_task(model_id, task).await;
        }
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

    async fn cooldown_and_invalidate_sticky(&self, providers: &[&str], tenant_id: &str) -> u64 {
        let mut max_cooldown: u64 = 0;
        if let Some(r) = self.router_ref.read().await.clone() {
            let tr = r.read().await;
            let tracker = tr.tracker_ref();
            for prov in providers {
                if let Some(s) = tracker.provider_cooldown_remaining_secs(prov).await {
                    max_cooldown = max_cooldown.max(s);
                }
            }
            tr.invalidate_sticky(tenant_id).await;
        }
        max_cooldown
    }

    fn effective_retry_after_secs(max_cooldown: u64, err_text: &str) -> u64 {
        if max_cooldown > 0 {
            return max_cooldown;
        }
        let lc = err_text.to_lowercase();
        if lc.contains("429")
            || lc.contains("rate")
            || lc.contains("quota")
            || lc.contains("satur")
            || lc.contains("timed out")
            || lc.contains("timeout")
            || lc.contains("502")
            || lc.contains("503")
        {
            15
        } else {
            0
        }
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

        let router_opt = self.router_ref.read().await.clone();
        let make_rate_limited_cb = |key_id: Option<String>| match (router_opt.clone(), key_id) {
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
        };

        // Mutable so that B1 (fallback chain) can swap to a backup model on
        // first-call failure without rebuilding the whole closure.
        let mut active_model_id = decision.model_id.clone();
        let mut active_provider = decision.provider.clone();
        let mut driver = CloudProxyDriver::new_with_callback(
            Arc::clone(&self.http_client),
            decision.api_url.clone(),
            decision.api_key.clone(),
            decision.model_id.clone(),
            decision.key_id.clone(),
            make_rate_limited_cb(decision.key_id.clone()),
        );

        let casual_turn = is_casual_chat_turn(pcb);
        // CORE-377: Cachear herramientas dinámicas descubiertas vía MCP para poder reconstruir
        // el esquema si el router hace failover hacia otro proveedor en medio del turno.
        let cached_mcp_tools = self.mcp_registry.list_tools().await;
        let module_tenant = pcb.tenant_id.as_deref().unwrap_or("default").to_string();
        let module_skey_opt = Self::valid_pcb_session_key(pcb).map(ToString::to_string);

        let is_video_req =
            Self::has_video_context(&pcb.memory_pointers.l1_instruction, &pcb.message_history);
        let load_extra_tools = || async {
            let mut list: Vec<crate::pcb::ToolDefinition> = cached_mcp_tools
                .iter()
                .map(|m| {
                    crate::pcb::ToolDefinition::new(
                        m.name.clone(),
                        m.description.clone(),
                        m.input_schema.clone(),
                    )
                })
                .collect();
            if let Some(ref module_skey) = module_skey_opt {
                if is_video_req {
                    if let Ok(db) = crate::enclave::TenantDB::open(&module_tenant, module_skey) {
                        if db
                            .get_kv("module_active:aegis.domain.video")
                            .unwrap_or(None)
                            .as_deref()
                            != Some("true")
                        {
                            let _ = db.set_kv("module_active:aegis.domain.video", "true");
                        }
                    }
                }
                if let Some(r) = self.router_ref.read().await.clone() {
                    let router_guard = r.read().await;
                    let mod_tools = router_guard
                        .get_enabled_module_tools(&module_tenant, module_skey)
                        .await;
                    list.extend(mod_tools);
                }
            }
            list
        };

        let mut extra_tools = if casual_turn {
            Vec::new()
        } else {
            load_extra_tools().await
        };

        let build_tools_for = |prov_str: &str,
                               dyn_tools: &[crate::pcb::ToolDefinition]|
         -> Option<Vec<serde_json::Value>> {
            if casual_turn {
                return None;
            }
            let prov_kind = ProviderKind::from_string(prov_str);
            let mut list = ToolRegistry::tools_for(&AgentRole::ChatAgent, &prov_kind);
            for dyn_tool in dyn_tools {
                list.push(ToolRegistry::serialize_canonical(dyn_tool, &prov_kind));
            }
            if list.is_empty() {
                None
            } else {
                Some(list)
            }
        };
        let mut active_key_id = decision.key_id.clone();
        let mut tools = build_tools_for(&decision.provider, &extra_tools);

        let (mut messages, build_warnings) = self
            .build_messages_for_chat_with_warnings(pcb, persona)
            .await;

        // CORE-FIX (A2): tell the client which model is about to answer so
        // the UI can render a "Claude Sonnet 4.6" / "GPT-4o" badge. The token
        // uses the same `__PREFIX__` convention as `__TOOL_CALL__` and is
        // intercepted by the WebSocket handler before reaching the user-visible
        // stream.
        let send_model_event =
            |tx: &tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
             model_id: &str,
             provider_str: &str| {
                let payload = serde_json::json!({
                    "model_id": model_id,
                    "provider": provider_str,
                });
                let _ = tx.send(Ok(format!("__MODEL_SELECTED__{}", payload)));
            };
        send_model_event(&text_tx, &decision.model_id, &decision.provider);

        // CORE-FIX (A3): surface any non-fatal warnings produced while building
        // the context (e.g. VCM assembly failure). The WS handler renders these
        // as a `warning` event so the user knows the response has reduced context.
        for w in build_warnings {
            let payload = serde_json::json!({ "category": "context_assembly", "message": w });
            let _ = text_tx.send(Ok(format!("__WARNING__{}", payload)));
        }

        const MAX_ITERATIONS: usize = 10;
        let mut finished = false;
        let mut last_assistant_text = String::new();
        let primary_model_id = decision.model_id.clone();
        let mut tried_models: std::collections::HashSet<String> = std::collections::HashSet::new();
        tried_models.insert(decision.model_id.clone());
        let mut pending_fallback_warning: Option<(String, String, &'static str, String)> = None;
        let mut has_streamed_text = false;
        let mut last_streamed_ends_newline = false;
        let mut rendered_video_ids: Vec<String> = Vec::new();

        for iteration in 0..MAX_ITERATIONS {
            let mut autocorrect_retries = 0;
            let mut local_messages = messages.clone();

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();
            let mut needs_paragraph_break = has_streamed_text && !last_streamed_ends_newline;

            loop {
                // CORE-FIX (B1): on the FIRST iteration, if the primary model
                // refuses the request (rate-limit, 5xx, timeout), try the fallback
                // chain before bubbling the error up to the user. After the first
                // iteration we don't switch models — the conversation has tool
                // history committed to the original model and changing mid-thread
                // would confuse it.
                let raw_stream = if iteration == 0 && autocorrect_retries == 0 {
                    match driver
                        .generate_stream(local_messages.clone(), None, tools.clone())
                        .await
                    {
                        Ok(s) => s,
                        Err(primary_err) => {
                            let tracker = self.router_ref.read().await.clone();
                            self.note_model_failure(
                                &active_model_id,
                                &active_provider,
                                pcb.task_type,
                                Some(&primary_err),
                            )
                            .await;
                            warn!(
                                pid = %pid,
                                primary = %active_model_id,
                                error = %primary_err,
                                "primary model failed on first call — trying key rotation then fallback chain"
                            );

                            let mut recovered: Option<_> = None;

                            // CORE-FIX: rotate to another key for the SAME (provider,
                            // model) before switching to a different model. The
                            // 429-rate-limited callback inside the cloud driver has
                            // already marked the failing key, so the next call to
                            // `get_available_key` will give us a different one if the
                            // user configured multiple keys (e.g. several Gemini API
                            // keys). This is the difference between "your Gemini
                            // quota ran out, here's a Groq response" and "your Gemini
                            // quota ran out, here's a response from your other
                            // Gemini key".
                            let err_lc = primary_err.to_string().to_lowercase();
                            let skip_key_rotation = is_model_scoped_failure(&primary_err)
                                || err_lc.contains("timed out")
                                || err_lc.contains("error sending request");
                            if let (false, Some(r)) = (skip_key_rotation, &tracker) {
                                let kp = r.read().await.key_pool_ref();
                                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                let mut tried: std::collections::HashSet<String> =
                                    std::collections::HashSet::new();
                                if let Some(ref kid) = decision.key_id {
                                    tried.insert(kid.clone());
                                }
                                for attempt in 0..2u32 {
                                    // CORE-362: Use get_available_key_excluding so we never
                                    // re-pick a key already in `tried` even if the spawned
                                    // mark_key_rate_limited task has not acquired the lock yet.
                                    let alt = match kp
                                        .get_available_key_excluding(
                                            &active_provider,
                                            &active_model_id,
                                            tenant_id,
                                            &tried,
                                        )
                                        .await
                                    {
                                        Some(k) => k,
                                        None => break,
                                    };
                                    tried.insert(alt.key_id.clone());
                                    let alt_url = alt
                                        .api_url
                                        .clone()
                                        .unwrap_or_else(|| decision.api_url.clone());
                                    let alt_driver = CloudProxyDriver::new_with_callback(
                                        Arc::clone(&self.http_client),
                                        alt_url,
                                        alt.api_key.clone(),
                                        active_model_id.clone(),
                                        Some(alt.key_id.clone()),
                                        make_rate_limited_cb(Some(alt.key_id.clone())),
                                    );
                                    match alt_driver
                                        .generate_stream(
                                            local_messages.clone(),
                                            None,
                                            tools.clone(),
                                        )
                                        .await
                                    {
                                        Ok(s) => {
                                            pending_fallback_warning = Some((
                                                active_model_id.clone(),
                                                active_provider.clone(),
                                                "key_rotated",
                                                format!(
                                                    "La key primaria de {} se agotó; \
                                                     cambié a otra del mismo proveedor.",
                                                    active_provider
                                                ),
                                            ));
                                            driver = alt_driver;
                                            active_key_id = Some(alt.key_id.clone());
                                            recovered = Some(s);
                                            break;
                                        }
                                        Err(e) => {
                                            self.note_model_failure(
                                                &active_model_id,
                                                &active_provider,
                                                pcb.task_type,
                                                Some(&e),
                                            )
                                            .await;
                                            warn!(
                                                pid = %pid,
                                                provider = %active_provider,
                                                key_id = %alt.key_id,
                                                attempt = attempt + 1,
                                                error = %e,
                                                "alternate key also failed; trying next"
                                            );
                                        }
                                    }
                                }
                            }

                            for fb in &decision.fallback_chain {
                                if recovered.is_some() {
                                    break;
                                }
                                if !tried_models.insert(fb.model_id.clone()) {
                                    continue;
                                }
                                let fb_driver = CloudProxyDriver::new_with_callback(
                                    Arc::clone(&self.http_client),
                                    fb.api_url.clone(),
                                    fb.api_key.clone(),
                                    fb.model_id.clone(),
                                    fb.key_id.clone(),
                                    make_rate_limited_cb(fb.key_id.clone()),
                                );
                                let fb_tools = build_tools_for(&fb.provider, &extra_tools);
                                match fb_driver
                                    .generate_stream(local_messages.clone(), None, fb_tools.clone())
                                    .await
                                {
                                    Ok(s) => {
                                        active_model_id = fb.model_id.clone();
                                        active_provider = fb.provider.clone();
                                        active_key_id = fb.key_id.clone();
                                        tools = fb_tools;
                                        driver = fb_driver;
                                        pending_fallback_warning = Some((
                                            fb.model_id.clone(),
                                            fb.provider.clone(),
                                            "model_fallback",
                                            format!(
                                                "El modelo primario ({}) falló; respondiendo con {}.",
                                                primary_model_id, fb.model_id
                                            ),
                                        ));
                                        recovered = Some(s);
                                        break;
                                    }
                                    Err(e) => {
                                        self.note_model_failure(
                                            &fb.model_id,
                                            &fb.provider,
                                            pcb.task_type,
                                            Some(&e),
                                        )
                                        .await;
                                        warn!(
                                            pid = %pid,
                                            fallback = %fb.model_id,
                                            error = %e,
                                            "fallback model also failed"
                                        );
                                    }
                                }
                            }

                            if recovered.is_none() {
                                if let Some(r) = &tracker {
                                    for _hop in 0..3u32 {
                                        let dyn_dec = match r
                                            .read()
                                            .await
                                            .decide_excluding(pcb, &tried_models)
                                            .await
                                        {
                                            Ok(d) => d,
                                            Err(_) => break,
                                        };
                                        if !tried_models.insert(dyn_dec.model_id.clone()) {
                                            break;
                                        }
                                        let dyn_driver = CloudProxyDriver::new_with_callback(
                                            Arc::clone(&self.http_client),
                                            dyn_dec.api_url.clone(),
                                            dyn_dec.api_key.clone(),
                                            dyn_dec.model_id.clone(),
                                            dyn_dec.key_id.clone(),
                                            make_rate_limited_cb(dyn_dec.key_id.clone()),
                                        );
                                        let dyn_tools =
                                            build_tools_for(&dyn_dec.provider, &extra_tools);
                                        match dyn_driver
                                            .generate_stream(
                                                local_messages.clone(),
                                                None,
                                                dyn_tools.clone(),
                                            )
                                            .await
                                        {
                                            Ok(s) => {
                                                active_model_id = dyn_dec.model_id.clone();
                                                active_provider = dyn_dec.provider.clone();
                                                active_key_id = dyn_dec.key_id.clone();
                                                tools = dyn_tools;
                                                driver = dyn_driver;
                                                pending_fallback_warning = Some((
                                                    dyn_dec.model_id.clone(),
                                                    dyn_dec.provider.clone(),
                                                    "model_fallback",
                                                    format!(
                                                        "El modelo primario ({}) falló; respondiendo con {}.",
                                                        primary_model_id, dyn_dec.model_id
                                                    ),
                                                ));
                                                recovered = Some(s);
                                                break;
                                            }
                                            Err(dyn_err) => {
                                                self.note_model_failure(
                                                    &dyn_dec.model_id,
                                                    &dyn_dec.provider,
                                                    pcb.task_type,
                                                    Some(&dyn_err),
                                                )
                                                .await;
                                                warn!(
                                                    pid = %pid,
                                                    fallback = %dyn_dec.model_id,
                                                    error = %dyn_err,
                                                    "dynamic router fallback also failed"
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            match recovered {
                                Some(s) => s,
                                None => {
                                    let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                    let mut providers: Vec<&str> = vec![active_provider.as_str()];
                                    for fb in &decision.fallback_chain {
                                        providers.push(fb.provider.as_str());
                                    }
                                    let max_cooldown = self
                                        .cooldown_and_invalidate_sticky(&providers, tenant_id)
                                        .await;
                                    let err_str = primary_err.to_string();
                                    let retry_after =
                                        Self::effective_retry_after_secs(max_cooldown, &err_str);
                                    let summary = summarise_provider_failure(&primary_err);
                                    let attempted: Vec<String> = decision
                                        .fallback_chain
                                        .iter()
                                        .map(|f| f.model_id.clone())
                                        .collect();
                                    let msg = format!(
                                        "No tengo modelos disponibles ahora. \
                                         Probamos '{}' y los respaldos ({}) sin \
                                         éxito. {} Reintentá en unos segundos o \
                                         configurá otro proveedor.",
                                        primary_model_id,
                                        if attempted.is_empty() {
                                            "sin respaldos".to_string()
                                        } else {
                                            attempted.join(", ")
                                        },
                                        summary
                                    );
                                    if !has_streamed_text {
                                        let _ = text_tx.send(Ok(format_model_error_meta_token(
                                            &msg,
                                            retry_after,
                                        )));
                                    } else {
                                        let _ = text_tx.send(Ok(msg));
                                    }
                                    return Err(primary_err);
                                }
                            }
                        }
                    }
                } else {
                    // Subsequent retries of the autocorrect loop: call driver directly,
                    // recovering via alternate keys or fallback_chain if the provider fails mid-turn.
                    match driver
                        .generate_stream(local_messages.clone(), None, tools.clone())
                        .await
                    {
                        Ok(s) => s,
                        Err(e) => {
                            self.note_model_failure(
                                &active_model_id,
                                &active_provider,
                                pcb.task_type,
                                Some(&e),
                            )
                            .await;

                            let err_lc = e.to_string().to_lowercase();
                            let skip_keys = is_model_scoped_failure(&e)
                                || err_lc.contains("timed out")
                                || err_lc.contains("error sending request");
                            let mut recovered_stream = None;

                            if !skip_keys {
                                if let Some(r) = self.router_ref.read().await.clone() {
                                    let kp = r.read().await.key_pool_ref();
                                    let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                    let mut tried: std::collections::HashSet<String> =
                                        std::collections::HashSet::new();
                                    if let Some(kid) = driver.current_key_id() {
                                        tried.insert(kid.to_string());
                                    }
                                    for _attempt in 0..2u32 {
                                        let Some(alt) = kp
                                            .get_available_key_excluding(
                                                &active_provider,
                                                &active_model_id,
                                                tenant_id,
                                                &tried,
                                            )
                                            .await
                                        else {
                                            break;
                                        };
                                        tried.insert(alt.key_id.clone());
                                        driver.swap_api_key(
                                            alt.api_key.clone(),
                                            Some(alt.key_id.clone()),
                                            make_rate_limited_cb(Some(alt.key_id.clone())),
                                        );
                                        match driver
                                            .generate_stream(
                                                local_messages.clone(),
                                                None,
                                                tools.clone(),
                                            )
                                            .await
                                        {
                                            Ok(s) => {
                                                tracing::info!(
                                                    pid = %pid,
                                                    provider = %active_provider,
                                                    model = %active_model_id,
                                                    key_id = %alt.key_id,
                                                    "ReAct continuation recovered via alternate key"
                                                );
                                                active_key_id = Some(alt.key_id.clone());
                                                recovered_stream = Some(s);
                                                break;
                                            }
                                            Err(alt_err) => {
                                                self.note_model_failure(
                                                    &active_model_id,
                                                    &active_provider,
                                                    pcb.task_type,
                                                    Some(&alt_err),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                }
                            }

                            if recovered_stream.is_none() {
                                for fb in &decision.fallback_chain {
                                    if !tried_models.insert(fb.model_id.clone()) {
                                        continue;
                                    }
                                    let fb_driver = CloudProxyDriver::new_with_callback(
                                        Arc::clone(&self.http_client),
                                        fb.api_url.clone(),
                                        fb.api_key.clone(),
                                        fb.model_id.clone(),
                                        fb.key_id.clone(),
                                        make_rate_limited_cb(fb.key_id.clone()),
                                    );
                                    let fb_tools = build_tools_for(&fb.provider, &extra_tools);
                                    match fb_driver
                                        .generate_stream(
                                            local_messages.clone(),
                                            None,
                                            fb_tools.clone(),
                                        )
                                        .await
                                    {
                                        Ok(s) => {
                                            let prev = active_model_id.clone();
                                            let reason = summarise_provider_failure(&e);
                                            active_model_id = fb.model_id.clone();
                                            active_provider = fb.provider.clone();
                                            active_key_id = fb.key_id.clone();
                                            tools = fb_tools;
                                            driver = fb_driver;
                                            pending_fallback_warning = Some((
                                                fb.model_id.clone(),
                                                fb.provider.clone(),
                                                "model_fallback",
                                                format!(
                                                    "El modelo ({}) falló ({}); respondiendo con {}.",
                                                    prev, reason, fb.model_id
                                                ),
                                            ));
                                            recovered_stream = Some(s);
                                            break;
                                        }
                                        Err(fb_err) => {
                                            self.note_model_failure(
                                                &fb.model_id,
                                                &fb.provider,
                                                pcb.task_type,
                                                Some(&fb_err),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }

                            if recovered_stream.is_none() {
                                if let Some(r) = self.router_ref.read().await.clone() {
                                    for _hop in 0..3u32 {
                                        let dyn_dec = match r
                                            .read()
                                            .await
                                            .decide_excluding(pcb, &tried_models)
                                            .await
                                        {
                                            Ok(d) => d,
                                            Err(_) => break,
                                        };
                                        if !tried_models.insert(dyn_dec.model_id.clone()) {
                                            break;
                                        }
                                        let dyn_driver = CloudProxyDriver::new_with_callback(
                                            Arc::clone(&self.http_client),
                                            dyn_dec.api_url.clone(),
                                            dyn_dec.api_key.clone(),
                                            dyn_dec.model_id.clone(),
                                            dyn_dec.key_id.clone(),
                                            make_rate_limited_cb(dyn_dec.key_id.clone()),
                                        );
                                        let dyn_tools =
                                            build_tools_for(&dyn_dec.provider, &extra_tools);
                                        match dyn_driver
                                            .generate_stream(
                                                local_messages.clone(),
                                                None,
                                                dyn_tools.clone(),
                                            )
                                            .await
                                        {
                                            Ok(s) => {
                                                let prev = active_model_id.clone();
                                                let reason = summarise_provider_failure(&e);
                                                active_model_id = dyn_dec.model_id.clone();
                                                active_provider = dyn_dec.provider.clone();
                                                active_key_id = dyn_dec.key_id.clone();
                                                tools = dyn_tools;
                                                driver = dyn_driver;
                                                pending_fallback_warning = Some((
                                                    dyn_dec.model_id.clone(),
                                                    dyn_dec.provider.clone(),
                                                    "model_fallback",
                                                    format!(
                                                        "El modelo ({}) falló ({}); respondiendo con {}.",
                                                        prev, reason, dyn_dec.model_id
                                                    ),
                                                ));
                                                recovered_stream = Some(s);
                                                break;
                                            }
                                            Err(dyn_err) => {
                                                self.note_model_failure(
                                                    &dyn_dec.model_id,
                                                    &dyn_dec.provider,
                                                    pcb.task_type,
                                                    Some(&dyn_err),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(s) = recovered_stream {
                                s
                            } else {
                                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                let mut providers: Vec<&str> = vec![active_provider.as_str()];
                                for fb in &decision.fallback_chain {
                                    providers.push(fb.provider.as_str());
                                }
                                let max_cooldown = self
                                    .cooldown_and_invalidate_sticky(&providers, tenant_id)
                                    .await;
                                let err_str = e.to_string();
                                let retry_after =
                                    Self::effective_retry_after_secs(max_cooldown, &err_str);
                                let summary = summarise_provider_failure(&e);
                                let msg = format!(
                                    "No pude completar la respuesta tras ejecutar la herramienta ({}). Reintentá en unos segundos.",
                                    summary
                                );
                                if !has_streamed_text {
                                    let _ = text_tx
                                        .send(Ok(format_model_error_meta_token(&msg, retry_after)));
                                } else {
                                    let prefix = if !last_streamed_ends_newline {
                                        "\n\n"
                                    } else {
                                        ""
                                    };
                                    let _ = text_tx.send(Ok(format!("{}{}", prefix, msg)));
                                }
                                return Ok(());
                            }
                        }
                    }
                };

                tokio::pin!(raw_stream);

                assistant_text = String::new();
                tool_calls = Vec::new();
                let mut stream_sanitizer = StreamTextSanitizer::default();

                while let Some(token_result) = raw_stream.next().await {
                    match token_result {
                        Ok(token) if token.starts_with("__TOOL_CALL__") => {
                            if let Some((fb_model, fb_prov, cat, warn_msg)) =
                                pending_fallback_warning.take()
                            {
                                if cat == "model_fallback" {
                                    send_model_event(&text_tx, &fb_model, &fb_prov);
                                }
                                let wpayload = serde_json::json!({
                                    "category": cat,
                                    "provider": fb_prov,
                                    "message": warn_msg,
                                });
                                let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                            }
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Ok(token.clone()));
                            }
                            let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                            if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str)
                            {
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
                            // CORE-282: Intercept legacy token for models without tool use
                            if let Some(caps) = LEGACY_SPAWN_RE.captures(&text) {
                                let args = serde_json::json!({
                                    "name": &caps[2],
                                    "scope": &caps[3]
                                });
                                tool_calls.push(ToolCallRecord {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    type_: "function".to_string(),
                                    function: FunctionCallRecord {
                                        name: "spawn_agent".to_string(),
                                        arguments: args.to_string(),
                                    },
                                });
                                continue; // never emit legacy token to output
                            }
                            let clean_text = stream_sanitizer.push(&text);
                            if clean_text.is_empty() {
                                continue;
                            }
                            if let Some((fb_model, fb_prov, cat, warn_msg)) =
                                pending_fallback_warning.take()
                            {
                                if cat == "model_fallback" {
                                    send_model_event(&text_tx, &fb_model, &fb_prov);
                                }
                                let wpayload = serde_json::json!({
                                    "category": cat,
                                    "provider": fb_prov,
                                    "message": warn_msg,
                                });
                                let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                            }
                            emit_stream_text_chunk(
                                &text_tx,
                                clean_text,
                                autocorrect_retries == 0,
                                &mut assistant_text,
                                &mut has_streamed_text,
                                &mut last_streamed_ends_newline,
                                &mut needs_paragraph_break,
                            );
                        }
                        Err(e) => {
                            let sys_err = SystemError::HardwareFailure(format!("{}", e));
                            self.note_model_failure(
                                &active_model_id,
                                &active_provider,
                                pcb.task_type,
                                Some(&sys_err),
                            )
                            .await;
                            if autocorrect_retries == 0
                                && assistant_text.is_empty()
                                && tool_calls.is_empty()
                                && !has_streamed_text
                            {
                                warn!(
                                    pid = %pid,
                                    model = %active_model_id,
                                    provider = %active_provider,
                                    error = %e,
                                    "CHAL: stream interrupted with 0 tokens emitted — triggering empty-stream failover"
                                );
                                break;
                            }
                            if iteration == 0
                                && autocorrect_retries == 0
                                && pcb.agent_id.is_none()
                                && is_video_req
                                && tool_calls.is_empty()
                            {
                                warn!(
                                    pid = %pid,
                                    model = %active_model_id,
                                    provider = %active_provider,
                                    error = %e,
                                    "CHAL: stream interrupted mid-video-request before tool call — falling through to CHAL Guard video_produce_auto"
                                );
                                if has_streamed_text && !last_streamed_ends_newline {
                                    needs_paragraph_break = true;
                                }
                                break;
                            }
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Err(e.clone()));
                            }
                            return Err(sys_err);
                        }
                    }
                }

                let sanitized_tail = stream_sanitizer.flush();
                if !sanitized_tail.is_empty() {
                    emit_stream_text_chunk(
                        &text_tx,
                        sanitized_tail,
                        autocorrect_retries == 0,
                        &mut assistant_text,
                        &mut has_streamed_text,
                        &mut last_streamed_ends_newline,
                        &mut needs_paragraph_break,
                    );
                }

                // CORE-FIX (A): recover when ANY turn (including post-tool ReAct
                // continuations on iteration > 0) returns 200 OK with zero tokens.
                let stream_was_empty =
                    autocorrect_retries == 0 && assistant_text.is_empty() && tool_calls.is_empty();

                if stream_was_empty {
                    // Discard any unconfirmed fallback notice since 0 tokens arrived.
                    pending_fallback_warning = None;
                    self.note_empty_response(&active_model_id, &active_provider, pcb.task_type)
                        .await;
                    warn!(
                        pid = %pid,
                        iteration = iteration,
                        model = %active_model_id,
                        "model returned 0 content tokens — trying key rotation then fallback chain"
                    );

                    let mut recovered = false;

                    // 1. Try other keys of the SAME (provider, model).
                    if let Some(r) = self.router_ref.read().await.clone() {
                        let kp = r.read().await.key_pool_ref();
                        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                        let mut tried: std::collections::HashSet<String> =
                            std::collections::HashSet::new();
                        if let Some(ref kid) = active_key_id {
                            tried.insert(kid.clone());
                        }
                        if let Some(kid) = driver.current_key_id() {
                            tried.insert(kid.to_string());
                        }
                        for attempt in 0..2u32 {
                            let Some(alt) = kp
                                .get_available_key_excluding(
                                    &active_provider,
                                    &active_model_id,
                                    tenant_id,
                                    &tried,
                                )
                                .await
                            else {
                                break;
                            };
                            tried.insert(alt.key_id.clone());
                            let alt_url = alt
                                .api_url
                                .clone()
                                .unwrap_or_else(|| decision.api_url.clone());
                            let alt_driver = CloudProxyDriver::new_with_callback(
                                Arc::clone(&self.http_client),
                                alt_url,
                                alt.api_key.clone(),
                                active_model_id.clone(),
                                Some(alt.key_id.clone()),
                                make_rate_limited_cb(Some(alt.key_id.clone())),
                            );
                            match alt_driver
                                .generate_stream(local_messages.clone(), None, tools.clone())
                                .await
                            {
                                Ok(s) => {
                                    let mut rec_sanitizer = StreamTextSanitizer::default();
                                    let warn_msg = format!(
                                        "El modelo no respondió con la primera key; cambié a otra del mismo proveedor ({}).",
                                        active_provider
                                    );
                                    let (alt_text, alt_tool_calls) = consume_recovery_stream(
                                        s,
                                        &text_tx,
                                        &active_model_id,
                                        &active_provider,
                                        "key_rotated",
                                        warn_msg,
                                        false,
                                        &mut rec_sanitizer,
                                        &mut has_streamed_text,
                                        &mut last_streamed_ends_newline,
                                        &mut needs_paragraph_break,
                                        pid,
                                        "alternate key",
                                    )
                                    .await;
                                    if !alt_text.is_empty() || !alt_tool_calls.is_empty() {
                                        driver = alt_driver;
                                        active_key_id = Some(alt.key_id.clone());
                                        assistant_text = alt_text;
                                        tool_calls = alt_tool_calls;
                                        recovered = true;
                                        break;
                                    } else {
                                        self.note_empty_response(
                                            &active_model_id,
                                            &active_provider,
                                            pcb.task_type,
                                        )
                                        .await;
                                        warn!(
                                            pid = %pid,
                                            provider = %active_provider,
                                            key_id = %alt.key_id,
                                            attempt = attempt + 1,
                                            "alternate key also returned empty stream"
                                        );
                                    }
                                }
                                Err(e) => {
                                    self.note_model_failure(
                                        &active_model_id,
                                        &active_provider,
                                        pcb.task_type,
                                        Some(&e),
                                    )
                                    .await;
                                    warn!(
                                        pid = %pid,
                                        provider = %active_provider,
                                        key_id = %alt.key_id,
                                        attempt = attempt + 1,
                                        error = %e,
                                        "alternate key failed with HTTP error"
                                    );
                                }
                            }
                        }
                    }

                    // 2. Try static fallback chain models.
                    if !recovered {
                        for fb in &decision.fallback_chain {
                            if !tried_models.insert(fb.model_id.clone()) {
                                continue;
                            }
                            let fb_driver = CloudProxyDriver::new_with_callback(
                                Arc::clone(&self.http_client),
                                fb.api_url.clone(),
                                fb.api_key.clone(),
                                fb.model_id.clone(),
                                fb.key_id.clone(),
                                make_rate_limited_cb(fb.key_id.clone()),
                            );
                            let fb_tools = build_tools_for(&fb.provider, &extra_tools);
                            match fb_driver
                                .generate_stream(local_messages.clone(), None, fb_tools.clone())
                                .await
                            {
                                Ok(s) => {
                                    let mut rec_sanitizer = StreamTextSanitizer::default();
                                    let warn_msg = format!(
                                        "El modelo ({}) no respondió; cambiamos a {}.",
                                        active_model_id, fb.model_id
                                    );
                                    let (fb_text, fb_tool_calls) = consume_recovery_stream(
                                        s,
                                        &text_tx,
                                        &fb.model_id,
                                        &fb.provider,
                                        "model_fallback",
                                        warn_msg,
                                        true,
                                        &mut rec_sanitizer,
                                        &mut has_streamed_text,
                                        &mut last_streamed_ends_newline,
                                        &mut needs_paragraph_break,
                                        pid,
                                        "fallback",
                                    )
                                    .await;
                                    if !fb_text.is_empty() || !fb_tool_calls.is_empty() {
                                        active_model_id = fb.model_id.clone();
                                        active_provider = fb.provider.clone();
                                        active_key_id = fb.key_id.clone();
                                        tools = fb_tools;
                                        driver = fb_driver;
                                        assistant_text = fb_text;
                                        tool_calls = fb_tool_calls;
                                        recovered = true;
                                        break;
                                    } else {
                                        self.note_empty_response(
                                            &fb.model_id,
                                            &fb.provider,
                                            pcb.task_type,
                                        )
                                        .await;
                                        warn!(
                                            pid = %pid,
                                            fallback = %fb.model_id,
                                            "fallback also returned empty"
                                        );
                                    }
                                }
                                Err(e) => {
                                    self.note_model_failure(
                                        &fb.model_id,
                                        &fb.provider,
                                        pcb.task_type,
                                        Some(&e),
                                    )
                                    .await;
                                    warn!(
                                        pid = %pid,
                                        fallback = %fb.model_id,
                                        error = %e,
                                        "fallback model also failed (HTTP error)"
                                    );
                                }
                            }
                        }
                    }

                    // 3. Dynamic router failover across any remaining healthy providers/models.
                    if !recovered {
                        if let Some(r) = self.router_ref.read().await.clone() {
                            for _hop in 0..3u32 {
                                let dyn_dec =
                                    match r.read().await.decide_excluding(pcb, &tried_models).await
                                    {
                                        Ok(d) => d,
                                        Err(_) => break,
                                    };
                                if !tried_models.insert(dyn_dec.model_id.clone()) {
                                    break;
                                }
                                let dyn_driver = CloudProxyDriver::new_with_callback(
                                    Arc::clone(&self.http_client),
                                    dyn_dec.api_url.clone(),
                                    dyn_dec.api_key.clone(),
                                    dyn_dec.model_id.clone(),
                                    dyn_dec.key_id.clone(),
                                    make_rate_limited_cb(dyn_dec.key_id.clone()),
                                );
                                let dyn_tools = build_tools_for(&dyn_dec.provider, &extra_tools);
                                match dyn_driver
                                    .generate_stream(
                                        local_messages.clone(),
                                        None,
                                        dyn_tools.clone(),
                                    )
                                    .await
                                {
                                    Ok(s) => {
                                        let mut rec_sanitizer = StreamTextSanitizer::default();
                                        let warn_msg = format!(
                                            "El modelo ({}) no respondió; cambiamos a {}.",
                                            active_model_id, dyn_dec.model_id
                                        );
                                        let (dyn_text, dyn_tool_calls) = consume_recovery_stream(
                                            s,
                                            &text_tx,
                                            &dyn_dec.model_id,
                                            &dyn_dec.provider,
                                            "model_fallback",
                                            warn_msg,
                                            true,
                                            &mut rec_sanitizer,
                                            &mut has_streamed_text,
                                            &mut last_streamed_ends_newline,
                                            &mut needs_paragraph_break,
                                            pid,
                                            "dynamic fallback",
                                        )
                                        .await;
                                        if !dyn_text.is_empty() || !dyn_tool_calls.is_empty() {
                                            active_model_id = dyn_dec.model_id.clone();
                                            active_provider = dyn_dec.provider.clone();
                                            active_key_id = dyn_dec.key_id.clone();
                                            tools = dyn_tools;
                                            driver = dyn_driver;
                                            assistant_text = dyn_text;
                                            tool_calls = dyn_tool_calls;
                                            recovered = true;
                                            break;
                                        } else {
                                            self.note_empty_response(
                                                &dyn_dec.model_id,
                                                &dyn_dec.provider,
                                                pcb.task_type,
                                            )
                                            .await;
                                        }
                                    }
                                    Err(e) => {
                                        self.note_model_failure(
                                            &dyn_dec.model_id,
                                            &dyn_dec.provider,
                                            pcb.task_type,
                                            Some(&e),
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                    }

                    if !recovered {
                        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                        let mut providers: Vec<&str> = vec![active_provider.as_str()];
                        for fb in &decision.fallback_chain {
                            providers.push(fb.provider.as_str());
                        }
                        let max_cooldown = self
                            .cooldown_and_invalidate_sticky(&providers, tenant_id)
                            .await;

                        let msg = if max_cooldown > 0 {
                            format!(
                                "No tengo modelos disponibles ahora. Probaron \
                                 {} y todos los respaldos sin éxito. Reintentá \
                                 en ~{}s o configurá un proveedor distinto.",
                                active_model_id, max_cooldown
                            )
                        } else if decision.fallback_chain.is_empty() {
                            format!(
                                "El modelo '{}' no devolvió respuesta y no hay \
                                 otros modelos disponibles ahora. Revisá tu \
                                 selección de proveedor o agregá una alternativa.",
                                active_model_id
                            )
                        } else {
                            format!(
                                "No tengo modelos disponibles ahora. El modelo \
                                 '{}' y todos sus respaldos fallaron. Reintentá \
                                 en unos segundos.",
                                active_model_id
                            )
                        };

                        if !has_streamed_text {
                            let _ =
                                text_tx.send(Ok(format_model_error_meta_token(&msg, max_cooldown)));
                        } else {
                            let prefix = if !last_streamed_ends_newline {
                                "\n\n"
                            } else {
                                ""
                            };
                            let _ = text_tx.send(Ok(format!("{}{}", prefix, msg)));
                        }
                        tool_calls.clear();
                        finished = true;
                        break;
                    }
                }

                // CORE-ARCH / GUARD: Si el ChatAgent (Nivel 0) no emitió tool calls pero prometió en texto
                // que el equipo/supervisor trabajaría, o si el pedido del usuario es técnico (script, código, etc.),
                // NO permitir alucinación en texto sin ejecución real. Sintetizar spawn_agent para garantizar
                // que el supervisor se cree, aparezca en el Dashboard y ejecute la tarea en segundo plano.
                if tool_calls.is_empty() && pcb.agent_id.is_none() && iteration == 0 {
                    if Self::is_status_query(&pcb.memory_pointers.l1_instruction) {
                        info!(
                            pid = %pid,
                            "CHAL Guard: ChatAgent Level 0 status query intercepted without tool call. Auto-synthesizing get_agent_status."
                        );
                        tool_calls.push(ToolCallRecord {
                            id: format!("call_status_{}", uuid::Uuid::new_v4()),
                            type_: "function".to_string(),
                            function: FunctionCallRecord {
                                name: "get_agent_status".to_string(),
                                arguments: serde_json::json!({}).to_string(),
                            },
                        });
                    } else if let Some(resolved_video_prompt) = Self::resolve_video_request_prompt(
                        &pcb.memory_pointers.l1_instruction,
                        &pcb.message_history,
                        &assistant_text,
                    ) {
                        info!(
                            pid = %pid,
                            "CHAL Guard: ChatAgent Level 0 video request/nudge intercepted without tool call. Auto-synthesizing video_produce_auto."
                        );
                        tool_calls.push(ToolCallRecord {
                            id: format!("call_video_{}", uuid::Uuid::new_v4()),
                            type_: "function".to_string(),
                            function: FunctionCallRecord {
                                name: "video_produce_auto".to_string(),
                                arguments: serde_json::json!({
                                    "prompt": resolved_video_prompt,
                                })
                                .to_string(),
                            },
                        });
                    } else {
                        let is_video_advisory = Self::is_video_advisory_or_followup_query(
                            &pcb.memory_pointers.l1_instruction,
                        );
                        let task_is_technical = !is_video_advisory
                            && Self::has_technical_context(
                                &pcb.memory_pointers.l1_instruction,
                                &pcb.message_history,
                            );
                        let claims_delegation = Self::assistant_claims_delegation(&assistant_text);

                        if claims_delegation || task_is_technical {
                            let project_scope = Self::derive_task_scope(
                                &pcb.memory_pointers.l1_instruction,
                                &pcb.message_history,
                            );
                            let project_name = Self::derive_task_project_name(&project_scope);
                            let task_type_str = match crate::pcb::infer_task_type(&project_scope) {
                                crate::pcb::TaskType::Code => "code",
                                crate::pcb::TaskType::Analysis => "analysis",
                                crate::pcb::TaskType::Planning => "planning",
                                _ => "code",
                            };

                            info!(
                                pid = %pid,
                                project = %project_name,
                                claims_delegation,
                                task_is_technical,
                                "CHAL Guard: ChatAgent Level 0 delegating via synthesized spawn_agent tool call"
                            );

                            tool_calls.push(ToolCallRecord {
                                id: format!("call_auto_{}", uuid::Uuid::new_v4()),
                                type_: "function".to_string(),
                                function: FunctionCallRecord {
                                    name: "spawn_agent".to_string(),
                                    arguments: serde_json::json!({
                                        "role": "project_supervisor",
                                        "name": project_name,
                                        "scope": project_scope,
                                        "task_type": task_type_str,
                                    })
                                    .to_string(),
                                },
                            });
                        }
                    }
                }

                if tool_calls.is_empty() {
                    break;
                }

                // Intercept and validate/sanitize tool calls
                let mut validated_calls = Vec::new();
                let mut validation_errors = Vec::new();
                let role = AgentRole::ChatAgent;

                for tc in &tool_calls {
                    // Never allow ChatAgent to hijack a video request into a background ProjectSupervisor.
                    let rewritten_tc;
                    let target_tc = if is_video_req && tc.function.name == "spawn_agent" {
                        info!(
                            pid = %pid,
                            "CHAL Guard: Redirecting accidental spawn_agent on video request to video_produce_auto"
                        );
                        let resolved_prompt = Self::resolve_video_request_prompt(
                            &pcb.memory_pointers.l1_instruction,
                            &pcb.message_history,
                            &assistant_text,
                        )
                        .unwrap_or_else(|| pcb.memory_pointers.l1_instruction.clone());
                        rewritten_tc = ToolCallRecord {
                            id: tc.id.clone(),
                            type_: "function".to_string(),
                            function: FunctionCallRecord {
                                name: "video_produce_auto".to_string(),
                                arguments: serde_json::json!({
                                    "prompt": resolved_prompt,
                                })
                                .to_string(),
                            },
                        };
                        &rewritten_tc
                    } else {
                        tc
                    };
                    match autocorrect::validate_and_sanitize_tool_call_with_extra(
                        target_tc,
                        &role,
                        &extra_tools,
                    ) {
                        Ok(sanitized) => validated_calls.push(sanitized),
                        Err(e) => validation_errors.push(e),
                    }
                }

                if validation_errors.is_empty() {
                    tool_calls = validated_calls;
                    messages = local_messages; // Commit the entire retry history!
                    break;
                }

                // Tool validation failed!
                if autocorrect_retries < 2 {
                    autocorrect_retries += 1;
                    warn!(
                        pid = %pid,
                        retries = autocorrect_retries,
                        errors = ?validation_errors,
                        "Chat loop: Tool validation failed, starting private autocorrect retry loop"
                    );

                    // Push the failing assistant response
                    local_messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: if assistant_text.is_empty() {
                            None
                        } else {
                            Some(assistant_text.clone())
                        },
                        tool_calls: Some(tool_calls.clone()),
                        ..Default::default()
                    });

                    // Push a Tool role message for each tool call
                    for tc in &tool_calls {
                        let error_for_this = validation_errors
                            .iter()
                            .find(|err| err.contains(&tc.function.name) || err.contains(&tc.id));
                        let err_msg = match error_for_this {
                            Some(msg) => msg.clone(),
                            None => format!(
                                "Error: La llamada a la herramienta '{}' no pasó la validación.",
                                tc.function.name
                            ),
                        };
                        local_messages.push(ChatMessage {
                            role: ChatRole::Tool,
                            content: Some(err_msg),
                            tool_call_id: Some(tc.id.clone()),
                            name: Some(tc.function.name.clone()),
                            ..Default::default()
                        });
                    }

                    // Continue loop to retry generation using local_messages
                    continue;
                } else {
                    let err_msg = format!(
                        "Chat loop failed tool validation after 2 retries. Errors: {:?}",
                        validation_errors
                    );
                    let _ = text_tx.send(Ok(format!(
                        "Lo siento, la llamada a la herramienta '{}' falló la validación y no se pudo corregir después de varios intentos internos. Detalle: {}",
                        tool_calls.first().map(|tc| &tc.function.name).unwrap_or(&"desconocida".to_string()),
                        validation_errors.join("; ")
                    )));
                    return Err(SystemError::HardwareFailure(err_msg));
                }
            }

            if finished {
                break;
            }

            if tool_calls.is_empty() {
                last_assistant_text = assistant_text.clone();
                if !rendered_video_ids.is_empty() {
                    let mut suffix = String::new();
                    for vid_id in &rendered_video_ids {
                        let tag = format!("[AEGIS_VIDEO_CARD:{}]", vid_id);
                        if !last_assistant_text.contains(&tag) {
                            suffix.push_str(&format!("\n\n{}", tag));
                        }
                    }
                    if !suffix.is_empty() {
                        let _ = text_tx.send(Ok(suffix.clone()));
                        last_assistant_text.push_str(&suffix);
                    }
                }
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
            let mut refresh_module_tools = false;
            for tc in &tool_calls {
                if tc.function.name == "manage_modules" {
                    refresh_module_tools = true;
                }
                if tc.function.name.starts_with("video_") {
                    let msg = if tc.function.name == "video_produce_auto" {
                        "🎬 Iniciando Aegis Video Studio: preparando escenas, locución y exportación MP4...".to_string()
                    } else {
                        format!(
                            "🎬 Ejecutando {} en Aegis Video Studio...",
                            tc.function.name
                        )
                    };
                    let start_prog = serde_json::json!({
                        "stage": tc.function.name,
                        "progress": 0.05,
                        "message": msg,
                        "done": false,
                    });
                    let _ = text_tx.send(Ok(format!("__PROGRESS__{}", start_prog)));
                }
                let result = self
                    .execute_tool_call_with_progress(tc, pid, pcb, Some(&text_tx))
                    .await;
                collect_rendered_video_card_ids(
                    &tc.function.name,
                    &result,
                    &mut rendered_video_ids,
                );
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
            if refresh_module_tools && !casual_turn {
                extra_tools = load_extra_tools().await;
                tools = build_tools_for(&active_provider, &extra_tools);
            }
        }

        if !finished {
            let _ = text_tx.send(Ok(
                "Lo siento, alcancé el límite máximo de pasos internos al procesar tu solicitud. Por favor, reformulá tu pedido."
                    .to_string(),
            ));
        }

        // CORE-FIX (D2): mark this model as successful so future routing
        // decisions can reflect its real-world success rate.
        // CORE-328: tallied under the (model, task) composite too.
        if let Some(r) = self.router_ref.read().await.clone() {
            r.read()
                .await
                .tracker_ref()
                .record_success_for_task(&active_model_id, pcb.task_type)
                .await;
        }

        // CORE-378: Indexación episódica semántica automática del turno conversacional
        if finished && !last_assistant_text.is_empty() {
            let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
            let safe_user_text = Self::mask_api_keys_in_text(&pcb.memory_pointers.l1_instruction);
            let safe_assistant_text = Self::mask_api_keys_in_text(last_assistant_text.trim());
            let memory_entry = format!(
                "User: {}\nAssistant: {}",
                safe_user_text, safe_assistant_text
            );
            let _ = self.store_memory(tenant, &memory_entry).await;
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

        let router_opt = self.router_ref.read().await.clone();
        let make_rate_limited_cb = |key_id: Option<String>| match (router_opt.clone(), key_id) {
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
        };

        let mut active_model_id = decision.model_id.clone();
        let mut active_provider = decision.provider.clone();
        let mut driver = CloudProxyDriver::new_with_callback(
            Arc::clone(&self.http_client),
            decision.api_url.clone(),
            decision.api_key.clone(),
            decision.model_id.clone(),
            decision.key_id.clone(),
            make_rate_limited_cb(decision.key_id.clone()),
        );

        // CORE-FIX: resolve the agent's real tenant_id ONCE so the tool
        // executor's mock PCB points filesystem + execute_command at the
        // correct workspace (users/<tenant>/workspace). Without this the
        // mock PCB had no tenant_id and `get_tenant_workspace` fell back to
        // `users/default/workspace` — so a specialist's `git clone` and every
        // read_file/list_files ran against the wrong (shared "default")
        // workspace, breaking the clone-and-verify flow and tenant isolation.
        let agent_tenant_id: Option<String> = {
            let orch = self.agent_orchestrator.read().await.clone();
            match orch {
                Some(o) => o
                    .tree
                    .read()
                    .await
                    .get(&agent_id)
                    .map(|n| n.tenant_id.clone())
                    .filter(|t| !t.is_empty()),
                None => None,
            }
        };

        let mut messages = messages;
        const MAX_ITERATIONS: usize = 10;

        let (agent_task_type, agent_role_meta, agent_model_pref) = {
            let orch = self.agent_orchestrator.read().await.clone();
            match orch {
                Some(o) => o
                    .tree
                    .read()
                    .await
                    .get(&agent_id)
                    .map(|n| (n.task_type, n.role.clone(), n.model_preference))
                    .unwrap_or((
                        crate::pcb::TaskType::Code,
                        AgentRole::Supervisor {
                            name: "Agent".to_string(),
                            scope: "generic".to_string(),
                        },
                        crate::scheduler::ModelPreference::HybridSmart,
                    )),
                None => (
                    crate::pcb::TaskType::Code,
                    AgentRole::Supervisor {
                        name: "Agent".to_string(),
                        scope: "generic".to_string(),
                    },
                    crate::scheduler::ModelPreference::HybridSmart,
                ),
            }
        };
        let mut tools = tools;
        let mut tried_models: std::collections::HashSet<String> = std::collections::HashSet::new();
        tried_models.insert(decision.model_id.clone());
        let build_agent_tools = |prov: &str| -> Option<Vec<serde_json::Value>> {
            let defs = ToolRegistry::tools_for(&agent_role_meta, &ProviderKind::from_string(prov));
            if defs.is_empty() {
                None
            } else {
                Some(defs)
            }
        };

        for iteration in 0..MAX_ITERATIONS {
            let mut autocorrect_retries = 0;
            let mut local_messages = messages.clone();

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

            loop {
                let raw_stream = match driver
                    .generate_stream(local_messages.clone(), None, tools.clone())
                    .await
                {
                    Ok(s) => s,
                    Err(primary_err) if iteration == 0 && autocorrect_retries == 0 => {
                        self.note_model_failure(
                            &active_model_id,
                            &active_provider,
                            agent_task_type,
                            Some(&primary_err),
                        )
                        .await;
                        let mut recovered = None;
                        for fb in &decision.fallback_chain {
                            if !tried_models.insert(fb.model_id.clone()) {
                                continue;
                            }
                            let fb_driver = CloudProxyDriver::new_with_callback(
                                Arc::clone(&self.http_client),
                                fb.api_url.clone(),
                                fb.api_key.clone(),
                                fb.model_id.clone(),
                                fb.key_id.clone(),
                                make_rate_limited_cb(fb.key_id.clone()),
                            );
                            let fb_tools = build_agent_tools(&fb.provider);
                            match fb_driver
                                .generate_stream(local_messages.clone(), None, fb_tools.clone())
                                .await
                            {
                                Ok(s) => {
                                    active_model_id = fb.model_id.clone();
                                    active_provider = fb.provider.clone();
                                    tools = fb_tools;
                                    driver = fb_driver;
                                    recovered = Some(s);
                                    break;
                                }
                                Err(e) => {
                                    self.note_model_failure(
                                        &fb.model_id,
                                        &fb.provider,
                                        agent_task_type,
                                        Some(&e),
                                    )
                                    .await;
                                }
                            }
                        }
                        if recovered.is_none() {
                            if let Some(r) = self.router_ref.read().await.clone() {
                                let mut dyn_pcb = crate::pcb::PCB::new(
                                    format!("agent_fb_{}", agent_id),
                                    5,
                                    String::new(),
                                );
                                dyn_pcb.task_type = agent_task_type;
                                dyn_pcb.model_pref = agent_model_pref;
                                dyn_pcb.agent_id = Some(agent_id);
                                dyn_pcb.tenant_id = agent_tenant_id.clone();
                                for _hop in 0..3u32 {
                                    let dyn_dec = match r
                                        .read()
                                        .await
                                        .decide_excluding(&dyn_pcb, &tried_models)
                                        .await
                                    {
                                        Ok(d) => d,
                                        Err(_) => break,
                                    };
                                    if !tried_models.insert(dyn_dec.model_id.clone()) {
                                        break;
                                    }
                                    let dyn_driver = CloudProxyDriver::new_with_callback(
                                        Arc::clone(&self.http_client),
                                        dyn_dec.api_url.clone(),
                                        dyn_dec.api_key.clone(),
                                        dyn_dec.model_id.clone(),
                                        dyn_dec.key_id.clone(),
                                        make_rate_limited_cb(dyn_dec.key_id.clone()),
                                    );
                                    let dyn_tools = build_agent_tools(&dyn_dec.provider);
                                    match dyn_driver
                                        .generate_stream(
                                            local_messages.clone(),
                                            None,
                                            dyn_tools.clone(),
                                        )
                                        .await
                                    {
                                        Ok(s) => {
                                            active_model_id = dyn_dec.model_id.clone();
                                            active_provider = dyn_dec.provider.clone();
                                            tools = dyn_tools;
                                            driver = dyn_driver;
                                            recovered = Some(s);
                                            break;
                                        }
                                        Err(e) => {
                                            self.note_model_failure(
                                                &dyn_dec.model_id,
                                                &dyn_dec.provider,
                                                agent_task_type,
                                                Some(&e),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                        }
                        match recovered {
                            Some(s) => s,
                            None => return Err(primary_err),
                        }
                    }
                    Err(e) => {
                        self.note_model_failure(
                            &active_model_id,
                            &active_provider,
                            agent_task_type,
                            Some(&e),
                        )
                        .await;
                        return Err(e);
                    }
                };
                tokio::pin!(raw_stream);

                assistant_text = String::new();
                tool_calls = Vec::new();

                while let Some(token_result) = raw_stream.next().await {
                    match token_result {
                        Ok(token) if token.starts_with("__TOOL_CALL__") => {
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Ok(token.clone()));
                            }
                            let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                            if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str)
                            {
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
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Ok(text));
                            }
                        }
                        Err(e) => {
                            let sys_err = SystemError::HardwareFailure(format!("{}", e));
                            self.note_model_failure(
                                &active_model_id,
                                &active_provider,
                                agent_task_type,
                                Some(&sys_err),
                            )
                            .await;
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Err(e.clone()));
                            }
                            return Err(sys_err);
                        }
                    }
                }

                if tool_calls.is_empty() {
                    break;
                }

                // Intercept and validate/sanitize tool calls
                let mut validated_calls = Vec::new();
                let mut validation_errors = Vec::new();

                let agent_role = {
                    let orch = self.agent_orchestrator.read().await.clone();
                    match orch {
                        Some(o) => o
                            .tree
                            .read()
                            .await
                            .get(&agent_id)
                            .map(|n| n.role.clone())
                            .unwrap_or(AgentRole::Supervisor {
                                name: "Agent".to_string(),
                                scope: "generic".to_string(),
                            }),
                        None => AgentRole::Supervisor {
                            name: "Agent".to_string(),
                            scope: "generic".to_string(),
                        },
                    }
                };

                for tc in &tool_calls {
                    match autocorrect::validate_and_sanitize_tool_call(tc, &agent_role) {
                        Ok(sanitized) => validated_calls.push(sanitized),
                        Err(e) => validation_errors.push(e),
                    }
                }

                if validation_errors.is_empty() {
                    tool_calls = validated_calls;
                    messages = local_messages; // Commit the entire retry history!
                    break;
                }

                // Tool validation failed!
                if autocorrect_retries < 2 {
                    autocorrect_retries += 1;
                    warn!(
                        agent_id = %agent_id,
                        retries = autocorrect_retries,
                        errors = ?validation_errors,
                        "Agent loop: Tool validation failed, starting private autocorrect retry loop"
                    );

                    // Push the failing assistant response
                    local_messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: if assistant_text.is_empty() {
                            None
                        } else {
                            Some(assistant_text.clone())
                        },
                        tool_calls: Some(tool_calls.clone()),
                        ..Default::default()
                    });

                    // Push a Tool role message for each tool call
                    for tc in &tool_calls {
                        let error_for_this = validation_errors
                            .iter()
                            .find(|err| err.contains(&tc.function.name) || err.contains(&tc.id));
                        let err_msg = match error_for_this {
                            Some(msg) => msg.clone(),
                            None => format!(
                                "Error: La llamada a la herramienta '{}' no pasó la validación.",
                                tc.function.name
                            ),
                        };
                        local_messages.push(ChatMessage {
                            role: ChatRole::Tool,
                            content: Some(err_msg),
                            tool_call_id: Some(tc.id.clone()),
                            name: Some(tc.function.name.clone()),
                            ..Default::default()
                        });
                    }

                    // Continue loop to retry generation using local_messages
                    continue;
                } else {
                    let err_msg = format!(
                        "Agent loop failed tool validation after 2 retries. Errors: {:?}",
                        validation_errors
                    );
                    let _ = text_tx.send(Ok(format!(
                        "Lo siento, la llamada a la herramienta '{}' falló la validación y no se pudo corregir después de varios intentos internos. Detalle: {}",
                        tool_calls.first().map(|tc| &tc.function.name).unwrap_or(&"desconocida".to_string()),
                        validation_errors.join("; ")
                    )));
                    return Err(SystemError::HardwareFailure(err_msg));
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
            // CORE-FIX: carry the real tenant so workspace-scoped tools resolve
            // to users/<tenant>/workspace instead of users/default.
            mock_pcb.tenant_id = agent_tenant_id.clone();

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

        if let Some(r) = self.router_ref.read().await.clone() {
            r.read()
                .await
                .tracker_ref()
                .record_success_for_task(&active_model_id, agent_task_type)
                .await;
        }

        Ok(())
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

    /// CORE-378: Realiza una búsqueda semántica en la memoria L3 del tenant
    pub async fn search_memory(
        &self,
        tenant_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::vcm::swap::MemoryFragment>, SystemError> {
        let driver = self.embedding_driver.as_ref().ok_or_else(|| {
            SystemError::HardwareFailure("No embedding driver configured for memory search".into())
        })?;

        let query_vec = driver.embed(query).await?;
        self.swap_manager
            .search(tenant_id, query_vec, limit)
            .await
            .map_err(|e| SystemError::HardwareFailure(format!("Swap search failed: {}", e)))
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
        let (messages, _warnings) = self.build_messages_with_warnings(pcb, persona).await;
        messages
    }

    async fn get_dynamic_scripts_section(tenant_id: &str) -> String {
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let workspace_path = std::path::Path::new(&base_dir)
            .join("users")
            .join(tenant_id)
            .join("workspace");

        let mut scripts_list = String::new();

        if let Ok(entries) = std::fs::read_dir(workspace_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("js") {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        let mut description = "No description available.".to_string();
                        let mut params_example = "{}".to_string();

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            for line in content.lines().take(5) {
                                let trimmed = line.trim();
                                if trimmed.starts_with("// Description:")
                                    || trimmed.starts_with("// description:")
                                {
                                    description = trimmed
                                        .split_once(':')
                                        .map(|x| x.1)
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                } else if trimmed.starts_with("// Parameters:")
                                    || trimmed.starts_with("// parameters:")
                                    || trimmed.starts_with("// Example:")
                                {
                                    params_example = trimmed
                                        .split_once(':')
                                        .map(|x| x.1)
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                }
                            }
                        }

                        scripts_list.push_str(&format!(
                        "- **{}**: {}\n  Uso: [SYS_CALL_MAKER(\"js\", \"return eval(read_file('{}'))\", {})]\n",
                        filename, description, filename, params_example
                    ));
                    }
                }
            }
        }

        if scripts_list.is_empty() {
            String::new()
        } else {
            format!(
            "\n\nDYNAMIC SCRIPTS (HERRAMIENTAS DINÁMICAS GUARDADAS EN EL WORKSPACE):\n\
             Podés reutilizar los siguientes scripts previamente creados y guardados en tu espacio de trabajo:\n\
             {}\n",
            scripts_list
        )
        }
    }

    /// CORE-FIX (A3): variant that also returns any non-fatal warnings produced
    /// while assembling the context (e.g. VCM failures that triggered a
    /// fallback to the raw l1_instruction). Callers that have a way to surface
    /// this to the user (the chat ReAct loop) should use this variant.
    pub async fn build_messages_with_warnings(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        self.build_messages_internal(pcb, persona, false).await
    }

    /// Compact chat-session variant used by `execute_chat_react`: omits redundant
    /// `[SYS_CALL_PLUGIN]` instructions for tools already exposed natively (`ledger`,
    /// `chronos`), and strips heavy sections on casual greetings.
    pub async fn build_messages_for_chat_with_warnings(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        self.build_messages_internal(pcb, persona, true).await
    }

    async fn build_messages_internal(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
        compact_chat: bool,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        let mut warnings: Vec<String> = Vec::new();
        let casual_turn = compact_chat && is_casual_chat_turn(pcb);

        // Skip heavy VCM assembly (git/governance/embeddings) for direct Chat turns
        // with no explicit L2/swap pointers so prompts stay compact and fast.
        let skip_vcm = pcb.task_type == crate::pcb::TaskType::Chat
            && pcb.agent_id.is_none()
            && pcb.memory_pointers.l2_context_refs.is_empty()
            && pcb.memory_pointers.swap_refs.is_empty();

        // 1. Ensamblar contexto via VCM (L1 + L2 + L3) cuando aplique
        let assembled_context = if skip_vcm {
            pcb.memory_pointers.l1_instruction.clone()
        } else {
            self.vcm
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
                    warnings.push(format!(
                        "Context assembly failed; responding with reduced memory. Detail: {}",
                        e
                    ));
                    pcb.memory_pointers.l1_instruction.clone()
                })
        };

        let tool_prompt = if casual_turn {
            String::new()
        } else if compact_chat && pcb.agent_id.is_none() {
            // ledger & chronos are already exposed as native ChatAgent function tools
            self.plugin_manager
                .read()
                .await
                .get_available_tools_prompt_excluding(&["ledger", "chronos"])
        } else {
            self.plugin_manager
                .read()
                .await
                .get_available_tools_prompt()
        };
        let mcp_tool_prompt = if casual_turn {
            String::new()
        } else {
            self.mcp_registry.generate_system_prompt().await
        };

        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        let modules_tool_prompt = if casual_turn {
            String::new()
        } else if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
            if let Some(ref router_lock) = *self.router.read().await {
                let router = router_lock.read().await;
                router
                    .generate_modules_prompt(
                        &pcb.memory_pointers.l1_instruction,
                        tenant_id,
                        session_key,
                    )
                    .await
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let persona_section = match persona {
            Some(p) if !p.trim().is_empty() => PERSONA_SECTION_TEMPLATE.replace("{persona}", p),
            _ => String::new(),
        };

        // CORE-148: Music instructions are only injected if the music_search plugin is active.
        let has_music_plugin = !casual_turn
            && self
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

        let has_maker_plugin =
            !casual_turn && self.plugin_manager.read().await.is_plugin_active("maker");

        let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
            let dynamic_scripts = Self::get_dynamic_scripts_section(tenant_id).await;
            if !compact_chat {
                format!("{}{}", MAKER_INSTRUCTIONS, dynamic_scripts)
            } else {
                let prompt_lower = pcb.memory_pointers.l1_instruction.to_lowercase();
                let wants_maker = !dynamic_scripts.trim().is_empty()
                    || prompt_lower.contains("script")
                    || prompt_lower.contains("javascript")
                    || prompt_lower.contains("maker")
                    || prompt_lower.contains("automatiz")
                    || prompt_lower.contains("boa");
                if wants_maker {
                    format!("{}{}", MAKER_INSTRUCTIONS, dynamic_scripts)
                } else {
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let system_content = if tool_prompt.trim().is_empty()
            && mcp_tool_prompt.trim().is_empty()
            && modules_tool_prompt.trim().is_empty()
        {
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
                "{}{}{}{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}{}{}",
                maker_section,
                SPAWN_INSTRUCTIONS,
                role_instructions,
                persona_section,
                music_section,
                tool_prompt,
                mcp_tool_prompt,
                modules_tool_prompt
            )
        };

        // CORE-375: Bloque de memoria a largo plazo y resumen financiero del tenant
        let personal_memory_section = if !casual_turn && pcb.agent_id.is_none() {
            if let (Some(tid), Some(skey)) = (&pcb.tenant_id, &pcb.session_key) {
                if let Ok(db) = crate::enclave::TenantDB::open(tid, skey) {
                    db.build_personal_memory_prompt_block().unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let mut full_system_content = system_content;
        if pcb.agent_id.is_none() {
            use chrono::Datelike;
            let now_local = chrono::Local::now();
            let weekday_es = match now_local.weekday() {
                chrono::Weekday::Mon => "Lunes",
                chrono::Weekday::Tue => "Martes",
                chrono::Weekday::Wed => "Miércoles",
                chrono::Weekday::Thu => "Jueves",
                chrono::Weekday::Fri => "Viernes",
                chrono::Weekday::Sat => "Sábado",
                chrono::Weekday::Sun => "Domingo",
            };
            full_system_content.push_str(&format!(
                "\n\n[CONTEXTO TEMPORAL ACTUAL]\n- Fecha y hora local del sistema: {} {}\n",
                weekday_es,
                now_local.format("%Y-%m-%d %H:%M (%:z)")
            ));
        }
        if !personal_memory_section.is_empty() {
            full_system_content.push_str(&personal_memory_section);
        }

        let mut messages = vec![ChatMessage {
            role: ChatRole::System,
            content: Some(full_system_content),
            ..Default::default()
        }];

        // CORE-260: Historial previo de la sesión
        messages.extend(pcb.message_history.clone());

        messages.push(ChatMessage {
            role: ChatRole::User,
            content: Some(assembled_context),
            ..Default::default()
        });

        (messages, warnings)
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
