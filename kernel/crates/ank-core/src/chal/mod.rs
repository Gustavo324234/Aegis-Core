use crate::plugins::PluginManager;
use crate::router::{CognitiveRouter, RoutingDecision};
use crate::scheduler::{ModelPreference, SharedPCB};
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex as TokioMutex, RwLock};
use tokio_stream::Stream;
use tracing::{info, warn};

pub mod drivers;
pub mod hardware;

/// --- SYSTEM PROMPT ---
/// CORE-128: System prompt base honesto y sin alucinaciones.
/// - No inventa capacidades que no tiene mediante herramientas activas.
/// - No inventa acciones que no ejecutó.
/// - Sin listas innecesarias en respuestas conversacionales.
/// - Identidad: "Aegis" por defecto; la Persona del tenant la sobreescribe (CORE-129).
pub const SYSTEM_PROMPT_MASTER: &str = "Eres Aegis, un asistente de IA.\n\
Responde en el idioma del usuario. Sé directo y conciso.\n\
REGLAS CRÍTICAS:\n\
- Solo afirma que hiciste algo si una herramienta te devolvió un resultado concreto. \
Nunca digas \"he registrado\", \"he guardado\" o \"queda anotado\" si no ejecutaste \
una herramienta que lo confirme.\n\
- Describe únicamente las capacidades que tus herramientas activas te permiten ejecutar. \
Si no hay herramientas de finanzas, no afirmes que podés llevar un registro de gastos.\n\
- Usa prosa directa. Evita listas numeradas o con viñetas salvo que el usuario \
las pida explícitamente o el contenido sea inherentemente una lista.\n";

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

/// --- INFERENCE DRIVER INTERFACE ---
#[async_trait]
pub trait InferenceDriver: Send + Sync {
    async fn generate_stream(
        &self,
        prompt: &str,
        grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>;

    async fn get_health_status(&self) -> DriverStatus;

    async fn load_model(&mut self, model_id: &str) -> Result<(), SystemError>;
}

/// --- COGNITIVE HAL (Hardware Abstraction Layer) ---
pub struct CognitiveHAL {
    pub drivers: HashMap<String, Box<dyn InferenceDriver>>,
    pub plugin_manager: Arc<RwLock<PluginManager>>,
    pub mcp_registry: Arc<ank_mcp::registry::McpToolRegistry>,
    pub router: Option<Arc<RwLock<CognitiveRouter>>>,
    pub hardware: TokioMutex<hardware::HardwareMonitor>,
    pub http_client: Arc<reqwest::Client>,
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

        let mut drivers: HashMap<String, Box<dyn InferenceDriver>> = HashMap::new();

        if let Some(cloud_driver) =
            crate::chal::drivers::CloudProxyDriver::from_env(Arc::clone(&http_client))
        {
            drivers.insert("cloud-driver".to_string(), Box::new(cloud_driver));
            tracing::info!("CloudProxyDriver initialized via ENV vars and registered.");
        }

        Ok(Self {
            drivers,
            plugin_manager,
            mcp_registry: Arc::new(ank_mcp::registry::McpToolRegistry::new()),
            router: None,
            hardware: TokioMutex::new(hardware::HardwareMonitor::new()),
            http_client,
        })
    }

    pub fn set_router(&mut self, router: Arc<RwLock<CognitiveRouter>>) {
        self.router = Some(router);
    }

    pub fn register_driver(&mut self, id: &str, driver: Box<dyn InferenceDriver>) {
        self.drivers.insert(id.to_string(), driver);
        tracing::info!(driver_id = %id, "New driver registered in HAL.");
    }

    pub fn update_cloud_credentials(&mut self, api_url: String, model: String, api_key: String) {
        let cloud_driver = crate::chal::drivers::CloudProxyDriver::new(
            Arc::clone(&self.http_client),
            api_url,
            api_key,
            model.clone(),
        );
        self.drivers
            .insert("cloud-driver".to_string(), Box::new(cloud_driver));
        tracing::info!(model = %model, "CloudProxyDriver credentials updated dynamically and driver re-registered in HAL.");
    }

    pub async fn route_and_execute(
        &self,
        shared_pcb: SharedPCB,
        persona: Option<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
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
        if let Some(router_rw) = &self.router {
            let router = router_rw.read().await;
            let pcb_snapshot = {
                let pcb = shared_pcb.read().await;
                pcb.clone()
            };
            match router.decide(&pcb_snapshot).await {
                Ok(decision) => {
                    return self
                        .execute_with_decision(decision, &instruction, &pid, persona.as_deref())
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
                let has_local_driver = self.drivers.contains_key("local-driver");
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

        let driver = self.drivers.get(driver_id).ok_or_else(|| {
            if driver_id == "cloud-driver" {
                SystemError::HardwareFailure(
                    "Driver cloud no configurado o sin credenciales.".to_string(),
                )
            } else {
                SystemError::DriverOffline(driver_id.to_string())
            }
        })?;

        let final_prompt = self.build_prompt(&instruction, persona.as_deref()).await;
        driver.generate_stream(&final_prompt, None).await
    }

    async fn execute_with_decision(
        &self,
        decision: RoutingDecision,
        instruction: &str,
        pid: &str,
        persona: Option<&str>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
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

        let final_prompt = self.build_prompt(instruction, persona).await;

        match driver.generate_stream(&final_prompt, None).await {
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
                    if let Ok(stream) = fallback_driver.generate_stream(&final_prompt, None).await {
                        return Ok(stream);
                    }
                }
                Err(e)
            }
        }
    }

    /// Construye el prompt final para el LLM.
    /// CORE-123: sin tag [USER_PROCESS_INSTRUCTION] que confundía al modelo.
    /// CORE-128: system prompt honesto sin alucinaciones.
    pub async fn build_prompt(&self, instruction: &str, persona: Option<&str>) -> String {
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

        let music_section = if std::env::var("YOUTUBE_API_KEY").is_ok() {
            "\n\nMÚSICA — INSTRUCCIONES:\
             \n- Para reproducir: [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista canción>\", \"max_results\": 1})] y luego [MUSIC_PLAY:<video_id>]\
             \n- Para pausar: responde brevemente y termina con [MUSIC_PAUSE]\
             \n- Para continuar: responde brevemente y termina con [MUSIC_RESUME]\
             \n- Para detener: responde brevemente y termina con [MUSIC_STOP]\
             \n- Para cambiar volumen: termina con [MUSIC_VOLUME:<0-100>]\
             \n- Para cambiar canción: haz una nueva búsqueda y usa [MUSIC_PLAY:<nuevo_video_id>]\
             \nNunca expliques estos tags al usuario. Solo úsalos.\n"
        } else {
            ""
        };

        if tool_prompt.trim().is_empty() && mcp_tool_prompt.trim().is_empty() {
            format!(
                "{}{}{}\n\n{}",
                SYSTEM_PROMPT_MASTER, persona_section, music_section, instruction
            )
        } else {
            format!(
                "{}{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}\n{}\n\nMENSAJE DEL USUARIO:\n{}",
                SYSTEM_PROMPT_MASTER,
                persona_section,
                music_section,
                tool_prompt,
                mcp_tool_prompt,
                instruction
            )
        }
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
        _prompt: &str,
        _grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
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
        let mut hal = CognitiveHAL::new(pm)?;
        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        );
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        );
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
        let mut hal = CognitiveHAL::new(pm)?;
        hal.register_driver(
            "local-driver",
            Box::new(DummyDriver {
                name: "local".to_string(),
            }),
        );
        hal.register_driver(
            "cloud-driver",
            Box::new(DummyDriver {
                name: "cloud".to_string(),
            }),
        );
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
    async fn test_build_prompt_no_tools_is_clean() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let prompt = hal.build_prompt("hola", None).await;
        assert!(
            !prompt.contains("[USER_PROCESS_INSTRUCTION]"),
            "El prompt no debe contener el tag USER_PROCESS_INSTRUCTION"
        );
        assert!(
            !prompt.contains("HERRAMIENTAS DISPONIBLES"),
            "Sin tools, no debe haber sección de herramientas"
        );
        assert!(
            prompt.contains("hola"),
            "El prompt debe contener la instrucción"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_build_prompt_with_persona() -> anyhow::Result<()> {
        let pm = Arc::new(RwLock::new(PluginManager::new()?));
        let hal = CognitiveHAL::new(pm)?;
        let prompt = hal
            .build_prompt("hola", Some("Eres Eve, asistente de ACME Corp."))
            .await;
        assert!(prompt.contains("Eve"), "El prompt debe contener la persona");
        assert!(
            prompt.contains("hola"),
            "El prompt debe contener la instrucción"
        );
        Ok(())
    }
}
