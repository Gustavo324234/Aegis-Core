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
        || lower.contains("límite de tokens")
        || lower.contains("limite de tokens")
        || lower.contains("límite de solicitudes")
        || lower.contains("limite de solicitudes")
        || lower.contains("límite de cuota")
        || lower.contains("limite de cuota")
        || lower.contains("reintentá en ~")
        || lower.contains("reintenta en ~")
    {
        return "Te quedaste sin cuota gratuita o sos rate-limited (HTTP 429). \
                Esperá unos segundos o agregá billing al proveedor."
            .to_string();
    }
    if lower.contains("503")
        || lower.contains("502")
        || lower.contains("529")
        || lower.contains("overloaded")
        || lower.contains("service unavailable")
    {
        return "El proveedor está temporalmente sobrecargado o no disponible (HTTP 5xx)."
            .to_string();
    }
    if lower.contains("404") {
        return "El modelo no existe o el endpoint está mal configurado \
                (HTTP 404)."
            .to_string();
    }
    if lower.contains("400") || lower.contains("bad request") {
        return "El proveedor rechazó la solicitud (HTTP 400). Verificá que el modelo seleccionado sea compatible con chat y que la API key tenga permisos para invocarlo."
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
    // Deprecated/removed model IDs (e.g. Google AI Studio 404 "no longer available to new users")
    if lower.contains("404")
        && (lower.contains("not found")
            || lower.contains("no longer available")
            || lower.contains("not supported for generatecontent"))
    {
        return true;
    }
    false
}

/// Errors that belong to a specific model or credit quota on that model rather
/// than an outage of the entire provider (402 credits, 404 deprecated model,
/// 403 plan-gated model, 429 tier-0 limit:0). Recording these at model scope
/// avoids tripping `provider_failures` across healthy sibling models/keys.
fn is_model_scoped_failure(err: &SystemError) -> bool {
    if is_permanent_model_failure(err) {
        let lower = err.to_string().to_lowercase();
        // Keep 401 unauthorized at key/provider scope unless explicitly per-model.
        if !(lower.contains("401") || lower.contains("unauthorized")) {
            return true;
        }
    }
    let lower = err.to_string().to_lowercase();
    lower.contains("402")
        || lower.contains("payment required")
        || lower.contains("requires more credits")
        || lower.contains("can only afford")
        || lower.contains("not a valid model")
}

/// Summarizes configured API keys cleanly for `manage_ai_provider("list")`:
/// deduplicates identical key configs into `key_count`, and filters catalog
/// previews on free-tier keys so paid models (e.g. `claude-sonnet-4-6`,
/// `openrouter/auto`) are never advertised as available on a free key.
pub(crate) fn summarize_configured_providers<I>(
    keys: I,
    catalog_models: &[crate::router::catalog::ModelEntry],
) -> Vec<serde_json::Value>
where
    I: IntoIterator<Item = crate::router::key_pool::ApiKeyEntry>,
{
    let mut summary: Vec<serde_json::Value> = Vec::new();
    let mut sig_to_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for k in keys {
        let mut sorted_active = k.active_models.clone().unwrap_or_default();
        sorted_active.sort();
        let sig = format!("{}:{}:{:?}", k.provider, k.is_free_tier, sorted_active);
        if let Some(&idx) = sig_to_idx.get(&sig) {
            if let Some(obj) = summary.get_mut(idx).and_then(|v| v.as_object_mut()) {
                let next = obj.get("key_count").and_then(|c| c.as_u64()).unwrap_or(1) + 1;
                obj.insert("key_count".to_string(), serde_json::json!(next));
            }
            continue;
        }

        let mut matching_entries: Vec<&crate::router::catalog::ModelEntry> = catalog_models
            .iter()
            .filter(|m| {
                let matches_provider = if k.provider == "openrouter" {
                    m.provider == "openrouter" || (!m.is_local && m.model_id.contains('/'))
                } else {
                    m.provider.eq_ignore_ascii_case(&k.provider)
                };
                if !matches_provider {
                    return false;
                }
                if k.is_free_tier {
                    m.model_id != "openrouter/auto"
                        && (m.free_tier_eligible
                            || m.model_id.ends_with(":free")
                            || (m.cost_input_per_mtok == 0.0 && m.cost_output_per_mtok == 0.0))
                } else {
                    true
                }
            })
            .collect();

        matching_entries.sort_by(|a, b| {
            let a_or_free = a.model_id == "openrouter/free";
            let b_or_free = b.model_id == "openrouter/free";
            let a_free = a.free_tier_eligible || a.model_id.ends_with(":free");
            let b_free = b.free_tier_eligible || b.model_id.ends_with(":free");
            b_or_free
                .cmp(&a_or_free)
                .then_with(|| b_free.cmp(&a_free))
                .then_with(|| a.cost_input_per_mtok.total_cmp(&b.cost_input_per_mtok))
                .then_with(|| a.model_id.cmp(&b.model_id))
        });

        let matching_catalog: Vec<String> = matching_entries
            .into_iter()
            .map(|m| m.model_id.clone())
            .collect();
        let models_preview = match k.active_models {
            Some(ref list) if !list.is_empty() => {
                if k.is_free_tier && k.provider == "openrouter" {
                    let filtered: Vec<String> = list
                        .iter()
                        .filter(|id| {
                            let id_l = id.to_lowercase();
                            id_l == "openrouter/free" || id_l.ends_with(":free")
                        })
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        matching_catalog.iter().take(15).cloned().collect()
                    } else {
                        filtered
                    }
                } else {
                    list.clone()
                }
            }
            _ => matching_catalog.iter().take(15).cloned().collect(),
        };

        sig_to_idx.insert(sig, summary.len());
        summary.push(serde_json::json!({
            "provider": k.provider,
            "models": models_preview,
            "catalog_total_models": matching_catalog.len(),
            "is_active": k.is_active,
            "is_free_tier": k.is_free_tier,
            "label": k.label,
            "key_count": 1u64,
        }));
    }

    summary
}

/// Returns true only for short conversational greetings/acknowledgments on a clean
/// root chat PCB so we skip injecting heavy tool schemas, VCM wrappers, and plugin docs.
fn is_casual_chat_turn(pcb: &PCB) -> bool {
    if pcb.task_type != crate::pcb::TaskType::Chat
        || pcb.agent_id.is_some()
        || !pcb.memory_pointers.l2_context_refs.is_empty()
        || !pcb.memory_pointers.swap_refs.is_empty()
    {
        return false;
    }
    let cleaned: String = pcb
        .memory_pointers
        .l1_instruction
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '!' | '?' | '.' | ',' | '¡' | '¿'))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.len() > 28 {
        return false;
    }
    match trimmed {
        "hola" | "holi" | "holis" | "buenas" | "buen dia" | "buen día" | "buenos dias"
        | "buenos días" | "buenas tardes" | "buenas noches" | "hello" | "hi" | "hey"
        | "que tal" | "qué tal" | "como estas" | "cómo estás" | "todo bien" | "gracias"
        | "muchas gracias" | "chau" | "adios" | "adiós" => true,
        // Confirmations like "dale" / "ok" may approve a pending tool workflow when there is prior history.
        "ok" | "okey" | "dale" | "genial" | "perfecto" => pcb.message_history.is_empty(),
        _ => false,
    }
}

fn collect_rendered_video_card_ids(tool_name: &str, result_json: &str, out: &mut Vec<String>) {
    if !matches!(
        tool_name,
        "video_render_mp4" | "video_produce_auto" | "video_create_project" | "video_get_overview"
    ) {
        return;
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(result_json) else {
        return;
    };
    let mut push_if_rendered = |proj: &serde_json::Value| {
        let has_mp4 = proj
            .get("output_mp4")
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if has_mp4 {
            if let Some(pid) = proj.get("id").and_then(|v| v.as_str()) {
                if !pid.trim().is_empty() && !out.iter().any(|existing| existing == pid) {
                    out.push(pid.to_string());
                }
            }
        }
    };
    if let Some(proj) = val.get("project") {
        push_if_rendered(proj);
    }
    if tool_name == "video_get_overview" {
        if let Some(arr) = val.get("projects").and_then(|v| v.as_array()) {
            // Attach only the latest rendered project to avoid flooding unrelated history.
            if let Some(first_rendered) = arr.iter().find(|p| {
                p.get("output_mp4")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
            }) {
                push_if_rendered(first_rendered);
            }
        }
    }
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
            "Alcanzaste el límite de tokens por minuto (TPM) del modelo. Reintentá en ~17s.",
            "Alcanzaste el límite de solicitudes (RPM) de tu plan. Reintentá en ~12s.",
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum SanitizerMode {
    #[default]
    Normal,
    InTagPrefix,
    InThink,
    InThought,
    InEndThinkPrefix,
    InEndThoughtPrefix,
    InSpecialToken,
}

/// Stateful per-stream text sanitizer that strips leaked reasoning blocks
/// (`<think>...</think>`, `<thought>...</thought>`) and special control tokens
/// (`<|call|>`, `<|im_end|>`, `<|channel|>`, etc.) across arbitrary TCP/SSE chunk splits.
#[derive(Debug, Default)]
struct StreamTextSanitizer {
    mode: SanitizerMode,
    buf: String,
}

impl StreamTextSanitizer {
    fn push(&mut self, chunk: &str) -> String {
        let mut out = String::new();
        for ch in chunk.chars() {
            if ch == '\0' || ch == '\u{FFFD}' {
                continue;
            }
            match self.mode {
                SanitizerMode::Normal => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InTagPrefix;
                    } else {
                        out.push(ch);
                    }
                }
                SanitizerMode::InTagPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "<|" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InSpecialToken;
                    } else if lc == "<think>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThink;
                    } else if lc == "<thought>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThought;
                    } else if "<think>".starts_with(&lc)
                        || "<thought>".starts_with(&lc)
                        || "<|".starts_with(&lc)
                    {
                        // keep buffering potential tag prefix
                    } else {
                        out.push_str(&self.buf);
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    }
                }
                SanitizerMode::InThink => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InEndThinkPrefix;
                    }
                }
                SanitizerMode::InEndThinkPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "</think>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else if "</think>".starts_with(&lc) {
                        // keep matching </think>
                    } else if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                    } else {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThink;
                    }
                }
                SanitizerMode::InThought => {
                    if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                        self.mode = SanitizerMode::InEndThoughtPrefix;
                    }
                }
                SanitizerMode::InEndThoughtPrefix => {
                    self.buf.push(ch);
                    let lc = self.buf.to_ascii_lowercase();
                    if lc == "</thought>" {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else if "</thought>".starts_with(&lc) {
                        // keep matching </thought>
                    } else if ch == '<' {
                        self.buf.clear();
                        self.buf.push('<');
                    } else {
                        self.buf.clear();
                        self.mode = SanitizerMode::InThought;
                    }
                }
                SanitizerMode::InSpecialToken => {
                    if ch == '>' && self.buf.ends_with('|') {
                        self.buf.clear();
                        self.mode = SanitizerMode::Normal;
                    } else {
                        self.buf.push(ch);
                        // Guard against malformed unclosed `<|` sequence
                        if self.buf.len() > 64 {
                            out.push_str("<|");
                            out.push_str(&self.buf);
                            self.buf.clear();
                            self.mode = SanitizerMode::Normal;
                        }
                    }
                }
            }
        }
        out
    }

    fn flush(&mut self) -> String {
        match self.mode {
            SanitizerMode::InTagPrefix => {
                self.mode = SanitizerMode::Normal;
                std::mem::take(&mut self.buf)
            }
            _ => {
                self.buf.clear();
                self.mode = SanitizerMode::Normal;
                String::new()
            }
        }
    }
}

fn emit_stream_text_chunk(
    text_tx: &tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    chunk: String,
    emit_to_user: bool,
    assistant_text: &mut String,
    has_streamed_text: &mut bool,
    last_streamed_ends_newline: &mut bool,
    needs_paragraph_break: &mut bool,
) {
    if chunk.is_empty() {
        return;
    }
    if emit_to_user {
        if *needs_paragraph_break
            && *has_streamed_text
            && !*last_streamed_ends_newline
            && !chunk.starts_with('\n')
        {
            let _ = text_tx.send(Ok("\n\n".to_string()));
        }
        *needs_paragraph_break = false;
        *has_streamed_text = true;
        *last_streamed_ends_newline = chunk.ends_with('\n');
        let _ = text_tx.send(Ok(chunk.clone()));
    }
    assistant_text.push_str(&chunk);
}

fn format_model_error_meta_token(message: &str, retry_after_secs: u64) -> String {
    format!(
        "__MODEL_ERROR__{}",
        serde_json::json!({
            "code": "models_unavailable",
            "message": message,
            "retry_after_secs": retry_after_secs,
        })
    )
}

#[allow(clippy::too_many_arguments)]
async fn consume_recovery_stream<S>(
    raw_stream: S,
    text_tx: &tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    new_model_id: &str,
    new_provider: &str,
    warn_category: &str,
    warn_message: String,
    emit_model_event: bool,
    sanitizer: &mut StreamTextSanitizer,
    has_streamed_text: &mut bool,
    last_streamed_ends_newline: &mut bool,
    needs_paragraph_break: &mut bool,
    pid: &str,
    log_label: &str,
) -> (String, Vec<ToolCallRecord>)
where
    S: futures::Stream<Item = Result<String, ExecutionError>>,
{
    tokio::pin!(raw_stream);
    let mut rec_text = String::new();
    let mut rec_tool_calls: Vec<ToolCallRecord> = Vec::new();
    let mut announced = false;

    let announce = |announced: &mut bool| {
        if !*announced {
            *announced = true;
            if emit_model_event {
                let payload = serde_json::json!({
                    "model_id": new_model_id,
                    "provider": new_provider,
                });
                let _ = text_tx.send(Ok(format!("__MODEL_SELECTED__{}", payload)));
            }
            let wpayload = serde_json::json!({
                "category": warn_category,
                "provider": new_provider,
                "message": warn_message,
            });
            let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
        }
    };

    while let Some(tr) = raw_stream.next().await {
        match tr {
            Ok(token) if token.starts_with("__TOOL_CALL__") => {
                announce(&mut announced);
                let _ = text_tx.send(Ok(token.clone()));
                let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str) {
                    rec_tool_calls.push(ToolCallRecord {
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
                let clean = sanitizer.push(&text);
                if !clean.is_empty() {
                    announce(&mut announced);
                    emit_stream_text_chunk(
                        text_tx,
                        clean,
                        true,
                        &mut rec_text,
                        has_streamed_text,
                        last_streamed_ends_newline,
                        needs_paragraph_break,
                    );
                }
            }
            Err(e) => {
                warn!(
                    pid = %pid,
                    model = %new_model_id,
                    provider = %new_provider,
                    error = %e,
                    "{} stream error during recovery",
                    log_label
                );
                break;
            }
        }
    }

    let tail = sanitizer.flush();
    if !tail.is_empty() {
        announce(&mut announced);
        emit_stream_text_chunk(
            text_tx,
            tail,
            true,
            &mut rec_text,
            has_streamed_text,
            last_streamed_ends_newline,
            needs_paragraph_break,
        );
    }

    (rec_text, rec_tool_calls)
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

    // --- CHAT AGENT LEVEL 0 DELEGATION & TASK HELPERS ---

    fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| match c {
                'á' | 'à' | 'ä' => 'a',
                'é' | 'è' | 'ë' => 'e',
                'í' | 'ì' | 'ï' => 'i',
                'ó' | 'ò' | 'ö' => 'o',
                'ú' | 'ù' | 'ü' => 'u',
                _ => c,
            })
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn assistant_claims_delegation(text: &str) -> bool {
        let norm = Self::normalize_text(text);
        norm.contains("pidiendo al equipo")
            || norm.contains("pidiendo al equipazo")
            || norm.contains("pido al equipo")
            || norm.contains("pedi al equipo")
            || norm.contains("le pedi a")
            || norm.contains("le he pedido a")
            || norm.contains("equipo de aegis")
            || norm.contains("equipazo de aegis")
            || norm.contains("equipo ya esta")
            || norm.contains("equipo esta")
            || norm.contains("equipazo ya esta")
            || norm.contains("equipazo esta")
            || norm.contains("asignando al supervisor")
            || norm.contains("asigne al supervisor")
            || norm.contains("asigne la tarea")
            || norm.contains("asignar la tarea")
            || norm.contains("empujoncito al equipo")
            || norm.contains("empujoncito")
            || norm.contains("supervisor de proyecto")
            || norm.contains("supervisor esta trabajando")
            || norm.contains("te aviso cuando este listo")
            || norm.contains("te avisare cuando")
            || norm.contains("estoy creando tu nuevo proyecto")
            || norm.contains("encargare de generar")
            || norm.contains("manos a la obra con tu nuevo")
            || norm.contains("programar eso para")
            || norm.contains("programando eso para")
            || norm.contains("en cuanto tenga el script listo")
            || norm.contains("en cuanto este listo el script")
            || norm.contains("en cuanto tenga el codigo listo")
            || norm.contains("en cuanto este listo el codigo")
    }

    pub fn is_video_production_request(prompt: &str) -> bool {
        if Self::is_status_query(prompt) {
            return false;
        }
        let lower = prompt.to_lowercase();
        let norm = Self::normalize_text(prompt);

        let explicit_video_markers = [
            "video_produce_auto",
            "video_create_project",
            "video_render_mp4",
            "gemini_studio",
            "free_builtin",
            "hybrid_pro",
            "video con ia",
            "video ia",
            "exportar mp4",
            "exportalo a mp4",
            "exporta el mp4",
        ];
        if explicit_video_markers
            .iter()
            .any(|m| lower.contains(m) || norm.contains(m))
        {
            return true;
        }

        let has_video_noun = norm.split_whitespace().any(|w| {
            matches!(
                w,
                "video" | "videos" | "mp4" | "reel" | "reels" | "short" | "shorts" | "cortometraje"
            )
        }) || lower.contains("9:16")
            || lower.contains("16:9")
            || norm.contains("video studio");

        if !has_video_noun {
            return false;
        }

        let action_verbs = [
            "crea",
            "creame",
            "crear",
            "genera",
            "generame",
            "generar",
            "hace",
            "haceme",
            "hacer",
            "arma",
            "armame",
            "armar",
            "edita",
            "editar",
            "renderiza",
            "renderizar",
            "exporta",
            "exportar",
            "produci",
            "producir",
            "promocionando",
            "promocional",
        ];
        norm.split_whitespace().any(|w| action_verbs.contains(&w))
            || lower.contains("4 escenas")
            || lower.contains("escenas")
    }

    pub fn assistant_claims_video_execution(text: &str) -> bool {
        let norm = Self::normalize_text(text);
        let markers = [
            "aqui va tu video",
            "aqui tienes tu video",
            "aqui tenes tu video",
            "aqui va el intento",
            "aqui va la llamada a la herramienta",
            "va la herramienta",
            "llamar a la herramienta",
            "invocar la herramienta",
            "preparando la invocacion",
            "a punto de invocar la herramienta",
            "video produce auto",
            "videoproduceauto",
            "video create project",
            "voy a crearlo con la herramienta",
            "estoy creando tu nuevo proyecto",
            "creando tu video",
        ];
        markers.iter().any(|m| norm.contains(m))
    }

    pub fn is_video_followup_nudge(prompt: &str) -> bool {
        let norm = Self::normalize_text(prompt);
        let markers = [
            "lo vas a hacer",
            "vas a seguir durmiendo",
            "y donde esta",
            "donde esta el video",
            "no muestra que este haciendo nada",
            "no hace nada",
            "no hizo nada",
            "estas inventando",
            "no enviaste nada",
            "cuanto vas a tardar",
            "hace 10 minutos",
            "hacelo",
            "si hacelo",
            "dale hacelo",
            "generalo",
            "terminalo",
            "proba de nuevo",
            "vamos a que",
        ];
        markers.iter().any(|m| norm.contains(m))
    }

    pub fn resolve_video_request_prompt(
        prompt: &str,
        history: &[ChatMessage],
        assistant_text: &str,
    ) -> Option<String> {
        let trimmed = prompt.trim();
        if Self::is_video_production_request(trimmed) {
            return Some(trimmed.to_string());
        }
        if Self::is_status_query(trimmed) || Self::is_video_advisory_or_followup_query(trimmed) {
            return None;
        }
        if Self::is_video_followup_nudge(trimmed)
            || (!assistant_text.trim().is_empty()
                && Self::assistant_claims_video_execution(assistant_text))
        {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        let c = content.trim();
                        if Self::is_video_production_request(c) {
                            return Some(c.to_string());
                        }
                    }
                }
            }
            if !assistant_text.trim().is_empty()
                && Self::assistant_claims_video_execution(assistant_text)
                && !trimmed.is_empty()
            {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    pub fn has_video_context(prompt: &str, history: &[ChatMessage]) -> bool {
        Self::resolve_video_request_prompt(prompt, history, "").is_some()
    }

    fn is_status_query(prompt: &str) -> bool {
        let norm = Self::normalize_text(prompt);
        norm == "que paso"
            || norm.starts_with("que paso")
            || norm == "como va"
            || norm.starts_with("como va")
            || norm.contains("novedades")
            || norm.contains("alguna novedad")
            || norm == "estado"
            || norm == "status"
            || norm.starts_with("como venimos")
            || norm.starts_with("termino")
            || norm == "terminaste"
            || norm == "ya esta"
            || norm.starts_with("ya esta listo")
            || norm.starts_with("ya esta terminad")
            || norm.starts_with("ya estan listo")
            || norm.starts_with("ya termino")
    }

    pub fn is_video_advisory_or_followup_query(prompt: &str) -> bool {
        if Self::is_video_production_request(prompt) {
            return false;
        }
        let norm = Self::normalize_text(prompt);
        let advisory_markers = [
            "que necesitas",
            "que necesitaria",
            "salga mejor",
            "salio mal",
            "otras apis",
            "otra api",
            "cual api",
            "que api",
            "como mejorar",
            "por que salio",
            "que motor",
            "cual motor",
            "mejor calidad",
            "conviene api",
            "api o local",
            "gemini pago",
            "tier 1",
        ];
        if advisory_markers.iter().any(|m| norm.contains(m)) {
            return true;
        }
        let media_terms = [
            "video",
            "videos",
            "mp4",
            "reel",
            "escena",
            "escenas",
            "freebuiltin",
            "geministudio",
            "hybridpro",
            "elevenlabs",
            "pexels",
        ];
        norm.contains("calidad") && media_terms.iter().any(|m| norm.contains(m))
    }

    fn is_technical_request(prompt: &str) -> bool {
        if Self::is_status_query(prompt)
            || Self::is_video_production_request(prompt)
            || Self::is_video_advisory_or_followup_query(prompt)
        {
            return false;
        }
        let norm = Self::normalize_text(prompt);
        let technical_keywords = [
            "script",
            "codigo",
            "python",
            "rust",
            "javascript",
            "typescript",
            "programa",
            "programar",
            "desarrollar",
            "desarrolla",
            "crea un",
            "crear un",
            "hace un",
            "hacer un",
            "implementa",
            "implementar",
            "escribe un",
            "escribir un",
            "analiza",
            "analizar",
            "corrige",
            "corregir",
            "revisa",
            "revisar",
            "refactor",
            "bug",
            "endpoint",
            "clona",
            "clonar",
            "clones",
            "repositorio",
            "compila",
            "compilar",
            "depura",
            "depurar",
            "pull request",
        ];
        technical_keywords.iter().any(|kw| norm.contains(kw))
    }

    fn has_technical_context(prompt: &str, history: &[ChatMessage]) -> bool {
        if Self::is_status_query(prompt)
            || Self::is_video_production_request(prompt)
            || Self::is_video_advisory_or_followup_query(prompt)
        {
            return false;
        }
        if Self::is_technical_request(prompt) {
            return true;
        }
        let short_reply = prompt.trim().len() < 50;
        if short_reply {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        if Self::is_technical_request(content) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn derive_task_scope(prompt: &str, history: &[ChatMessage]) -> String {
        let trimmed = prompt.trim();
        if trimmed.len() < 50 {
            for msg in history.iter().rev() {
                if msg.role == ChatRole::User {
                    if let Some(ref content) = msg.content {
                        if Self::is_technical_request(content) {
                            return format!("{}. Parámetros/Detalles: {}", content.trim(), trimmed);
                        }
                    }
                }
            }
        }
        trimmed.to_string()
    }

    fn derive_task_project_name(scope: &str) -> String {
        let lower = scope.to_lowercase();
        if lower.contains("python") && (lower.contains("disco") || lower.contains("espacio")) {
            "Script Espacio Disco".to_string()
        } else if lower.contains("python") {
            "Script Python".to_string()
        } else if lower.contains("script") {
            "Script Automatización".to_string()
        } else {
            let clean: String = scope
                .chars()
                .take(35)
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .collect();
            let trimmed = clean.trim();
            if trimmed.is_empty() {
                "Proyecto".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }

    /// Detecta si un token parece una clave API real (Groq, Gemini, OpenRouter, OpenAI, Anthropic, xAI).
    fn is_likely_api_key_token(token: &str) -> bool {
        let clean = token.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']'
            )
        });
        if clean.len() < 20 {
            return false;
        }
        clean.starts_with("gsk_")
            || clean.starts_with("AIza")
            || clean.starts_with("sk-or-")
            || clean.starts_with("sk-ant-")
            || clean.starts_with("xai-")
            || clean.starts_with("sk-")
    }

    /// Extrae la primera clave API reconocible dentro de un texto libre del usuario.
    pub fn extract_api_key_from_text(text: &str) -> Option<String> {
        Self::extract_all_api_keys_from_text(text)
            .into_iter()
            .map(|(_, k)| k)
            .next()
    }

    /// Extrae todas las claves API reconocibles dentro del texto junto con su proveedor inferido.
    pub fn extract_all_api_keys_from_text(text: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for line in text.lines() {
            let line_trim = line.trim();
            for word in line_trim.split_whitespace() {
                let clean = word.trim_matches(|c: char| {
                    matches!(
                        c,
                        '"' | '\''
                            | '`'
                            | ','
                            | '.'
                            | ';'
                            | ':'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | '='
                    )
                });
                let is_key = Self::is_likely_api_key_token(clean)
                    || (clean.len() >= 24
                        && (line_trim.to_lowercase().contains("ollama")
                            || line_trim.to_lowercase().contains("elevenlabs")
                            || (clean.contains('.') && clean.len() >= 30)));

                if is_key && seen.insert(clean.to_string()) {
                    let prov = Self::infer_provider_from_key_or_text("", clean, line_trim);
                    results.push((prov, clean.to_string()));
                }
            }
        }
        results
    }

    /// Enmascara una clave API individual (`gsk_••••1234`) para no exponerla en logs ni respuestas.
    pub fn mask_single_key(key: &str) -> String {
        let trimmed = key.trim();
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() <= 8 {
            return "••••".to_string();
        }
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{}••••{}", prefix, suffix)
    }

    /// Enmascara cualquier clave API presente en un texto antes de persistirlo en historial o memoria.
    pub fn mask_api_keys_in_text(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.char_indices().peekable();

        while let Some(&(start, ch)) = chars.peek() {
            let is_ws = ch.is_whitespace();
            let mut end = text.len();
            while let Some(&(idx, next_ch)) = chars.peek() {
                if next_ch.is_whitespace() != is_ws {
                    end = idx;
                    break;
                }
                chars.next();
            }
            let slice = &text[start..end];
            if is_ws {
                out.push_str(slice);
            } else {
                let clean = slice.trim_matches(|c: char| {
                    matches!(
                        c,
                        '"' | '\'' | '`' | ',' | '.' | ';' | ':' | '(' | ')' | '[' | ']'
                    )
                });
                if Self::is_likely_api_key_token(clean)
                    || (clean.len() >= 30
                        && (clean.contains('.') || clean.chars().all(|c| c.is_ascii_hexdigit())))
                {
                    out.push_str(&slice.replace(clean, &Self::mask_single_key(clean)));
                } else {
                    out.push_str(slice);
                }
            }
        }

        out
    }

    /// Infiere el ID canónico del proveedor a partir del prefijo de la API key o del texto.
    pub fn infer_provider_from_key_or_text(
        raw_provider: &str,
        api_key: &str,
        hint_text: &str,
    ) -> String {
        let k = api_key.trim();
        if k.starts_with("gsk_") {
            return "groq".to_string();
        }
        if k.starts_with("AIza") {
            return "gemini".to_string();
        }
        if k.starts_with("sk-or-") {
            return "openrouter".to_string();
        }
        if k.starts_with("sk-ant-") {
            return "anthropic".to_string();
        }
        if k.starts_with("xai-") {
            return "xai".to_string();
        }

        if !raw_provider.trim().is_empty() {
            return crate::router::normalize_provider_id(raw_provider);
        }

        let lower = hint_text.to_lowercase();
        for (kw, prov) in [
            ("elevenlabs", "elevenlabs"),
            ("openrouter", "openrouter"),
            ("gemini", "gemini"),
            ("google", "gemini"),
            ("groq", "groq"),
            ("anthropic", "anthropic"),
            ("claude", "anthropic"),
            ("deepseek", "deepseek"),
            ("mistral", "mistral"),
            ("ollama", "ollama_cloud"),
            ("xai", "xai"),
            ("grok", "xai"),
            ("openai", "openai"),
            ("gpt", "openai"),
        ] {
            if lower.contains(kw) {
                return prov.to_string();
            }
        }

        if k.starts_with("sk-") {
            return "openai".to_string();
        }

        String::new()
    }

    /// Retorna `(default_model, default_api_url)` para cada proveedor soportado.
    pub fn default_model_and_url_for_provider(provider: &str) -> (&'static str, &'static str) {
        match provider {
            "groq" => ("openai/gpt-oss-120b", "https://api.groq.com/openai/v1"),
            "gemini" => (
                "gemini-2.5-flash",
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            "openrouter" => ("openrouter/free", "https://openrouter.ai/api/v1"),
            "ollama_cloud" => ("gpt-oss:120b", "https://ollama.com"),
            "ollama" => ("llama3.2", "http://localhost:11434/v1"),
            "openai" => ("gpt-4o-mini", "https://api.openai.com/v1"),
            "anthropic" => ("claude-3-5-sonnet-latest", "https://api.anthropic.com/v1"),
            "deepseek" => ("deepseek-chat", "https://api.deepseek.com/v1"),
            "xai" => ("grok-2-latest", "https://api.x.ai/v1"),
            "mistral" => ("mistral-small-latest", "https://api.mistral.ai/v1"),
            _ => ("gpt-4o-mini", "https://api.openai.com/v1"),
        }
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

    // --- CORE-275: Filesystem helpers ---

    fn get_tenant_workspace(pcb: &PCB) -> std::path::PathBuf {
        let data_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
        std::path::Path::new(&data_dir)
            .join("users")
            .join(tenant)
            .join("workspace")
    }

    fn valid_pcb_session_key(pcb: &PCB) -> Option<&str> {
        pcb.session_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty() && *k != "default")
    }

    async fn get_approved_paths(pcb: &PCB) -> Vec<String> {
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
            return vec![];
        };
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
        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
            return false;
        };
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
        pid: &str,
        pcb: &PCB,
    ) -> String {
        self.execute_tool_call_with_progress(tc, pid, pcb, None)
            .await
    }

    async fn execute_tool_call_with_progress(
        &self,
        tc: &ToolCallRecord,
        _pid: &str,
        pcb: &PCB,
        progress_tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>>,
    ) -> String {
        let mut args: serde_json::Value =
            serde_json::from_str(&tc.function.arguments).unwrap_or_default();

        match tc.function.name.as_str() {
            "spawn_agent" => {
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        let name = args["name"].as_str().map(String::from);
                        let scope = args["scope"].as_str().unwrap_or("").to_string();
                        let task_type_str = args["task_type"].as_str();
                        let task_type = task_type_str
                            .map(crate::syscalls::parse_task_type)
                            .or_else(|| {
                                if !scope.trim().is_empty() {
                                    Some(crate::pcb::infer_task_type(&scope))
                                } else {
                                    Some(crate::pcb::TaskType::Planning)
                                }
                            });

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
                                    scope.clone(),
                                    task_type,
                                    pcb.tenant_id.clone(),
                                )
                                .await
                            {
                                Ok(agent_id) => {
                                    let task = if !scope.trim().is_empty() {
                                        scope.clone()
                                    } else {
                                        pcb.memory_pointers.l1_instruction.clone()
                                    };
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
                                    // CORE-EMIT: Actualizar inmediatamente el árbol en la UI para que aparezca en el Dashboard
                                    let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
                                    let nodes = orchestrator.tree_snapshot_for_tenant(tenant).await;
                                    orchestrator.emit_event(
                                        crate::agents::event::AgentEvent::TreeSnapshot { nodes },
                                    );

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
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

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

            // CORE-375: Herramientas personales instantáneas del Chat Agent
            "log_expense_or_entry" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let raw_category = args["category"].as_str().unwrap_or("general");
                let description = args["description"].as_str().unwrap_or("");
                let amount = args["amount"].as_f64().unwrap_or(0.0);

                let norm_category =
                    crate::enclave::normalize_expense_category(raw_category, description);

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => {
                        if let Err(e) = db.add_expense(amount, description, Some(&norm_category)) {
                            return format!(
                                "{{\"error\":\"add_expense_failed\",\"detail\":\"{}\"}}",
                                e
                            );
                        }
                        match db.query_expenses_summary(Some(&norm_category)) {
                            Ok(summary) => summary.to_string(),
                            Err(e) => {
                                format!("{{\"error\":\"summary_failed\",\"detail\":\"{}\"}}", e)
                            }
                        }
                    }
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "query_ledger" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let category_opt = args["category"].as_str();

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match db.query_expenses_summary(category_opt) {
                        Ok(summary) => summary.to_string(),
                        Err(e) => format!("{{\"error\":\"summary_failed\",\"detail\":\"{}\"}}", e),
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "remember_fact" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let action = args["action"]
                    .as_str()
                    .unwrap_or("upsert")
                    .trim()
                    .to_lowercase();
                let id_opt = args.get("id").and_then(|v| {
                    v.as_i64().or_else(|| {
                        v.as_str()
                            .map(|s| s.trim().trim_start_matches('#').trim())
                            .and_then(|s| s.parse::<i64>().ok())
                    })
                });
                let category = args["category"].as_str().unwrap_or("general");
                let fact = args["fact"].as_str().unwrap_or("").trim();

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match action.as_str() {
                        "delete" => {
                            let Some(fid) = id_opt else {
                                return "{\"error\":\"missing_id\",\"detail\":\"Indicá el id de la nota a eliminar.\"}".to_string();
                            };
                            match db.delete_personal_fact(fid) {
                                Ok(true) => serde_json::json!({
                                    "status": "deleted",
                                    "id": fid
                                })
                                .to_string(),
                                Ok(false) => serde_json::json!({
                                    "status": "not_found",
                                    "id": fid
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"remember_fact_delete_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        _ => {
                            if fact.is_empty() {
                                return "{\"error\":\"empty_fact\",\"detail\":\"El hecho a guardar no puede estar vacío.\"}".to_string();
                            }
                            match db.upsert_personal_fact(id_opt, category, fact, "chat") {
                                Ok(crate::enclave::PersonalFactUpsertOutcome::Inserted { id }) => {
                                    serde_json::json!({
                                        "status": "remembered",
                                        "id": id,
                                        "fact": fact,
                                        "category": category
                                    })
                                    .to_string()
                                }
                                Ok(crate::enclave::PersonalFactUpsertOutcome::Updated {
                                    id,
                                    previous_fact,
                                }) => serde_json::json!({
                                    "status": "updated",
                                    "id": id,
                                    "fact": fact,
                                    "previous_fact": previous_fact,
                                    "category": category
                                })
                                .to_string(),
                                Ok(crate::enclave::PersonalFactUpsertOutcome::AlreadyKnown {
                                    id,
                                }) => serde_json::json!({
                                    "status": "already_known",
                                    "id": id,
                                    "fact": fact,
                                    "category": category
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"remember_fact_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "manage_reminder" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let action = args["action"].as_str().unwrap_or("list").trim();
                let id_arg = args.get("id").and_then(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                });
                let text = args["text"].as_str().unwrap_or("").trim();
                let time_hint = args["time_hint"].as_str().unwrap_or("").trim();
                let raw_rec = args["recurrence"].as_str().unwrap_or("").trim();

                let mut norm_rec = crate::enclave::normalize_recurrence(raw_rec);
                let th_lc = time_hint.to_lowercase();
                if norm_rec.is_empty()
                    && (th_lc.contains("todos los")
                        || th_lc.contains("cada ")
                        || th_lc.contains("diari")
                        || th_lc.contains("semanal")
                        || th_lc.contains("mensual"))
                {
                    norm_rec = crate::enclave::normalize_recurrence(time_hint);
                }

                let now_local = chrono::Local::now();
                let remind_at =
                    crate::enclave::normalize_remind_at(time_hint, &norm_rec, now_local);

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match action {
                        "add" => {
                            if text.is_empty() {
                                return "{\"error\":\"missing_text\",\"detail\":\"Falta la descripción del recordatorio.\"}".to_string();
                            }
                            match db.add_reminder_with_recurrence(&remind_at, text, Some(&norm_rec))
                            {
                                Ok(new_id) => serde_json::json!({
                                    "status": "added",
                                    "id": new_id,
                                    "description": text,
                                    "remind_at": remind_at,
                                    "recurrence": norm_rec
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"add_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "update" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' para actualizar el recordatorio.\"}".to_string();
                            }
                            let new_remind_at =
                                (!time_hint.is_empty()).then_some(remind_at.as_str());
                            let new_desc = (id_arg.is_some() && !text.is_empty()).then_some(text);
                            let new_rec = args
                                .get("recurrence")
                                .is_some()
                                .then_some(norm_rec.as_str())
                                .or_else(|| (!norm_rec.is_empty()).then_some(norm_rec.as_str()));
                            match db.update_reminder(target, new_remind_at, new_desc, new_rec) {
                                Ok(count) => {
                                    let reminders = db.get_reminders(30).unwrap_or_default();
                                    serde_json::json!({
                                        "status": "updated",
                                        "updated_count": count,
                                        "target": target,
                                        "reminders": reminders
                                    })
                                    .to_string()
                                }
                                Err(e) => format!(
                                    "{{\"error\":\"update_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "delete" | "remove" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' del recordatorio a eliminar.\"}".to_string();
                            }
                            match db.delete_reminder(target) {
                                Ok(count) => serde_json::json!({
                                    "status": "deleted",
                                    "deleted_count": count,
                                    "target": target
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"delete_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "complete" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' del recordatorio a completar.\"}".to_string();
                            }
                            match db.complete_reminder(target) {
                                Ok(count) => serde_json::json!({
                                    "status": "completed",
                                    "matched_count": count,
                                    "target": target
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"complete_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        _ => match db.get_reminders(30) {
                            Ok(reminders) => serde_json::json!({
                                "status": "ok",
                                "reminders": reminders
                            })
                            .to_string(),
                            Err(e) => format!(
                                "{{\"error\":\"list_reminders_failed\",\"detail\":\"{}\"}}",
                                e
                            ),
                        },
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-385: Puente directo hacia Google Antigravity (SDK + IDE Outbox)
            "send_to_antigravity" => {
                let instruction = args["instruction"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if instruction.is_empty() {
                    return "{\"error\":\"instruction is required\"}".to_string();
                }
                let project_name = args
                    .get("project_name")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Antigravity")
                    .to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        // 1. Encolar siempre en el outbox para el hook del IDE Antigravity
                        orchestrator
                            .agy_runner
                            .push_ide_outbox(instruction.clone())
                            .await;

                        // 2. Si el SDK google-antigravity está habilitado, despachar también al supervisor
                        if orchestrator.agy_runner.is_enabled() {
                            if let Ok(agent_id) = orchestrator
                                .create_project(
                                    project_name.clone(),
                                    instruction.clone(),
                                    Some(crate::pcb::TaskType::Code),
                                    pcb.tenant_id.clone(),
                                )
                                .await
                            {
                                let _ = orchestrator
                                    .dispatch(agent_id, instruction.clone(), vec![])
                                    .await;
                                let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
                                let nodes = orchestrator.tree_snapshot_for_tenant(tenant).await;
                                orchestrator.emit_event(
                                    crate::agents::event::AgentEvent::TreeSnapshot { nodes },
                                );
                                return serde_json::json!({
                                    "status": "dispatched_to_antigravity_sdk_and_ide_outbox",
                                    "project": project_name,
                                    "agent_id": agent_id.to_string(),
                                })
                                .to_string();
                            }
                        }

                        serde_json::json!({
                            "status": "queued_for_antigravity_ide",
                            "project": project_name,
                            "message": "Instrucción encolada para Google Antigravity."
                        })
                        .to_string()
                    }
                }
            }

            // Gestión conversacional de proveedores de IA, claves API y modelos desde el chat
            "manage_ai_provider" => {
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();
                let raw_provider = args["provider"].as_str().unwrap_or("").trim();
                let mut raw_key = args["api_key"].as_str().unwrap_or("").trim().to_string();
                let raw_model = args["model"].as_str().unwrap_or("").trim();
                let raw_url = args["api_url"].as_str().unwrap_or("").trim();
                let set_primary = args["set_primary"].as_bool().unwrap_or(false);

                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                let router_opt = self.router_ref.read().await.clone();
                let Some(router_rw) = router_opt else {
                    return "{\"error\":\"router_not_available\"}".to_string();
                };

                if raw_key.is_empty() {
                    if let Some(extracted) =
                        Self::extract_api_key_from_text(&pcb.memory_pointers.l1_instruction)
                    {
                        raw_key = extracted;
                    }
                }

                let provider = Self::infer_provider_from_key_or_text(
                    raw_provider,
                    &raw_key,
                    &pcb.memory_pointers.l1_instruction,
                );

                match action.as_str() {
                    "list" => {
                        let router = router_rw.read().await;
                        let global_keys = router.list_global_keys().await;
                        let tenant_keys = router.list_tenant_keys(tenant_id).await;
                        let catalog_models = router.list_models_for_catalog().await;
                        let providers_summary = summarize_configured_providers(
                            tenant_keys.into_iter().chain(global_keys.into_iter()),
                            &catalog_models,
                        );

                        let agy_enabled = match self.agent_orchestrator.read().await.clone() {
                            Some(orch) => orch.agy_runner.is_enabled(),
                            None => false,
                        };

                        serde_json::json!({
                            "status": "ok",
                            "configured_providers": providers_summary,
                            "total_catalog_models": catalog_models.len(),
                            "antigravity_sdk_enabled": agy_enabled,
                            "hint": "Para ver la lista completa o modelos gratuitos de un proveedor (ej. openrouter), llamá a manage_ai_provider con action='catalog'."
                        })
                        .to_string()
                    }

                    "catalog" => {
                        let free_only = args["free_only"].as_bool().unwrap_or(false);
                        let limit = args["limit"].as_u64().unwrap_or(30).clamp(1, 100) as usize;
                        let filter_provider = if raw_provider.eq_ignore_ascii_case("openroute")
                            || provider.eq_ignore_ascii_case("openroute")
                        {
                            "openrouter"
                        } else if !raw_provider.is_empty() {
                            raw_provider
                        } else {
                            provider.as_str()
                        };

                        let router = router_rw.read().await;
                        let mut matches: Vec<_> = router
                            .list_models_for_catalog()
                            .await
                            .into_iter()
                            .filter(|m| {
                                if !filter_provider.is_empty() {
                                    if filter_provider.eq_ignore_ascii_case("openrouter") {
                                        if !(m.provider == "openrouter"
                                            || (!m.is_local && m.model_id.contains('/')))
                                        {
                                            return false;
                                        }
                                    } else if !m.provider.eq_ignore_ascii_case(filter_provider) {
                                        return false;
                                    }
                                }
                                if free_only {
                                    let is_free = m.free_tier_eligible
                                        || m.model_id.ends_with(":free")
                                        || (m.cost_input_per_mtok == 0.0
                                            && m.cost_output_per_mtok == 0.0
                                            && m.model_id != "openrouter/auto");
                                    if !is_free {
                                        return false;
                                    }
                                }
                                true
                            })
                            .collect();

                        matches.sort_by(|a, b| {
                            let a_free = a.free_tier_eligible || a.model_id.ends_with(":free");
                            let b_free = b.free_tier_eligible || b.model_id.ends_with(":free");
                            b_free
                                .cmp(&a_free)
                                .then_with(|| {
                                    a.cost_input_per_mtok.total_cmp(&b.cost_input_per_mtok)
                                })
                                .then_with(|| a.model_id.cmp(&b.model_id))
                        });

                        let total_matching = matches.len();
                        let models: Vec<serde_json::Value> = matches
                            .into_iter()
                            .take(limit)
                            .map(|m| {
                                let is_free = m.free_tier_eligible
                                    || m.model_id.ends_with(":free")
                                    || (m.cost_input_per_mtok == 0.0
                                        && m.cost_output_per_mtok == 0.0
                                        && m.model_id != "openrouter/auto");
                                serde_json::json!({
                                    "model_id": m.model_id,
                                    "display_name": m.display_name,
                                    "provider": m.provider,
                                    "free": is_free,
                                    "cost_input_per_mtok": m.cost_input_per_mtok,
                                    "cost_output_per_mtok": m.cost_output_per_mtok,
                                    "context_window": m.context_window,
                                    "supports_tools": m.supports_tools,
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "status": "ok",
                            "provider_filter": if filter_provider.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(filter_provider.to_string()) },
                            "free_only": free_only,
                            "total_matching": total_matching,
                            "returned": models.len(),
                            "models": models,
                        })
                        .to_string()
                    }

                    "add" => {
                        let is_free_prompt = pcb
                            .memory_pointers
                            .l1_instruction
                            .to_lowercase()
                            .contains("gratis")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("free")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("gratuito")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("cuenta gratuita");

                        let all_detected = Self::extract_all_api_keys_from_text(
                            &pcb.memory_pointers.l1_instruction,
                        );
                        if all_detected.len() > 1 {
                            let mut added_list = Vec::new();
                            let router = router_rw.read().await;
                            for (p, k) in all_detected {
                                if p.is_empty() {
                                    continue;
                                }
                                if p == "elevenlabs" {
                                    let masked = Self::mask_single_key(&k);
                                    if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                                        if let Ok(db) =
                                            crate::enclave::TenantDB::open(tenant_id, session_key)
                                        {
                                            let _ = db.set_kv("elevenlabs_api_key", &k);
                                        }
                                    }
                                    added_list.push(serde_json::json!({
                                        "provider": "elevenlabs",
                                        "service": "Siren Voice (TTS)",
                                        "masked_key": masked,
                                    }));
                                    continue;
                                }

                                let (def_m, def_u) = Self::default_model_and_url_for_provider(&p);
                                let discovered =
                                    match crate::router::discovery::fetch_provider_models(
                                        &p,
                                        Some(def_u),
                                        &k,
                                    )
                                    .await
                                    {
                                        Ok(list) if !list.is_empty() => {
                                            list.into_iter().map(|d| d.model_id).collect()
                                        }
                                        _ => vec![def_m.to_string()],
                                    };
                                let masked = Self::mask_single_key(&k);
                                let entry = crate::router::key_pool::ApiKeyEntry {
                                    key_id: uuid::Uuid::new_v4().to_string(),
                                    provider: p.clone(),
                                    api_key: k.clone(),
                                    api_url: Some(def_u.to_string()),
                                    label: Some(format!("Chat ({})", p)),
                                    is_active: true,
                                    rate_limited_until: None,
                                    active_models: Some(discovered.clone()),
                                    is_free_tier: is_free_prompt,
                                };
                                let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                                let _ = router.add_global_key(entry.clone()).await;
                                let _ = crate::router::syncer::register_provider_models(
                                    &p,
                                    &discovered,
                                    &router.catalog_ref(),
                                )
                                .await;
                                added_list.push(serde_json::json!({
                                    "provider": p,
                                    "models": discovered,
                                    "masked_key": masked,
                                }));
                            }
                            router.invalidate_all_sticky().await;
                            return serde_json::json!({
                                "status": "batch_added",
                                "count": added_list.len(),
                                "providers": added_list,
                                "message": format!("Se guardaron {} proveedores con éxito en el enclave cifrado.", added_list.len()),
                            }).to_string();
                        }

                        if provider == "elevenlabs" {
                            let masked = Self::mask_single_key(&raw_key);
                            if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                                if let Ok(db) =
                                    crate::enclave::TenantDB::open(tenant_id, session_key)
                                {
                                    let _ = db.set_kv("elevenlabs_api_key", &raw_key);
                                }
                            }
                            return serde_json::json!({
                                "status": "added",
                                "provider": "elevenlabs",
                                "service": "Siren Voice (TTS)",
                                "masked_key": masked,
                                "message": "Clave de ElevenLabs configurada en el motor de voz Siren."
                            }).to_string();
                        }

                        if provider.is_empty() {
                            return serde_json::json!({
                                "error": "provider_unknown",
                                "message": "No pude identificar el proveedor. Indicá por ejemplo: Groq, Gemini, OpenRouter, OpenAI, Anthropic, DeepSeek u Ollama."
                            })
                            .to_string();
                        }

                        if provider != "ollama" && raw_key.is_empty() {
                            return serde_json::json!({
                                "error": "missing_api_key",
                                "provider": provider,
                                "message": format!("Falta la clave API para {}. Pasame la API key para guardarla en el enclave.", provider)
                            })
                            .to_string();
                        }

                        let (default_model, default_url) =
                            Self::default_model_and_url_for_provider(&provider);
                        let effective_model = if !raw_model.is_empty() {
                            raw_model.to_string()
                        } else {
                            default_model.to_string()
                        };
                        let effective_url = if !raw_url.is_empty() {
                            raw_url.to_string()
                        } else {
                            default_url.to_string()
                        };

                        let masked = Self::mask_single_key(&raw_key);
                        let discovered = if !raw_model.is_empty() {
                            vec![raw_model.to_string()]
                        } else {
                            match crate::router::discovery::fetch_provider_models(
                                &provider,
                                Some(&effective_url),
                                &raw_key,
                            )
                            .await
                            {
                                Ok(list) if !list.is_empty() => {
                                    list.into_iter().map(|d| d.model_id).collect()
                                }
                                _ => vec![effective_model.clone()],
                            }
                        };

                        let entry = crate::router::key_pool::ApiKeyEntry {
                            key_id: uuid::Uuid::new_v4().to_string(),
                            provider: provider.clone(),
                            api_key: raw_key.clone(),
                            api_url: Some(effective_url.clone()),
                            label: Some(format!("Chat ({})", provider)),
                            is_active: true,
                            rate_limited_until: None,
                            active_models: Some(discovered.clone()),
                            is_free_tier: is_free_prompt,
                        };

                        {
                            let router = router_rw.read().await;
                            let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                            let _ = router.add_global_key(entry.clone()).await;
                            let _ = crate::router::syncer::register_provider_models(
                                &provider,
                                &discovered,
                                &router.catalog_ref(),
                            )
                            .await;
                            router.invalidate_all_sticky().await;
                        }

                        if set_primary {
                            self.update_cloud_credentials(
                                effective_url,
                                effective_model.clone(),
                                raw_key,
                            )
                            .await;
                        }

                        serde_json::json!({
                            "status": "added",
                            "provider": provider,
                            "model": effective_model,
                            "masked_key": masked,
                            "set_primary": set_primary,
                            "message": "Proveedor y clave guardados de forma cifrada en el enclave. Nunca repitas la clave completa en el chat."
                        })
                        .to_string()
                    }

                    "add_model" => {
                        if provider.is_empty() || raw_model.is_empty() {
                            return serde_json::json!({
                                "error": "missing_params",
                                "message": "Indicá tanto el proveedor (ej: gemini, groq, openrouter, ollama) como el nombre del modelo a sumar."
                            })
                            .to_string();
                        }

                        let router = router_rw.read().await;
                        let existing = router
                            .get_raw_key_for_provider(&provider, Some(tenant_id))
                            .await;

                        let Some(mut entry) = existing else {
                            return serde_json::json!({
                                "error": "provider_not_configured",
                                "provider": provider,
                                "message": format!("Todavía no tenés una clave configurada para {}. Pasame primero su API key.", provider)
                            })
                            .to_string();
                        };

                        let mut models = entry.active_models.take().unwrap_or_default();
                        if !models.iter().any(|m| m == raw_model) {
                            models.push(raw_model.to_string());
                        }
                        entry.active_models = Some(models.clone());

                        let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                        let _ = router.add_global_key(entry).await;
                        let _ = crate::router::syncer::register_provider_models(
                            &provider,
                            &models,
                            &router.catalog_ref(),
                        )
                        .await;
                        router.invalidate_all_sticky().await;

                        serde_json::json!({
                            "status": "model_added",
                            "provider": provider,
                            "added_model": raw_model,
                            "active_models": models,
                        })
                        .to_string()
                    }

                    "remove" => {
                        if provider.is_empty() {
                            return "{\"error\":\"provider is required to remove\"}".to_string();
                        }
                        let router = router_rw.read().await;
                        let mut removed = 0usize;
                        for k in router.list_tenant_keys(tenant_id).await {
                            if k.provider == provider
                                && router.delete_key(&k.key_id, Some(tenant_id)).await.is_ok()
                            {
                                removed += 1;
                            }
                        }
                        for k in router.list_global_keys().await {
                            if k.provider == provider
                                && router.delete_key(&k.key_id, None).await.is_ok()
                            {
                                removed += 1;
                            }
                        }
                        router.invalidate_all_sticky().await;
                        serde_json::json!({
                            "status": "removed",
                            "provider": provider,
                            "removed_keys_count": removed
                        })
                        .to_string()
                    }

                    _ => "{\"error\":\"invalid_action\"}".to_string(),
                }
            }

            // Inspección y habilitación/deshabilitación de módulos del microkernel y plugins cargados
            "manage_modules" => {
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();
                let module_id = args["module_id"].as_str().unwrap_or("").trim();
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let router_opt = self.router_ref.read().await.clone();
                let Some(router_rw) = router_opt else {
                    return "{\"error\":\"router_not_available\"}".to_string();
                };

                let router = router_rw.read().await;
                let modules_map = router.modules.read().await;
                let db_opt = Self::valid_pcb_session_key(pcb)
                    .and_then(|skey| crate::enclave::TenantDB::open(tenant_id, skey).ok());

                match action.as_str() {
                    "list" => {
                        let mut manifests: Vec<_> = modules_map.values().collect();
                        manifests.sort_by(|a, b| a.module_id.cmp(&b.module_id));

                        let microkernel_modules: Vec<serde_json::Value> = manifests
                            .into_iter()
                            .map(|m| {
                                let enabled = db_opt
                                    .as_ref()
                                    .and_then(|db| {
                                        db.get_kv(&format!("module_active:{}", m.module_id)).ok()
                                    })
                                    .flatten()
                                    .map(|v| v == "true")
                                    .unwrap_or(false);
                                let tools: Vec<serde_json::Value> = m
                                    .exposed_tools
                                    .iter()
                                    .map(|t| {
                                        serde_json::json!({
                                            "name": t.name,
                                            "description": t.description
                                        })
                                    })
                                    .collect();
                                serde_json::json!({
                                    "module_id": m.module_id,
                                    "display_name": m.display_name,
                                    "version": m.version,
                                    "enabled": enabled,
                                    "tools": tools,
                                })
                            })
                            .collect();

                        let loaded_plugins: Vec<serde_json::Value> = self
                            .plugin_manager
                            .read()
                            .await
                            .list_plugin_summaries()
                            .into_iter()
                            .map(|(name, description, kind)| {
                                serde_json::json!({
                                    "name": name,
                                    "description": description,
                                    "kind": kind,
                                    "active": true,
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "status": "ok",
                            "microkernel_modules": microkernel_modules,
                            "loaded_plugins": loaded_plugins,
                            "builtin_capabilities": ["maker (JS sandbox)"],
                        })
                        .to_string()
                    }
                    "enable" | "disable" => {
                        if module_id.is_empty() {
                            return "{\"error\":\"module_id_required\"}".to_string();
                        }
                        if !modules_map.contains_key(module_id) {
                            let available: Vec<&str> =
                                modules_map.keys().map(String::as_str).collect();
                            return serde_json::json!({
                                "error": "module_not_found",
                                "module_id": module_id,
                                "available_modules": available,
                            })
                            .to_string();
                        }
                        let Some(ref db) = db_opt else {
                            return "{\"error\":\"tenant_db_unavailable\"}".to_string();
                        };
                        let enabled = action == "enable";
                        if let Err(e) = db.set_kv(
                            &format!("module_active:{}", module_id),
                            if enabled { "true" } else { "false" },
                        ) {
                            return serde_json::json!({
                                "error": "db_write_failed",
                                "detail": e.to_string()
                            })
                            .to_string();
                        }
                        serde_json::json!({
                            "status": if enabled { "enabled" } else { "disabled" },
                            "module_id": module_id,
                            "enabled": enabled,
                        })
                        .to_string()
                    }
                    _ => "{\"error\":\"invalid_action\"}".to_string(),
                }
            }

            "manage_google_calendar" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();

                if action == "status" {
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            let connected = db.is_oauth_connected("google").unwrap_or(false);
                            let email = db.get_oauth_email("google").ok().flatten();
                            let scope = db.get_oauth_scope("google").ok().flatten();
                            let creds = crate::oauth::resolve_client_credentials(&db, "google");
                            serde_json::json!({
                                "status": "ok",
                                "connected": connected,
                                "email": email,
                                "scope": scope,
                                "client_configured": creds.client_id.is_some(),
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                let connected = crate::enclave::TenantDB::open(&tenant_id, session_key)
                    .map(|db| db.is_oauth_connected("google").unwrap_or(false))
                    .unwrap_or(false);
                if !connected {
                    return serde_json::json!({
                        "error": "google_not_connected",
                        "detail": "La cuenta de Google no está vinculada. El usuario debe conectarla en Settings → Cuentas Conectadas."
                    })
                    .to_string();
                }

                let res = match action.as_str() {
                    "list" => {
                        let days = args["days"].as_u64().unwrap_or(7).clamp(1, 60) as u8;
                        let max_results =
                            args["max_results"].as_u64().unwrap_or(20).clamp(1, 50) as u8;
                        let query = args["query"].as_str();
                        crate::oauth::google_calendar::list_events(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            days,
                            max_results,
                            query,
                        )
                        .await
                    }
                    "create" => {
                        let summary = args["summary"].as_str().unwrap_or("").to_string();
                        let start = args["start"].as_str().unwrap_or("").to_string();
                        let end = args["end"].as_str().map(String::from);
                        let description = args["description"].as_str().map(String::from);
                        let location = args["location"].as_str().map(String::from);
                        let all_day = args["all_day"].as_bool();
                        let attendees = args["attendees"].as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        });
                        crate::oauth::google_calendar::create_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            crate::oauth::google_calendar::CalendarEventInput {
                                summary,
                                start,
                                end,
                                description,
                                location,
                                attendees,
                                all_day,
                            },
                        )
                        .await
                    }
                    "update" => {
                        let event_id = args["event_id"].as_str().unwrap_or("");
                        crate::oauth::google_calendar::update_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            event_id,
                            args["summary"].as_str(),
                            args["start"].as_str(),
                            args["end"].as_str(),
                            args["description"].as_str(),
                            args["location"].as_str(),
                        )
                        .await
                    }
                    "delete" => {
                        let event_id = args["event_id"].as_str().unwrap_or("");
                        crate::oauth::google_calendar::delete_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            event_id,
                        )
                        .await
                    }
                    _ => Err(anyhow::anyhow!(
                        "Acción de calendario desconocida: {}",
                        action
                    )),
                };

                match res {
                    Ok(val) => val.to_string(),
                    Err(e) => serde_json::json!({
                        "error": "google_calendar_failed",
                        "detail": e.to_string()
                    })
                    .to_string(),
                }
            }

            "manage_whatsapp" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };
                let action = args["action"]
                    .as_str()
                    .unwrap_or("status")
                    .trim()
                    .to_lowercase();

                if action == "set_confirmation" {
                    let enabled = args["require_confirmation"].as_bool().unwrap_or(true);
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            if let Err(e) = crate::whatsapp::set_require_confirmation(&db, enabled)
                            {
                                serde_json::json!({
                                    "error": "whatsapp_settings_failed",
                                    "detail": e.to_string()
                                })
                                .to_string()
                            } else {
                                serde_json::json!({
                                    "status": "updated",
                                    "require_confirmation": enabled,
                                    "message": if enabled {
                                        "Confirmación previa ACTIVADA: voy a pedirte OK antes de enviar cada mensaje."
                                    } else {
                                        "Confirmación previa DESACTIVADA: enviaré los mensajes directamente (siempre bajo el limitador anti-spam) hasta que me digas que vuelva a confirmar."
                                    }
                                })
                                .to_string()
                            }
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "list_pending" {
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            let pending = crate::whatsapp::list_pending_messages(&db);
                            serde_json::json!({
                                "status": "ok",
                                "count": pending.len(),
                                "pending": pending,
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "cancel_pending" {
                    let target = args["pending_id"].as_str();
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => match crate::whatsapp::cancel_pending_messages(&db, target) {
                            Ok(removed) => serde_json::json!({
                                "status": "cancelled",
                                "removed_count": removed,
                                "remaining": crate::whatsapp::list_pending_messages(&db),
                            })
                            .to_string(),
                            Err(e) => serde_json::json!({
                                "error": "cancel_failed",
                                "detail": e.to_string()
                            })
                            .to_string(),
                        },
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "save_contact" {
                    let alias = args["alias"]
                        .as_str()
                        .or_else(|| args["name"].as_str())
                        .or_else(|| args["search"].as_str())
                        .unwrap_or("");
                    let target = args["to"]
                        .as_str()
                        .or_else(|| args["phone"].as_str())
                        .or_else(|| args["chat"].as_str())
                        .unwrap_or("");
                    let note = args["note"].as_str().or_else(|| args["message"].as_str());
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => match crate::whatsapp::save_contact_alias(
                            &db, alias, target, note,
                        ) {
                            Ok(entry) => serde_json::json!({
                                "status": "saved",
                                "contact": entry,
                                "total_aliases": crate::whatsapp::list_contact_aliases(&db).len(),
                            })
                            .to_string(),
                            Err(e) => serde_json::json!({
                                "error": "save_contact_failed",
                                "detail": e.to_string(),
                            })
                            .to_string(),
                        },
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string(),
                        })
                        .to_string(),
                    };
                }

                if action == "list_contacts" {
                    let search = args["search"].as_str();
                    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
                    let wa_opt = self.whatsapp.read().await.clone();
                    if let Some(wa) = wa_opt {
                        return match wa
                            .list_contacts(&tenant_id, session_key, search, limit)
                            .await
                        {
                            Ok(val) => val.to_string(),
                            Err(e) => serde_json::json!({
                                "error": "whatsapp_failed",
                                "detail": e.to_string(),
                            })
                            .to_string(),
                        };
                    }
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => serde_json::json!({
                            "saved_aliases": crate::whatsapp::list_contact_aliases(&db),
                            "bridge_connected": false,
                            "bridge_contacts": [],
                        })
                        .to_string(),
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string(),
                        })
                        .to_string(),
                    };
                }

                let wa_opt = self.whatsapp.read().await.clone();
                let Some(wa) = wa_opt else {
                    return "{\"error\":\"whatsapp_manager_unavailable\",\"detail\":\"El gestor de WhatsApp no está inicializado en el kernel.\"}".to_string();
                };

                let res = match action.as_str() {
                    "status" => wa.get_status(&tenant_id, session_key, true).await,
                    "list_chats" => {
                        let search = args["search"].as_str();
                        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                        wa.list_chats(&tenant_id, search, limit).await
                    }
                    "read_messages" => {
                        let chat = args["chat"]
                            .as_str()
                            .or_else(|| args["to"].as_str())
                            .unwrap_or("");
                        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                        wa.read_messages_with_aliases(&tenant_id, session_key, chat, limit)
                            .await
                    }
                    "send_message" => {
                        let to = args["to"].as_str().unwrap_or("");
                        let msg = args["message"].as_str().unwrap_or("");
                        wa.send_or_queue_message(&tenant_id, session_key, to, msg)
                            .await
                    }
                    "confirm_send" => {
                        let pid = args["pending_id"].as_str();
                        wa.confirm_pending_and_send(&tenant_id, session_key, pid)
                            .await
                    }
                    _ => Err(anyhow::anyhow!(
                        "Acción de WhatsApp desconocida: {}",
                        action
                    )),
                };

                match res {
                    Ok(val) => val.to_string(),
                    Err(e) => serde_json::json!({
                        "error": "whatsapp_failed",
                        "detail": e.to_string()
                    })
                    .to_string(),
                }
            }

            other => {
                // Módulos de dominio del microkernel (Aegis Creator Studio, Video Studio, Dev Studio, Business & POS, etc.)
                if let Some(router_rw) = self.router_ref.read().await.clone() {
                    let (manifest_opt, key_pool_arc) = {
                        let router_guard = router_rw.read().await;
                        (
                            router_guard.find_module_for_tool(other).await,
                            router_guard.key_pool_arc(),
                        )
                    };
                    if let Some(manifest) = manifest_opt {
                        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                            return "{\"error\":\"missing_session_key\"}".to_string();
                        };
                        if let Ok(db) = crate::enclave::TenantDB::open(tenant_id, session_key) {
                            let active_key = format!("module_active:{}", manifest.module_id);
                            if db.get_kv(&active_key).unwrap_or(None).as_deref() != Some("true") {
                                let _ = db.set_kv(&active_key, "true");
                            }
                        }
                        let mut video_ctx = crate::video_studio::VideoExecContext {
                            key_pool: Some(key_pool_arc),
                            router: Some(router_rw.clone()),
                            ..Default::default()
                        };
                        if let Some(ptx) = progress_tx {
                            let tx_clone = ptx.clone();
                            let cb: crate::video_studio::types::ProgressCallback =
                                Arc::new(move |evt| {
                                    if let Ok(payload) = serde_json::to_string(&evt) {
                                        let _ =
                                            tx_clone.send(Ok(format!("__PROGRESS__{}", payload)));
                                    }
                                });
                            video_ctx.progress_cb = Some(cb);
                        }
                        if pcb.agent_id.is_none()
                            && other == "video_create_project"
                            && args.get("auto_render").is_none()
                            && !args
                                .get("draft_only")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                        {
                            if let Some(obj) = args.as_object_mut() {
                                obj.insert("auto_render".to_string(), serde_json::json!(true));
                            }
                        }
                        let args_str = args.to_string();
                        return match crate::router::modules::execute_module_tool_with_context(
                            &manifest,
                            other,
                            &args_str,
                            tenant_id,
                            session_key,
                            Some(&video_ctx),
                        )
                        .await
                        {
                            Ok(res) => res,
                            Err(e) => serde_json::json!({
                                "error": "module_tool_failed",
                                "module_id": manifest.module_id,
                                "detail": e,
                            })
                            .to_string(),
                        };
                    }
                }

                // CORE-377: Check if the tool is registered in the MCP Registry
                if self.mcp_registry.has_tool(other).await {
                    // CORE-377 Criterion 4: Aislamiento de permisos contra políticas de tenant / Citadel
                    if let Some(tenant_id) = &pcb.tenant_id {
                        if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                            if let Ok(db) = crate::enclave::TenantDB::open(tenant_id, session_key) {
                                let tool_policy_key = format!("mcp_tool_policy:{}", other);
                                if let Ok(Some(policy)) = db.get_kv(&tool_policy_key) {
                                    if policy == "deny" {
                                        tracing::warn!(
                                            pid = %_pid,
                                            tool = %other,
                                            tenant = %tenant_id,
                                            "CORE-377: MCP tool call denied by tenant policy"
                                        );
                                        return format!(
                                            "{{\"error\": \"Permission denied: tool '{}' is restricted for this tenant.\"}}",
                                            other
                                        );
                                    }
                                }
                            }
                        }
                    }

                    match ank_mcp::registry::McpToolDispatcher::execute(
                        &self.mcp_registry,
                        other,
                        args,
                    )
                    .await
                    {
                        Ok(val) => {
                            tracing::info!(
                                pid = %_pid,
                                tool = %other,
                                "CORE-377: MCP tool successfully executed"
                            );
                            val.to_string()
                        }
                        Err(e) => {
                            tracing::error!(
                                pid = %_pid,
                                tool = %other,
                                error = %e,
                                "CORE-377: MCP tool execution failed"
                            );
                            format!(
                                "{{\"error\":\"mcp_execution_failed\",\"detail\":\"{}\"}}",
                                e
                            )
                        }
                    }
                } else {
                    format!("{{\"error\":\"Unknown tool: {}\"}}", other)
                }
            }
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

    #[tokio::test]
    async fn test_build_messages_for_chat_compact_on_greeting() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test".into(), 5, "hola!".into());
        assert!(is_casual_chat_turn(&pcb));

        let (messages, _warnings) = hal.build_messages_for_chat_with_warnings(&pcb, None).await;
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

        assert!(!system_msg.contains("HERRAMIENTAS"));
        assert!(!system_msg.contains("SYS_CALL_MAKER"));
        assert!(!system_msg.contains("SYS_CALL_PLUGIN"));
        assert!(!user_msg.contains("Aegis Neural Kernel VCM"));

        let pcb_query = PCB::new("test".into(), 5, "que modelos tenes disponibles?".into());
        assert!(!is_casual_chat_turn(&pcb_query));
        let (query_msgs, _) = hal
            .build_messages_for_chat_with_warnings(&pcb_query, None)
            .await;
        let query_sys = query_msgs
            .iter()
            .find(|m| m.role == ChatRole::System)
            .unwrap()
            .content
            .as_ref()
            .unwrap();
        assert!(!query_sys.contains("SYS_CALL_PLUGIN(\"ledger\""));
        assert!(!query_sys.contains("SYS_CALL_MAKER"));
        Ok(())
    }

    #[test]
    fn test_is_model_scoped_failure_isolated() {
        assert!(is_model_scoped_failure(&SystemError::HardwareFailure(
            "API Error 402 Payment Required: You requested up to 131072 tokens, but can only afford 2182".into()
        )));
        assert!(is_model_scoped_failure(&SystemError::HardwareFailure(
            "API Error 404 Not Found: models/gemini-2.5-flash-lite is no longer available to new users".into()
        )));
        assert!(!is_model_scoped_failure(&SystemError::HardwareFailure(
            "API Error 503 Service Unavailable".into()
        )));
        assert!(!is_model_scoped_failure(&SystemError::HardwareFailure(
            "Cloud API Error 401 Unauthorized".into()
        )));
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
        let env = crate::enclave::ScopedTestDataDir::new()?;
        let workspace_dir = env
            .path()
            .join("users")
            .join("test_tenant")
            .join("workspace");
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

        // Call the helper directly
        let section = CognitiveHAL::get_dynamic_scripts_section("test_tenant").await;

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

    #[test]
    fn test_chat_agent_guard_helpers() {
        // 1. Assistant claims delegation
        let sol_hallucination = "¡Entendido, mi cielo! Perfecto, ya le estoy pidiendo al equipazo de \"Aegis\" que se ponga a programar eso para ti.";
        assert!(CognitiveHAL::assistant_claims_delegation(sol_hallucination));

        let empujoncito = "¡Dale, mi vida! Ya les di un buen empujoncito al equipo de Aegis para que se apuren con ese script de Python para el disco C:!";
        assert!(CognitiveHAL::assistant_claims_delegation(empujoncito));

        let normal_chat = "Hola! ¿En qué te puedo ayudar hoy?";
        assert!(!CognitiveHAL::assistant_claims_delegation(normal_chat));

        // 2. Status queries
        assert!(CognitiveHAL::is_status_query("que paso?"));
        assert!(CognitiveHAL::is_status_query(
            "¿Cómo va el estado del proyecto?"
        ));
        assert!(CognitiveHAL::is_status_query("cómo va?"));
        assert!(CognitiveHAL::is_status_query("ya esta?"));
        assert!(CognitiveHAL::is_status_query("ya esta listo?"));
        assert!(!CognitiveHAL::is_status_query("ya esta conectado"));
        assert!(!CognitiveHAL::is_status_query("whatsapp ya esta conectado"));
        assert!(!CognitiveHAL::is_status_query("Crea un script en Python"));

        // 2a. Video production routing vs technical delegation guard
        let vid_prompt_1 = "Creame un proyecto nuevo vertical 9:16 con preset gemini_studio, voz Charon y 4 escenas promocionando Aegis: 2) Activá solo tus módulos de código, video o negocio";
        assert!(CognitiveHAL::is_video_production_request(vid_prompt_1));
        assert!(!CognitiveHAL::is_technical_request(vid_prompt_1));
        let vid_prompt_2 = "Creame un proyecto nuevo vertical 9:16 con preset free_builtin, voz en-US-AndrewMultilingualNeural y 4 escenas";
        assert!(CognitiveHAL::is_video_production_request(vid_prompt_2));
        assert!(!CognitiveHAL::is_video_production_request(
            "que paso con el video?"
        ));
        let advisory_prompt = "que necesitas para que el video salga mejor? otras apis?";
        assert!(CognitiveHAL::is_video_advisory_or_followup_query(
            advisory_prompt
        ));
        assert!(!CognitiveHAL::is_technical_request(advisory_prompt));
        assert!(!CognitiveHAL::has_technical_context(advisory_prompt, &[]));
        assert!(!CognitiveHAL::is_technical_request(
            "por que salio con menos escenas el video?"
        ));
        let vid_history = vec![ChatMessage {
            role: ChatRole::User,
            content: Some(vid_prompt_2.to_string()),
            ..Default::default()
        }];
        assert!(CognitiveHAL::has_video_context(
            "y donde esta?",
            &vid_history
        ));
        assert!(CognitiveHAL::has_video_context(
            "lo vas a hacer o vas a seguir durmiendo?",
            &vid_history
        ));
        assert_eq!(
            CognitiveHAL::resolve_video_request_prompt(
                "y donde esta?",
                &vid_history,
                "Aquí tienes tu video, voy a llamar a la herramienta video_produce_auto"
            )
            .as_deref(),
            Some(vid_prompt_2)
        );
        assert!(CognitiveHAL::assistant_claims_video_execution(
            "Aquí va la llamada a la herramienta. ¡Cruzá los dedos!"
        ));
        assert!(CognitiveHAL::is_technical_request(
            "Escribí un script en Python para ordenar archivos"
        ));

        // 2b. Stream sanitizer & paragraph break helper
        let mut san = StreamTextSanitizer::default();
        let mut combined = String::new();
        for piece in [
            "<thi",
            "nk>razonamiento ",
            "oculto</thi",
            "nk>Hola <|call",
            "|>Aegis ",
            "(x < 5)!",
        ] {
            combined.push_str(&san.push(piece));
        }
        combined.push_str(&san.flush());
        assert_eq!(combined, "Hola Aegis (x < 5)!");

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut assistant_text = String::new();
        let mut has_streamed = false;
        let mut ends_nl = false;
        let mut needs_break = false;
        emit_stream_text_chunk(
            &tx,
            "🤔".into(),
            true,
            &mut assistant_text,
            &mut has_streamed,
            &mut ends_nl,
            &mut needs_break,
        );
        needs_break = has_streamed && !ends_nl;
        emit_stream_text_chunk(
            &tx,
            "¡Buenas noticias!".into(),
            true,
            &mut assistant_text,
            &mut has_streamed,
            &mut ends_nl,
            &mut needs_break,
        );
        drop(tx);
        let mut streamed_out = String::new();
        while let Ok(Ok(tok)) = rx.try_recv() {
            streamed_out.push_str(&tok);
        }
        assert_eq!(streamed_out, "🤔\n\n¡Buenas noticias!");

        // 3. Technical requests
        assert!(CognitiveHAL::is_technical_request(
            "Crea un script en Python que verifique el espacio en disco"
        ));
        assert!(CognitiveHAL::is_technical_request(
            "implementá un endpoint de login"
        ));
        assert!(!CognitiveHAL::is_technical_request("hola cómo estás"));

        // 4. Contextual follow-up
        let history = vec![
            ChatMessage {
                role: ChatRole::User,
                content: Some(
                    "Crea un script en Python que verifique el espacio en disco".to_string(),
                ),
                ..Default::default()
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: Some("¿En qué disco querés enfocarlo?".to_string()),
                ..Default::default()
            },
        ];
        assert!(CognitiveHAL::has_technical_context("si, disco c", &history));
        assert!(!CognitiveHAL::has_technical_context("hola", &[]));

        let scope = CognitiveHAL::derive_task_scope("si, disco c", &history);
        assert!(scope.contains("Crea un script en Python"));
        assert!(scope.contains("si, disco c"));

        let name = CognitiveHAL::derive_task_project_name(&scope);
        assert_eq!(name, "Script Espacio Disco");
    }

    #[test]
    fn test_summarize_configured_providers_free_tier_and_dedup() {
        use crate::router::catalog::{ModelEntry, TaskScores, ToolUseSupport};
        use crate::router::key_pool::ApiKeyEntry;

        let make_model = |id: &str, provider: &str, cost: f64, free_ok: bool| ModelEntry {
            model_id: id.to_string(),
            provider: provider.to_string(),
            display_name: id.to_string(),
            context_window: 128_000,
            cost_input_per_mtok: cost,
            cost_output_per_mtok: cost,
            supports_tools: true,
            supports_json_mode: false,
            tool_use_support: ToolUseSupport::Unknown,
            task_scores: TaskScores::default(),
            is_local: false,
            avg_latency_ms: None,
            free_tier_rpm: None,
            free_tier_rpd: None,
            free_tier_eligible: free_ok,
        };

        let catalog = vec![
            make_model("openrouter/free", "openrouter", 0.0, true),
            make_model("openrouter/auto", "openrouter", 0.0, false),
            make_model("anthropic/claude-sonnet-4-6", "openrouter", 3.0, false),
            make_model("openai/gpt-4o", "openrouter", 2.5, false),
            make_model(
                "meta-llama/llama-3.3-70b-instruct:free",
                "openrouter",
                0.0,
                true,
            ),
            make_model("openai/gpt-oss-20b", "groq", 0.0, true),
        ];

        let keys = vec![
            ApiKeyEntry {
                key_id: "or-env".to_string(),
                provider: "openrouter".to_string(),
                api_key: "sk-or-test-1".to_string(),
                api_url: None,
                label: Some("Env (openrouter)".to_string()),
                is_active: true,
                rate_limited_until: None,
                active_models: None,
                is_free_tier: true,
            },
            ApiKeyEntry {
                key_id: "groq-1".to_string(),
                provider: "groq".to_string(),
                api_key: "gsk-1".to_string(),
                api_url: None,
                label: Some("Chat (groq)".to_string()),
                is_active: true,
                rate_limited_until: None,
                active_models: Some(vec![
                    "openai/gpt-oss-20b".to_string(),
                    "allam-2-7b".to_string(),
                ]),
                is_free_tier: true,
            },
            ApiKeyEntry {
                key_id: "groq-2".to_string(),
                provider: "groq".to_string(),
                api_key: "gsk-2".to_string(),
                api_url: None,
                label: Some("Chat (groq)".to_string()),
                is_active: true,
                rate_limited_until: None,
                active_models: Some(vec![
                    "allam-2-7b".to_string(),
                    "openai/gpt-oss-20b".to_string(),
                ]),
                is_free_tier: true,
            },
        ];

        let summary = summarize_configured_providers(keys, &catalog);
        assert_eq!(summary.len(), 2, "Duplicate groq keys must collapse");

        let or_entry = summary
            .iter()
            .find(|v| v["provider"] == "openrouter")
            .expect("openrouter entry missing");
        let or_models: Vec<&str> = or_entry["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str())
            .collect();
        assert!(or_models.contains(&"openrouter/free"));
        assert!(!or_models.contains(&"openrouter/auto"));
        assert!(!or_models.contains(&"anthropic/claude-sonnet-4-6"));
        assert!(!or_models.contains(&"openai/gpt-4o"));

        let groq_entry = summary
            .iter()
            .find(|v| v["provider"] == "groq")
            .expect("groq entry missing");
        assert_eq!(groq_entry["key_count"], 2);
    }

    #[test]
    fn test_mask_api_keys_in_text_preserves_multiline_markdown() {
        let input = "Resumen:\n\n| Proveedor | Clave |\n|---|---|\n| groq | `gsk_abcdefghijklmnopqrstuvwxyz1234` |\n| hex | 0123456789abcdef0123456789abcdef |\n\n> Nota final\n  - Item indentado";
        let masked = CognitiveHAL::mask_api_keys_in_text(input);

        assert!(!masked.contains("gsk_abcdefghijklmnopqrstuvwxyz1234"));
        assert!(!masked.contains("0123456789abcdef0123456789abcdef"));
        assert!(masked.contains("\n\n| Proveedor | Clave |\n|---|---|\n| groq | "));
        assert!(masked.contains("\n\n> Nota final\n  - Item indentado"));
    }
}
