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

/// --- INFERENCE DRIVER INTERFACE ---
#[async_trait]
pub trait InferenceDriver: Send + Sync {
    async fn generate_stream(
        &self,
        prompt: String,
        grammar: Option<Grammar>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> GenerateStreamResult;

    async fn get_health_status(&self) -> DriverStatus;

    async fn load_model(&mut self, model_id: &str) -> Result<(), SystemError>;
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
        })
    }

    pub async fn set_router(&self, router: Arc<RwLock<CognitiveRouter>>) {
        let mut r = self.router.write().await;
        *r = Some(router);
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
        &self,
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

        // Try CognitiveRouter first if available
        let router_opt = self.router.read().await.clone();
        if let Some(router_rw) = router_opt {
            let router = router_rw.read().await;
            let pcb_snapshot = {
                let pcb = shared_pcb.read().await;
                pcb.clone()
            };
            match router.decide(&pcb_snapshot).await {
                Ok(decision) => {
                    return self
                        .execute_with_decision(decision, &pcb_snapshot, &pid, persona.as_deref())
                        .await;
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

        let final_prompt = self.build_prompt(&pcb_snapshot, persona.as_deref()).await;
        driver.generate_stream(final_prompt, None, None).await
    }

    async fn execute_with_decision(
        &self,
        decision: RoutingDecision,
        pcb: &PCB,
        pid: &str,
        persona: Option<&str>,
    ) -> GenerateStreamResult {
        use crate::chal::drivers::CloudProxyDriver;

        tracing::info!(
            pid = %pid,
            model = %decision.model_id,
            provider = %decision.provider,
            "CognitiveRouter: routing to model"
        );

        let driver = CloudProxyDriver::new(
            Arc::clone(&self.http_client),
            decision.api_url.clone(),
            decision.api_key.clone(),
            decision.model_id.clone(),
        );

        let final_prompt = self.build_prompt(pcb, persona).await;

        // CORE-240: Obtener tools del ToolRegistry si el PCB es un agente del árbol
        let tools = if pcb.agent_id.is_some() {
            // Determinar el rol del agente — por defecto ChatAgent
            let role = AgentRole::ChatAgent;
            let provider = ProviderKind::from_string(&decision.provider);
            let tool_defs = ToolRegistry::tools_for(&role, &provider);
            if tool_defs.is_empty() {
                None
            } else {
                Some(tool_defs)
            }
        } else {
            // El ChatAgent principal (sin agent_id) también recibe tools
            let provider = ProviderKind::from_string(&decision.provider);
            let tool_defs = ToolRegistry::tools_for(&AgentRole::ChatAgent, &provider);
            if tool_defs.is_empty() {
                None
            } else {
                Some(tool_defs)
            }
        };

        match driver
            .generate_stream(final_prompt.clone(), None, tools.clone())
            .await
        {
            Ok(stream) => Ok(stream),
            Err(e) => {
                for fallback in &decision.fallback_chain {
                    warn!(
                        pid = %pid,
                        model = %fallback.model_id,
                        "CognitiveRouter: primary failed, trying fallback"
                    );
                    let fallback_driver = CloudProxyDriver::new(
                        Arc::clone(&self.http_client),
                        fallback.api_url.clone(),
                        fallback.api_key.clone(),
                        fallback.model_id.clone(),
                    );
                    // Re-compute tools for fallback provider
                    let fallback_tools = {
                        let provider = ProviderKind::from_string(&fallback.provider);
                        let role = AgentRole::ChatAgent;
                        let defs = ToolRegistry::tools_for(&role, &provider);
                        if defs.is_empty() {
                            None
                        } else {
                            Some(defs)
                        }
                    };
                    if let Ok(stream) = fallback_driver
                        .generate_stream(final_prompt.clone(), None, fallback_tools)
                        .await
                    {
                        return Ok(stream);
                    }
                }
                Err(e)
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
                    let test_prompt =
                        "[TOOL_USE_PROBE] Respond with a tool call if supported.".to_string();
                    match driver.generate_stream(test_prompt, None, None).await {
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

    /// Construye el prompt final para el LLM usando VCM para ensamblar contexto.
    pub async fn build_prompt(&self, pcb: &PCB, persona: Option<&str>) -> String {
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
            "build_prompt: role instructions loaded"
        );

        let final_prompt = if tool_prompt.trim().is_empty() && mcp_tool_prompt.trim().is_empty() {
            format!(
                "{}{}{}\n\n{}",
                role_instructions, persona_section, music_section, assembled_context
            )
        } else {
            format!(
                "{}{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}\n{}\n\nCONTENIDO ENSAMBLADO (CONTEXTO):\n{}",
                role_instructions,
                persona_section,
                music_section,
                tool_prompt,
                mcp_tool_prompt,
                assembled_context
            )
        };

        let has_maker_plugin = self.plugin_manager.read().await.is_plugin_active("maker");

        let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
            MAKER_INSTRUCTIONS
        } else {
            ""
        };

        // CORE-150: Maker Instructions
        // CORE-154: Multi-Agent Instructions
        format!("{}{}{}", maker_section, SPAWN_INSTRUCTIONS, final_prompt)
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
        _prompt: String,
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
        let hal = CognitiveHAL::new(pm)?;
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
        let hal = CognitiveHAL::new(pm)?;
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
    async fn test_build_prompt_default_tools_presence() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test".into(), 5, "hola".into());
        let prompt = hal.build_prompt(&pcb, None).await;
        assert!(
            !prompt.contains("[USER_PROCESS_INSTRUCTION]"),
            "El prompt no debe contener el tag USER_PROCESS_INSTRUCTION"
        );
        assert!(
            prompt.contains("HERRAMIENTAS (PLUGINS) DISPONIBLES:"),
            "Deben aparecer los plugins de dominio por defecto"
        );
        assert!(
            prompt.contains("ledger") && prompt.contains("chronos"),
            "Debe contener las herramientas ledger y chronos"
        );
        assert!(
            prompt.contains("hola"),
            "El prompt debe contener la instrucción"
        );
        // CORE-148: Music instructions are only injected if plugin is active.
        assert!(
            !prompt.contains("MÚSICA"),
            "Music instructions must NOT be present if plugin is not active (CORE-148)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_build_prompt_with_persona() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let pcb = PCB::new("test".into(), 5, "hola".into());
        let prompt = hal
            .build_prompt(&pcb, Some("Eres Eve, asistente de ACME Corp."))
            .await;
        assert!(prompt.contains("Eve"), "El prompt debe contener la persona");
        assert!(
            prompt.contains("hola"),
            "El prompt debe contener la instrucción"
        );
        Ok(())
    }
}
