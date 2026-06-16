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
}

#[cfg(test)]
fn _assert_hal_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<CognitiveHAL>();
}

/// CORE-FIX (B): translate the raw `SystemError` we got from a failed provider
/// call into a one-sentence user-facing description. The chat layer uses this
/// to compose "No pude responder con X. <summary>" so the user understands
/// *why* every model failed instead of seeing silent empty output.
fn summarise_provider_failure(err: &SystemError) -> String {
    let s = err.to_string();
    let lower = s.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        return "Las API keys configuradas no son válidas o no tienen acceso a \
                este modelo (HTTP 401)."
            .to_string();
    }
    if lower.contains("429")
        || lower.contains("resource_exhausted")
        || lower.contains("quota")
        || lower.contains("rate")
    {
        return "Te quedaste sin cuota gratuita o sos rate-limited (HTTP 429). \
                Esperá unos segundos o agregá billing al proveedor."
            .to_string();
    }
    if lower.contains("404") {
        return "El modelo no existe o el endpoint está mal configurado \
                (HTTP 404)."
            .to_string();
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return "El proveedor no respondió a tiempo (timeout).".to_string();
    }
    if lower.contains("connection") || lower.contains("dns") {
        return "No pude alcanzar el endpoint del proveedor (problema de red).".to_string();
    }
    "El proveedor devolvió un error desconocido.".to_string()
}

/// CORE-FIX (F): is this a DETERMINISTIC failure that won't succeed on retry
/// with the same model? Two cases the smoke test exposed:
///   - HTTP 401 Unauthorized: the key isn't authorized for this model/provider.
///   - Gemini 429 with `limit: 0`: the model isn't on the key's tier at all
///     (e.g. gemini-2.5-pro on the free tier — quota is literally zero, not
///     "temporarily exhausted"). A normal 429 with retryDelay is NOT permanent.
///
/// When true, the caller trips the per-model circuit so the router stops
/// re-picking a model that can never work and rotates to one that can
/// (gemini-2.5-flash, gpt-oss:120b, …).
fn is_permanent_model_failure(err: &SystemError) -> bool {
    let s = err.to_string();
    let lower = s.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        return true;
    }
    // HTTP 403 / subscription-gated model: the key authenticates fine but this
    // specific model isn't on its plan (e.g. Ollama free tier returns 403
    // "this model requires a subscription, upgrade for access" for the 671B
    // models, which the /models listing still advertises). Retrying never
    // helps — pin the model out so the router promotes a callable sibling.
    if lower.contains("403")
        || lower.contains("forbidden")
        || lower.contains("requires a subscription")
        || lower.contains("upgrade for access")
    {
        return true;
    }
    // Gemini tier-0: the body carries `"limit": 0` (possibly with spaces).
    // Only treat 429s this way — a 200 with limit:0 makes no sense.
    if lower.contains("429") || lower.contains("resource_exhausted") {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.contains("\"limit\":0") || compact.contains("limit:0") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod summarise_tests {
    use super::*;

    #[test]
    fn detects_401() {
        let e =
            SystemError::HardwareFailure("Cloud API Error 401 Unauthorized: unauthorized".into());
        assert!(summarise_provider_failure(&e).contains("401"));
    }

    #[test]
    fn detects_429_and_quota() {
        for body in [
            "API Error 429 Too Many Requests",
            "RESOURCE_EXHAUSTED",
            "rate limit exceeded",
            "quota exhausted",
        ] {
            let e = SystemError::HardwareFailure(body.into());
            assert!(
                summarise_provider_failure(&e).contains("429"),
                "expected 429 summary for {:?}",
                body
            );
        }
    }

    #[test]
    fn falls_back_to_unknown() {
        let e = SystemError::HardwareFailure("something weird happened".into());
        assert!(summarise_provider_failure(&e).contains("desconocido"));
    }

    #[test]
    fn permanent_failure_detects_401() {
        let e =
            SystemError::HardwareFailure("Cloud API Error 401 Unauthorized: unauthorized".into());
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_detects_403_subscription() {
        // Ollama free tier serving a 671B model: key is valid but the model
        // is subscription-gated. Must be treated as permanent, not retried.
        let e = SystemError::HardwareFailure(
            "Cloud API Error 403 Forbidden: {\"error\":\"this model requires a \
             subscription, upgrade for access: https://ollama.com/upgrade\"}"
                .into(),
        );
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_detects_gemini_tier_zero() {
        // The real shape from the smoke test: 429 RESOURCE_EXHAUSTED with limit: 0.
        let e = SystemError::HardwareFailure(
            "API Error 429 Too Many Requests: { \"error\": { \"code\": 429, \
             \"message\": \"Quota exceeded ... limit: 0, model: gemini-2.5-pro\", \
             \"status\": \"RESOURCE_EXHAUSTED\" } }"
                .into(),
        );
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_ignores_transient_429() {
        // A normal rate limit (limit > 0, has retryDelay) is NOT permanent —
        // the model works, it's just throttled. Must not trip the model circuit.
        let e = SystemError::HardwareFailure(
            "API Error 429 Too Many Requests: Rate limit reached ... \
             Limit 12000, Used 10797. Please try again in 11.725s"
                .into(),
        );
        assert!(!is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_ignores_plain_500() {
        let e = SystemError::HardwareFailure("API Error 500 Internal Server Error".into());
        assert!(!is_permanent_model_failure(&e));
    }
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
            // CORE-328: tally under the (model, task) composite too, so the
            // scorer can learn that a model fails THIS kind of task.
            tracker
                .record_failure_for_task(model_id, provider, task)
                .await;
            if let Some(e) = err {
                if is_permanent_model_failure(e) {
                    tracker.record_model_unavailable(model_id).await;
                }
            }
        }
    }

    /// CORE-325: a 200-OK stream with zero content tokens is an implicit
    /// failure — feed both the per-model empty-response circuit and the
    /// failure window so decide() reranks away from the silent model.
    async fn note_empty_response(
        &self,
        model_id: &str,
        provider: &str,
        task: crate::pcb::TaskType,
    ) {
        if let Some(r) = self.router_ref.read().await.clone() {
            let router = r.read().await;
            let tracker = router.tracker_ref();
            tracker.record_empty_response(model_id).await;
            tracker
                .record_failure_for_task(model_id, provider, task)
                .await;
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
            on_rate_limited.clone(),
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

        let (mut messages, build_warnings) = self.build_messages_with_warnings(pcb, persona).await;

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

        for iteration in 0..MAX_ITERATIONS {
            let mut autocorrect_retries = 0;
            let mut local_messages = messages.clone();

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

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
                            if let Some(r) = &tracker {
                                let kp = r.read().await.key_pool_ref();
                                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                let mut tried: std::collections::HashSet<String> =
                                    std::collections::HashSet::new();
                                if let Some(ref kid) = decision.key_id {
                                    tried.insert(kid.clone());
                                }
                                for attempt in 0..3u32 {
                                    let alt = match kp
                                        .get_available_key(
                                            &active_provider,
                                            &active_model_id,
                                            tenant_id,
                                        )
                                        .await
                                    {
                                        Some(k) if !tried.contains(&k.key_id) => k,
                                        _ => break,
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
                                        on_rate_limited.clone(),
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
                                            let wpayload = serde_json::json!({
                                                "category": "key_rotated",
                                                "provider": active_provider,
                                                "attempt": attempt + 1,
                                                "message": format!(
                                                    "La key primaria de {} se agotó; \
                                                     cambié a otra del mismo proveedor.",
                                                    active_provider
                                                ),
                                            });
                                            let _ = text_tx
                                                .send(Ok(format!("__WARNING__{}", wpayload)));
                                            driver = alt_driver;
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

                            // If rotation didn't help AND we have no fallback chain,
                            // surface a user-visible message before bubbling the
                            // original error up — otherwise the chat shows nothing.
                            if recovered.is_none() && decision.fallback_chain.is_empty() {
                                if let Some(r) = &tracker {
                                    let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                                    r.read().await.invalidate_sticky(tenant_id).await;
                                }
                                let summary = summarise_provider_failure(&primary_err);
                                let msg = format!(
                                    "No pude responder con '{}'. {} No tengo modelos \
                                     de respaldo configurados — revisá las API keys \
                                     o agregá otro proveedor.",
                                    active_model_id, summary
                                );
                                let _ = text_tx.send(Ok(msg.clone()));
                                let wpayload = serde_json::json!({
                                    "category": "all_models_failed",
                                    "primary_model": active_model_id,
                                    "primary_provider": active_provider,
                                    "reason": summary,
                                    "message": msg,
                                });
                                let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                                return Err(primary_err);
                            }

                            for fb in &decision.fallback_chain {
                                if recovered.is_some() {
                                    break;
                                }
                                let fb_driver = CloudProxyDriver::new_with_callback(
                                    Arc::clone(&self.http_client),
                                    fb.api_url.clone(),
                                    fb.api_key.clone(),
                                    fb.model_id.clone(),
                                    None,
                                    on_rate_limited.clone(),
                                );
                                match fb_driver
                                    .generate_stream(local_messages.clone(), None, tools.clone())
                                    .await
                                {
                                    Ok(s) => {
                                        active_model_id = fb.model_id.clone();
                                        active_provider = fb.provider.clone();
                                        driver = fb_driver;
                                        send_model_event(
                                            &text_tx,
                                            &active_model_id,
                                            &active_provider,
                                        );
                                        let wpayload = serde_json::json!({
                                            "category": "model_fallback",
                                            "message": format!(
                                                "El modelo primario falló; respondiendo con {}.",
                                                fb.model_id
                                            )
                                        });
                                        let _ =
                                            text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
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

                            match recovered {
                                Some(s) => s,
                                None => {
                                    // All fallbacks failed too. Invalidate sticky so
                                    // the next request re-evaluates from scratch, and
                                    // surface a user-visible message — without this
                                    // the chat appears silent on cascading failures
                                    // (smoke-test symptom: Gemini quota exhausted +
                                    // all OpenRouter fallbacks 401).
                                    if let Some(r) = &tracker {
                                        let tenant_id =
                                            pcb.tenant_id.as_deref().unwrap_or("default");
                                        r.read().await.invalidate_sticky(tenant_id).await;
                                    }
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
                                        active_model_id,
                                        if attempted.is_empty() {
                                            "sin respaldos".to_string()
                                        } else {
                                            attempted.join(", ")
                                        },
                                        summary
                                    );
                                    let _ = text_tx.send(Ok(msg.clone()));
                                    let wpayload = serde_json::json!({
                                        "category": "all_models_failed",
                                        "primary_model": active_model_id,
                                        "primary_provider": active_provider,
                                        "attempted_fallbacks": attempted,
                                        "reason": summary,
                                        "message": msg,
                                    });
                                    let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                                    return Err(primary_err);
                                }
                            }
                        }
                    }
                } else {
                    // Subsequent retries of the autocorrect loop: call driver directly
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
                            return Err(e);
                        }
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
                            assistant_text.push_str(&text);
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Ok(text));
                            }
                        }
                        Err(e) => {
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Err(e.clone()));
                            }
                            return Err(SystemError::HardwareFailure(format!("{}", e)));
                        }
                    }
                }

                // CORE-FIX (A): the smoke test caught cogito-2.1:671b on ollama_cloud
                // returning 200 OK with zero content tokens.
                let stream_was_empty = iteration == 0
                    && autocorrect_retries == 0
                    && assistant_text.is_empty()
                    && tool_calls.is_empty();

                if stream_was_empty {
                    self.note_empty_response(&active_model_id, &active_provider, pcb.task_type)
                        .await;
                    warn!(
                        pid = %pid,
                        model = %active_model_id,
                        "primary returned 0 content tokens — trying key rotation then fallback chain"
                    );

                    let mut recovered = false;

                    // CORE-FIX: same key-rotation logic as the 429 path. If the
                    // current key gave us an empty stream, try other keys of the
                    // SAME (provider, model) before switching models.
                    if let Some(r) = self.router_ref.read().await.clone() {
                        let kp = r.read().await.key_pool_ref();
                        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                        let mut tried: std::collections::HashSet<String> =
                            std::collections::HashSet::new();
                        if let Some(ref kid) = decision.key_id {
                            tried.insert(kid.clone());
                        }
                        for attempt in 0..3u32 {
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
                                on_rate_limited.clone(),
                            );
                            match alt_driver
                                .generate_stream(local_messages.clone(), None, tools.clone())
                                .await
                            {
                                Ok(s) => {
                                    tokio::pin!(s);
                                    let mut alt_text = String::new();
                                    let mut alt_tool_calls: Vec<ToolCallRecord> = Vec::new();
                                    while let Some(tr) = s.next().await {
                                        match tr {
                                            Ok(token) if token.starts_with("__TOOL_CALL__") => {
                                                let _ = text_tx.send(Ok(token.clone()));
                                                let json_str = token
                                                    .strip_prefix("__TOOL_CALL__")
                                                    .unwrap_or_default();
                                                if let Ok(tc) =
                                                    serde_json::from_str::<DriverToolCallPayload>(
                                                        json_str,
                                                    )
                                                {
                                                    alt_tool_calls.push(ToolCallRecord {
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
                                                alt_text.push_str(&text);
                                                let _ = text_tx.send(Ok(text));
                                            }
                                            Err(e) => {
                                                let _ = text_tx.send(Err(e));
                                            }
                                        }
                                    }
                                    if !alt_text.is_empty() || !alt_tool_calls.is_empty() {
                                        let wpayload = serde_json::json!({
                                            "category": "key_rotated",
                                            "provider": active_provider,
                                            "attempt": attempt + 1,
                                            "message": format!(
                                                "El modelo no respondió con la \
                                                 primera key; cambié a otra del \
                                                 mismo proveedor ({}).",
                                                active_provider
                                            ),
                                        });
                                        let _ =
                                            text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                                        driver = alt_driver;
                                        assistant_text = alt_text;
                                        tool_calls = alt_tool_calls;
                                        recovered = true;
                                        break;
                                    } else {
                                        // Alt key also empty — record + try next.
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

                    if recovered {
                        if tool_calls.is_empty() {
                            finished = true;
                            break;
                        }
                    } else {
                        for fb in &decision.fallback_chain {
                            let fb_driver = CloudProxyDriver::new_with_callback(
                                Arc::clone(&self.http_client),
                                fb.api_url.clone(),
                                fb.api_key.clone(),
                                fb.model_id.clone(),
                                None,
                                on_rate_limited.clone(),
                            );
                            match fb_driver
                                .generate_stream(local_messages.clone(), None, tools.clone())
                                .await
                            {
                                Ok(s) => {
                                    tokio::pin!(s);
                                    let mut fb_text = String::new();
                                    let mut fb_tool_calls: Vec<ToolCallRecord> = Vec::new();
                                    while let Some(tr) = s.next().await {
                                        match tr {
                                            Ok(token) if token.starts_with("__TOOL_CALL__") => {
                                                let _ = text_tx.send(Ok(token.clone()));
                                                let json_str = token
                                                    .strip_prefix("__TOOL_CALL__")
                                                    .unwrap_or_default();
                                                if let Ok(tc) =
                                                    serde_json::from_str::<DriverToolCallPayload>(
                                                        json_str,
                                                    )
                                                {
                                                    fb_tool_calls.push(ToolCallRecord {
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
                                                fb_text.push_str(&text);
                                                let _ = text_tx.send(Ok(text));
                                            }
                                            Err(e) => {
                                                let _ = text_tx.send(Err(e));
                                            }
                                        }
                                    }
                                    if !fb_text.is_empty() || !fb_tool_calls.is_empty() {
                                        active_model_id = fb.model_id.clone();
                                        active_provider = fb.provider.clone();
                                        driver = fb_driver;
                                        send_model_event(
                                            &text_tx,
                                            &active_model_id,
                                            &active_provider,
                                        );
                                        let wpayload = serde_json::json!({
                                            "category": "model_fallback",
                                            "message": format!(
                                                "El modelo primario no respondió; cambiamos a {}.",
                                                fb.model_id
                                            )
                                        });
                                        let _ =
                                            text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
                                        assistant_text = fb_text;
                                        tool_calls = fb_tool_calls;
                                        recovered = true;
                                        break;
                                    } else {
                                        // Fallback also returned empty. Record and try next.
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

                    if !recovered {
                        // CORE-FIX (B): emit a user-visible message before bailing,
                        // so the chat doesn't show a silent empty response.
                        let mut max_cooldown: u64 = 0;
                        if let Some(r) = self.router_ref.read().await.clone() {
                            let tr = r.read().await;
                            let tracker = tr.tracker_ref();
                            if let Some(s) = tracker
                                .provider_cooldown_remaining_secs(&active_provider)
                                .await
                            {
                                max_cooldown = max_cooldown.max(s);
                            }
                            for fb in &decision.fallback_chain {
                                if let Some(s) =
                                    tracker.provider_cooldown_remaining_secs(&fb.provider).await
                                {
                                    max_cooldown = max_cooldown.max(s);
                                }
                            }
                            // Invalidate sticky so next request re-evaluates from scratch.
                            let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                            tr.invalidate_sticky(tenant_id).await;
                        }

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
                                 modelos de respaldo configurados. Revisá tu \
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

                        // Visible to the user as assistant text.
                        let _ = text_tx.send(Ok(msg.clone()));
                        // Also surface as a structured warning event for the UI.
                        let wpayload = serde_json::json!({
                            "category": "all_models_failed",
                            "primary_model": active_model_id,
                            "primary_provider": active_provider,
                            "cooldown_secs": max_cooldown,
                            "message": msg.clone(),
                        });
                        let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));

                        drop(msg); // mark used so clippy is happy
                        tool_calls.clear();
                        finished = true;
                        break;
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
                    match autocorrect::validate_and_sanitize_tool_call(tc, &role) {
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

        for _iteration in 0..MAX_ITERATIONS {
            let mut autocorrect_retries = 0;
            let mut local_messages = messages.clone();

            let mut assistant_text = String::new();
            let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

            loop {
                let raw_stream = driver
                    .generate_stream(local_messages.clone(), None, tools.clone())
                    .await?;
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
                            if autocorrect_retries == 0 {
                                let _ = text_tx.send(Err(e.clone()));
                            }
                            return Err(SystemError::HardwareFailure(format!("{}", e)));
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

    /// CORE-276 (autonomous mode): whether the project the calling agent belongs
    /// to is marked autonomous — i.e. the user opted this project into skipping
    /// the external-path approval gate (full filesystem access, no per-path
    /// prompts). Resolves the agent's project via the orchestrator tree, then
    /// checks the tenant enclave. Defaults to false (locked down) on any miss.
    async fn is_project_autonomous(&self, pcb: &PCB) -> bool {
        let agent_id = match pcb.agent_id {
            Some(id) => id,
            None => return false,
        };
        let orchestrator = match self.agent_orchestrator.read().await.clone() {
            Some(o) => o,
            None => return false,
        };
        let project_id = {
            let tree = orchestrator.tree.read().await;
            match tree.get(&agent_id) {
                Some(node) => node.project_id.clone(),
                None => return false,
            }
        };
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let session_key = pcb.session_key.as_deref().unwrap_or("");
        match crate::enclave::TenantDB::open(tenant_id, session_key) {
            Ok(db) => db.is_project_autonomous(&project_id),
            Err(_) => false,
        }
    }

    fn strip_unc_prefix(path: &std::path::Path) -> std::path::PathBuf {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            if let Some(unc_stripped) = stripped.strip_prefix("UNC\\") {
                std::path::PathBuf::from(format!(r"\\{}", unc_stripped))
            } else {
                std::path::PathBuf::from(stripped)
            }
        } else {
            path.to_path_buf()
        }
    }

    fn resolve_path(
        workspace: &std::path::Path,
        input_path: &str,
        approved_paths: &[String],
        autonomous: bool,
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

        // CORE-FIX: canonicalize workspace BEFORE comparing. Without this, a
        // workspace that contains a symlink anywhere in its path (e.g. /var/lib
        // is a symlink to /mnt/data/lib on the host) produces a canonical
        // candidate under /mnt/data/lib/... that does NOT start_with the
        // original /var/lib/... — and we'd incorrectly reject legitimate paths.
        let workspace_canonical = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());

        let canonical_clean = Self::strip_unc_prefix(&canonical);
        let workspace_canonical_clean = Self::strip_unc_prefix(&workspace_canonical);

        if canonical_clean.starts_with(&workspace_canonical_clean) {
            return Ok(canonical);
        }

        // Autonomous project: the user opted this project into full filesystem
        // access, so external paths are allowed without per-path approval.
        if autonomous {
            return Ok(canonical);
        }

        // Path externo — verificar aprobaciones
        let canonical_str = canonical_clean.to_string_lossy().to_string();
        let approved = approved_paths.iter().any(|a| {
            let approved_clean = Self::strip_unc_prefix(std::path::Path::new(a));
            canonical_clean.starts_with(&approved_clean)
        });

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
                        let name = args["name"].as_str().map(String::from);
                        let scope = args["scope"].as_str().unwrap_or("").to_string();
                        let task_type_str = args["task_type"].as_str().unwrap_or("planning");
                        let task_type = Some(crate::syscalls::parse_task_type(task_type_str));

                        if let Some(caller_id) = pcb.agent_id {
                            let role_str = args["role"].as_str().unwrap_or("").to_string();
                            let agent_role = match role_str.to_lowercase().as_str() {
                                "project_supervisor" => {
                                    crate::agents::node::AgentRole::ProjectSupervisor {
                                        name: name.clone().unwrap_or_else(|| scope.clone()),
                                        description: scope.clone(),
                                    }
                                }
                                "supervisor" => crate::agents::node::AgentRole::Supervisor {
                                    name: name.clone().unwrap_or_else(|| scope.clone()),
                                    scope: scope.clone(),
                                },
                                _ => crate::agents::node::AgentRole::Specialist {
                                    scope: scope.clone(),
                                },
                            };

                            let call = crate::agents::message::AgentToolCall::Spawn {
                                role: agent_role,
                                name,
                                scope,
                                task_type,
                            };

                            match orchestrator.handle_tool_call(caller_id, call).await {
                                Ok(result) => result,
                                Err(e) => format!("{{\"error\":\"{}\"}}", e),
                            }
                        } else {
                            let project_name = name
                                .clone()
                                .unwrap_or_else(|| scope.chars().take(40).collect());

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
                    // CORE-253: mensaje legible al usuario cuando el plugin no existe.
                    Err(crate::plugins::PluginError::FunctionNotFound(_)) => format!(
                        "El plugin '{}' no está instalado o no está activo en este tenant. \
                         Podés activarlo desde Configuración → Plugins.",
                        plugin_name
                    ),
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

                // Autonomous project: don't pause for the user. Auto-approve so the
                // supervisor proceeds without per-question permission prompts — the
                // behavioural half of "omitir permisos por proyecto" (the gate
                // bypass in resolve_path is the filesystem half).
                if self.is_project_autonomous(pcb).await {
                    tracing::info!(
                        agent = %agent_uuid,
                        question = %question,
                        "ask_user auto-approved — project is in autonomous mode"
                    );
                    return serde_json::json!({
                        "user_answer": "Modo autónomo activo en este proyecto: tenés acceso completo, procedé sin pedir permiso."
                    })
                    .to_string();
                }

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

                // CORE-268: notificar a la UI que el supervisor necesita respuesta.
                // Guardamos también el evento para re-enviarlo si el usuario
                // reconecta el WebSocket mientras el supervisor sigue esperando.
                let question_event = crate::agents::event::AgentEvent::SupervisorQuestion {
                    agent_id: agent_uuid,
                    project_name: project_name.clone(),
                    question: question.clone(),
                    context: context.clone(),
                    timestamp: chrono::Utc::now(),
                };
                orchestrator
                    .set_pending_question(agent_uuid, question_event.clone())
                    .await;
                orchestrator.emit_event(question_event);

                match tokio::time::timeout(std::time::Duration::from_secs(600), reply_rx).await {
                    Ok(Ok(answer)) => {
                        orchestrator.clear_pending_question(&agent_uuid).await;
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
                        orchestrator.clear_pending_question(&agent_uuid).await;
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
                let autonomous = self.is_project_autonomous(pcb).await;

                let resolved =
                    match Self::resolve_path(&workspace, &input_path, &approved, autonomous) {
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
                let autonomous = self.is_project_autonomous(pcb).await;

                // write_file stays inside the workspace — unless the project is
                // autonomous, in which case external writes are allowed too.
                let resolved = match Self::resolve_path(&workspace, &input_path, &[], autonomous) {
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
                let autonomous = self.is_project_autonomous(pcb).await;

                let resolved =
                    match Self::resolve_path(&workspace, input_path, &approved, autonomous) {
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

            // CORE-289: Chat Agent consulta el estado actual del árbol de agentes
            // CORE-300: filtrar por tenant_id del PCB — aislamiento cross-tenant.
            "get_agent_status" => {
                let project_name = args
                    .get("project_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
                };

                let tenant_id = pcb
                    .tenant_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string());
                let snapshot = orchestrator.tree_snapshot_for_tenant(&tenant_id).await;

                let filtered: Vec<_> = if project_name.is_empty() {
                    snapshot
                } else {
                    snapshot
                        .into_iter()
                        .filter(|n| {
                            n.project_id
                                .to_lowercase()
                                .contains(&project_name.to_lowercase())
                                || n.role_label
                                    .to_lowercase()
                                    .contains(&project_name.to_lowercase())
                        })
                        .collect()
                };

                if filtered.is_empty() {
                    return serde_json::json!({
                        "status": "no_active_agents",
                        "project": project_name,
                        "message": "No active agents for this project. You can spawn a new supervisor."
                    })
                    .to_string();
                }

                serde_json::to_string(&serde_json::json!({
                    "agents": filtered.iter().map(|n| serde_json::json!({
                        "role": n.role_label,
                        "state": n.state,
                        "project": n.project_id,
                        "last_report": n.last_report,
                        "degraded": n.degraded,
                    })).collect::<Vec<_>>()
                }))
                .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string())
            }

            // CORE-FIX: Antes no existía handler para `report` — la tool definida en
            // ToolRegistry caía en el fallback "Unknown tool: report" y los agentes
            // del árbol no podían reportar status=error|blocked al supervisor padre.
            // Ahora delegamos a AgentOrchestrator::handle_tool_call que ya tiene la
            // lógica para setear AgentState y last_report en el nodo.
            "report" => {
                let caller_id = match pcb.agent_id {
                    Some(id) => id,
                    None => {
                        return "{\"error\":\"report requires agent_id in PCB (only tree agents can report)\"}".to_string();
                    }
                };
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let status_str = args["status"].as_str().unwrap_or("completed");
                let status = match status_str.to_lowercase().as_str() {
                    "error" => crate::agents::message::ToolCallReportStatus::Error,
                    "blocked" => crate::agents::message::ToolCallReportStatus::Blocked,
                    _ => crate::agents::message::ToolCallReportStatus::Completed,
                };
                let summary = args["summary"].as_str().unwrap_or("").to_string();
                let observations = args
                    .get("observations")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let call = crate::agents::message::AgentToolCall::Report {
                    status,
                    summary,
                    observations,
                };
                match orchestrator.handle_tool_call(caller_id, call).await {
                    Ok(result) => result,
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            // CORE-FIX: Specialists declaran "Verify the result (build, test, lint)"
            // en su prompt pero antes no tenían ninguna tool para hacerlo. Esta tool
            // permite ejecutar comandos de verificación con una whitelist estricta,
            // timeout de 60s y output truncado a 8KB. No es un shell general.
            "execute_command" => {
                const ALLOWED_PROGRAMS: &[&str] = &[
                    "cargo", "rustc", "npm", "pnpm", "yarn", "git", "python", "python3", "pytest",
                    "node", "deno", "bun", "go", "gradle", "mvn", "make", "ls", "echo", "pwd",
                    "cat", "head", "tail", "protoc",
                ];
                let timeout_secs = std::env::var("AEGIS_COMMAND_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(300); // Generous 5-minute default
                const MAX_OUTPUT_BYTES: usize = 8 * 1024;

                let command = args["command"].as_str().unwrap_or("").trim().to_string();
                if command.is_empty() {
                    return "{\"error\":\"empty_command\"}".to_string();
                }

                let program = command.split_whitespace().next().unwrap_or("");
                if !ALLOWED_PROGRAMS.contains(&program) {
                    return format!(
                        "{{\"error\":\"command_not_allowed\",\"detail\":\"'{}' is not whitelisted\",\"allowed\":{}}}",
                        program,
                        serde_json::to_string(ALLOWED_PROGRAMS).unwrap_or_else(|_| "[]".to_string())
                    );
                }

                let mut cmd_str = command.clone();
                if cfg!(windows) {
                    // Quick translation of common Unix shell commands to Windows CMD builtins
                    if cmd_str.starts_with("ls ") || cmd_str == "ls" {
                        cmd_str = cmd_str.replace("ls", "dir");
                    }
                    if cmd_str.starts_with("cat ") {
                        cmd_str = cmd_str.replace("cat", "type");
                    }
                    if cmd_str.starts_with("rm -rf ") {
                        let target = cmd_str.strip_prefix("rm -rf ").unwrap_or("").trim();
                        cmd_str = format!("rmdir /s /q {}", target);
                    }
                    if cmd_str == "pwd" {
                        cmd_str = "cd".to_string();
                    }
                    if cmd_str == "clear" {
                        cmd_str = "cls".to_string();
                    }
                }

                // --- Auto-installation check on Linux ---
                if cfg!(target_os = "linux") {
                    fn check_tool_exists(prog: &str) -> bool {
                        std::process::Command::new("sh")
                            .args(["-c", &format!("command -v {}", prog)])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or_else(|_| {
                                std::process::Command::new("which")
                                    .arg(prog)
                                    .output()
                                    .map(|o| o.status.success())
                                    .unwrap_or(false)
                            })
                    }

                    fn install_tool_on_linux(prog: &str) {
                        let package = match prog {
                            "git" => "git",
                            "protoc" | "protobuf" => "protobuf-compiler",
                            "node" | "npm" => "nodejs npm",
                            "cargo" | "rustc" => "rustc cargo",
                            "make" | "gcc" => "build-essential pkg-config libssl-dev",
                            _ => return,
                        };

                        println!("[AEGIS] La herramienta '{}' no esta instalada. Intentando instalar el paquete '{}' de forma rapida...", prog, package);

                        // Try fast apt-get install directly first (saves 10-20 seconds!)
                        let fast_cmd = format!("apt-get install -y {}", package);
                        let output = std::process::Command::new("sh")
                            .args(["-c", &fast_cmd])
                            .output();

                        if let Ok(ref out) = output {
                            if out.status.success() {
                                println!(
                                    "[AEGIS] Instale con exito '{}' de forma rapida.",
                                    package
                                );
                                return;
                            }
                        }

                        // Fallback: update package list and try again
                        println!("[AEGIS] Intento rapido fallido. Actualizando repositorio e instalando '{}'...", package);
                        let cmd = format!("apt-get update && apt-get install -y {}", package);
                        let output = std::process::Command::new("sh").args(["-c", &cmd]).output();

                        if let Ok(ref out) = output {
                            if out.status.success() {
                                println!(
                                    "[AEGIS] Instale con exito '{}' via apt-get con actualizacion.",
                                    package
                                );
                                return;
                            }
                        }

                        // Try with sudo if previous attempts failed
                        let sudo_cmd =
                            format!("sudo apt-get update && sudo apt-get install -y {}", package);
                        let _ = std::process::Command::new("sh")
                            .args(["-c", &sudo_cmd])
                            .output();
                    }

                    // Check primary program
                    let exists = if program == "cargo" || program == "rustc" {
                        check_tool_exists("cargo") && check_tool_exists("rustc")
                    } else {
                        check_tool_exists(program)
                    };

                    if !exists {
                        install_tool_on_linux(program);
                    }

                    // Check protobuf compiler if program is cargo or make
                    if (program == "cargo" || program == "make") && !check_tool_exists("protoc") {
                        install_tool_on_linux("protoc");
                    }

                    // Check gcc/make compiler tools if program is cargo or make
                    if (program == "cargo" || program == "make")
                        && (!check_tool_exists("gcc") || !check_tool_exists("make"))
                    {
                        install_tool_on_linux("make");
                    }
                }

                let workspace = Self::get_tenant_workspace(pcb);
                // CORE-FIX: make sure the workspace exists before we try to run
                // anything in it — otherwise a specialist's first command
                // (typically `git clone …` on a brand-new tenant) fails at
                // spawn time with "No such file or directory" because
                // current_dir() points at a path that was never created.
                if let Err(e) = tokio::fs::create_dir_all(&workspace).await {
                    return format!(
                        "{{\"error\":\"workspace_unavailable\",\"detail\":\"{}\"}}",
                        e
                    );
                }
                let cwd_arg = args.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
                let cwd = match Self::resolve_path(&workspace, cwd_arg, &[], false) {
                    Ok(p) if p.is_dir() => p,
                    _ => workspace.clone(),
                };

                // OPTIMIZATION: Configure Shared Cargo Target Directory cache to speed up subagent rust checks from minutes to seconds
                let central_target = workspace
                    .parent()
                    .unwrap_or(&workspace)
                    .join("cargo_target_cache");

                let output_result = if cfg!(windows) {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        tokio::process::Command::new("cmd")
                            .args(["/C", cmd_str.as_str()])
                            .current_dir(&cwd)
                            .env("CARGO_TARGET_DIR", &central_target)
                            .output(),
                    )
                    .await
                } else {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        tokio::process::Command::new("sh")
                            .args(["-c", cmd_str.as_str()])
                            .current_dir(&cwd)
                            .env("CARGO_TARGET_DIR", &central_target)
                            .output(),
                    )
                    .await
                };

                fn truncate_output(s: String, max: usize) -> String {
                    if s.len() > max {
                        let half = max / 2;
                        let head: String = s.chars().take(half).collect();
                        let tail: String = s
                            .chars()
                            .skip(s.chars().count().saturating_sub(half))
                            .collect();
                        format!("{}\n\n[...truncated...]\n\n{}", head, tail)
                    } else {
                        s
                    }
                }

                match output_result {
                    Ok(Ok(output)) => {
                        let stdout = truncate_output(
                            String::from_utf8_lossy(&output.stdout).to_string(),
                            MAX_OUTPUT_BYTES,
                        );
                        let stderr = truncate_output(
                            String::from_utf8_lossy(&output.stderr).to_string(),
                            MAX_OUTPUT_BYTES,
                        );
                        serde_json::json!({
                            "exit_code": output.status.code(),
                            "stdout": stdout,
                            "stderr": stderr,
                            "cwd": cwd.to_string_lossy(),
                        })
                        .to_string()
                    }
                    Ok(Err(e)) => {
                        format!("{{\"error\":\"spawn_failed\",\"detail\":\"{}\"}}", e)
                    }
                    Err(_) => format!(
                        "{{\"error\":\"timeout\",\"detail\":\"command exceeded {}s\"}}",
                        timeout_secs
                    ),
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
        let mut warnings: Vec<String> = Vec::new();

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
                warnings.push(format!(
                    "Context assembly failed; responding with reduced memory. Detail: {}",
                    e
                ));
                pcb.memory_pointers.l1_instruction.clone()
            });

        let tool_prompt = self
            .plugin_manager
            .read()
            .await
            .get_available_tools_prompt();
        let mcp_tool_prompt = self.mcp_registry.generate_system_prompt().await;

        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let session_key = pcb.session_key.as_deref().unwrap_or("default");

        let modules_tool_prompt = if let Some(ref router_lock) = *self.router.read().await {
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
        };

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
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
            let dynamic_scripts = Self::get_dynamic_scripts_section(tenant_id).await;
            format!("{}{}", MAKER_INSTRUCTIONS, dynamic_scripts)
        } else {
            "".to_string()
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

    #[test]
    fn test_strip_unc_prefix() {
        let p1 = std::path::Path::new(r"\\?\C:\foo\bar");
        let p2 = std::path::Path::new(r"\\?\UNC\server\share\foo");
        let p3 = std::path::Path::new(r"C:\foo\bar");

        assert_eq!(
            CognitiveHAL::strip_unc_prefix(p1),
            std::path::PathBuf::from(r"C:\foo\bar")
        );
        assert_eq!(
            CognitiveHAL::strip_unc_prefix(p2),
            std::path::PathBuf::from(r"\\server\share\foo")
        );
        assert_eq!(
            CognitiveHAL::strip_unc_prefix(p3),
            std::path::PathBuf::from(r"C:\foo\bar")
        );
    }

    #[test]
    fn test_resolve_path_unc_and_nonexistent() {
        let temp_dir = std::env::temp_dir();
        let ws_dir = temp_dir.join("aegis_test_ws");
        let _ = std::fs::create_dir_all(&ws_dir);

        let existing_sub = ws_dir.join("existing_folder");
        let _ = std::fs::create_dir_all(&existing_sub);

        let new_file = "existing_folder/new_file.txt";

        let resolved = CognitiveHAL::resolve_path(&ws_dir, new_file, &[], false);
        assert!(resolved.is_ok());
        let res_path = resolved.unwrap();
        assert!(res_path.to_string_lossy().contains("new_file.txt"));

        let _ = std::fs::remove_dir_all(&ws_dir);
    }

    #[test]
    fn test_autocorrect_typo_correction() {
        use super::autocorrect::normalize_tool_name;
        assert_eq!(normalize_tool_name("spawnagent"), "spawn_agent");
        assert_eq!(normalize_tool_name("writefile"), "write_file");
        assert_eq!(normalize_tool_name("run_command"), "execute_command");
        assert_eq!(normalize_tool_name("websearch"), "web_search");
    }

    #[test]
    fn test_autocorrect_balance_braces() {
        use super::autocorrect::balance_braces;
        assert_eq!(balance_braces("{\"a\": 1"), "{\"a\": 1}");
        assert_eq!(balance_braces("{\"a\": [1, 2"), "{\"a\": [1, 2]}");
        assert_eq!(balance_braces("{\"a\": \"val"), "{\"a\": \"val\"}");
    }

    #[test]
    fn test_autocorrect_sanitize_json_string() {
        use super::autocorrect::sanitize_json_string;
        assert_eq!(
            sanitize_json_string("```json\n{\"a\": 1}\n```"),
            "{\"a\": 1}"
        );
        assert_eq!(
            sanitize_json_string("  ```\n{\"a\": 1\n```  "),
            "{\"a\": 1}"
        );
    }

    #[test]
    fn test_validate_and_sanitize_tool_call() {
        use super::autocorrect::validate_and_sanitize_tool_call;
        use crate::agents::node::AgentRole;
        use crate::chal::FunctionCallRecord;
        use crate::chal::ToolCallRecord;

        // Valid spawn_agent call with coercion and typo fix
        let tc = ToolCallRecord {
            id: "call-123".to_string(),
            type_: "function".to_string(),
            function: FunctionCallRecord {
                name: "spawnagent".to_string(),
                arguments: "```json\n{\n  \"name\": \"Test Supervisor\",\n  \"scope\": \"Test Scope\",\n  \"task_type\": \"planning\",\n  \"role\": \"supervisor\"\n}\n```".to_string(),
            },
        };

        let res = validate_and_sanitize_tool_call(&tc, &AgentRole::ChatAgent);
        assert!(res.is_ok(), "Should successfully validate: {:?}", res.err());
        let sanitized = res.unwrap();
        assert_eq!(sanitized.function.name, "spawn_agent");

        let args: serde_json::Value = serde_json::from_str(&sanitized.function.arguments).unwrap();
        assert_eq!(args["name"], "Test Supervisor");
        assert_eq!(args["role"], "supervisor");

        // Specialist tool call with type coercion: write_file
        let tc_spec = ToolCallRecord {
            id: "call-456".to_string(),
            type_: "function".to_string(),
            function: FunctionCallRecord {
                name: "writefile".to_string(), // Typo fix
                arguments:
                    "{\"path\": \"test.txt\", \"content\": \"hello\", \"mode\": \"rewrite\"}"
                        .to_string(),
            },
        };
        let res_spec = validate_and_sanitize_tool_call(
            &tc_spec,
            &AgentRole::Specialist {
                scope: "code".to_string(),
            },
        );
        assert!(res_spec.is_ok(), "Should validate: {:?}", res_spec.err());
        let sanitized_spec = res_spec.unwrap();
        assert_eq!(sanitized_spec.function.name, "write_file");

        // Expect failure when non-permitted tool is used
        let tc_invalid = ToolCallRecord {
            id: "call-789".to_string(),
            type_: "function".to_string(),
            function: FunctionCallRecord {
                name: "execute_command".to_string(),
                arguments: "{\"command\": \"ls\"}".to_string(),
            },
        };
        let res_invalid = validate_and_sanitize_tool_call(&tc_invalid, &AgentRole::ChatAgent);
        assert!(res_invalid.is_err(), "ChatAgent cannot run execute_command");
    }

    #[test]
    fn test_sanitize_prompt_content() {
        use super::autocorrect::sanitize_prompt_content;

        // OpenAI key
        let text1 =
            "My key is sk-123456789012345678901234567890123456789012345678 and it is secret.";
        assert_eq!(
            sanitize_prompt_content(text1),
            "My key is [REDACTED_OPENAI_KEY] and it is secret."
        );

        // OpenRouter key
        let text2 = format!("OpenRouter key: sk-or-v1-{}", "a".repeat(64));
        assert_eq!(
            sanitize_prompt_content(&text2),
            "OpenRouter key: [REDACTED_OPENROUTER_KEY]"
        );

        // Gemini key
        let text3 = "Gemini AIzaSyAbC123_def456-ghi789_jkl123-mno45";
        assert_eq!(
            sanitize_prompt_content(text3),
            "Gemini [REDACTED_GEMINI_KEY]"
        );

        // Anthropic key
        let text4 = "Anthropic: sk-ant-sid01-123456789012345678901234567890123456789012345678901234567890abc123456789012345678901234567890";
        assert_eq!(
            sanitize_prompt_content(text4),
            "Anthropic: [REDACTED_ANTHROPIC_KEY]"
        );

        // Private Key
        let text5 = "Here is a key:\n-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0y...\n-----END RSA PRIVATE KEY-----\nEnd of text.";
        assert_eq!(
            sanitize_prompt_content(text5),
            "Here is a key:\n[REDACTED_PRIVATE_KEY]\nEnd of text."
        );

        // Database Password
        let text6 =
            "Connect using postgres://admin:super_secret_password@localhost:5432/mydb and enjoy!";
        assert_eq!(
            sanitize_prompt_content(text6),
            "Connect using postgres://admin:[REDACTED_PASSWORD]@localhost:5432/mydb and enjoy!"
        );
    }

    #[test]
    fn test_is_light_task() {
        use super::autocorrect::is_light_task;
        use crate::pcb::TaskType;

        // Chat but very short -> true
        assert!(is_light_task("hola", TaskType::Chat));
        assert!(is_light_task("hi", TaskType::Chat));
        assert!(is_light_task("what is the status?", TaskType::Chat));

        // Code and short -> false (not TaskType::Chat)
        assert!(!is_light_task("hola", TaskType::Code));

        // Chat and very long -> false
        let long_prompt = "A".repeat(150);
        assert!(!is_light_task(&long_prompt, TaskType::Chat));
    }

    #[tokio::test]
    async fn test_dynamic_maker_prompt_injection() -> anyhow::Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join(format!("aegis_maker_test_{}", uuid::Uuid::new_v4()));
        let workspace_dir = test_dir.join("users").join("test_tenant").join("workspace");
        std::fs::create_dir_all(&workspace_dir)?;

        // Define dynamic script inside workspace
        let script_path = workspace_dir.join("my_custom_tool.js");
        let script_content = r#"// Description: Multiplica dos números usando sandbox
// Parameters: {"a": 2, "b": 3}
function multiply(a, b) {
    return a * b;
}
"#;
        std::fs::write(&script_path, script_content)?;

        // Set env variable
        std::env::set_var("AEGIS_DATA_DIR", &test_dir);

        // Call the helper directly
        let section = CognitiveHAL::get_dynamic_scripts_section("test_tenant").await;

        // Clean up environment
        std::env::remove_var("AEGIS_DATA_DIR");
        let _ = std::fs::remove_dir_all(&test_dir);

        assert!(section.contains("DYNAMIC SCRIPTS"));
        assert!(section.contains("my_custom_tool.js"));
        assert!(section.contains("Multiplica dos números usando sandbox"));
        assert!(section.contains("[SYS_CALL_MAKER(\"js\", \"return eval(read_file('my_custom_tool.js'))\", {\"a\": 2, \"b\": 3})]"));

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_command_configurable_timeout() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test_tenant".into(), 5, "hola".into());

        // 1. Test standard success command with a moderate timeout
        std::env::set_var("AEGIS_COMMAND_TIMEOUT", "10");
        let cmd = ToolCallRecord {
            id: "call-123".to_string(),
            type_: "function".to_string(),
            function: FunctionCallRecord {
                name: "execute_command".to_string(),
                arguments: "{\"command\": \"echo test_ok\"}".to_string(),
            },
        };
        let res = hal
            .execute_tool_call_internal(&cmd, "some_agent", &pcb)
            .await;
        assert!(res.contains("test_ok"));

        // 2. Test timeout trigger by setting a very short timeout (1s)
        // and running a command that is slow (using whitelisted python)
        std::env::set_var("AEGIS_COMMAND_TIMEOUT", "1");

        let cmd_slow = ToolCallRecord {
            id: "call-456".to_string(),
            type_: "function".to_string(),
            function: FunctionCallRecord {
                name: "execute_command".to_string(),
                arguments: "{\"command\": \"git clone http://10.255.255.1/repo.git\"}".to_string(),
            },
        };
        let res_slow = hal
            .execute_tool_call_internal(&cmd_slow, "some_agent", &pcb)
            .await;
        assert!(
            res_slow.contains("timeout") || res_slow.contains("exceeded 1s"),
            "res_slow was: {}",
            res_slow
        );

        std::env::remove_var("AEGIS_COMMAND_TIMEOUT");
        Ok(())
    }
}
