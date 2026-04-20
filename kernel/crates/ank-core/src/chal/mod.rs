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

/// --- SYSTEM PROMPT CONSTANTS ---
pub const SYSTEM_PROMPT_MASTER: &str = r#"[AEGIS NEURAL KERNEL - ISA v1.0]
Eres una ALU Cognitiva (Unidad Lógica Aritmética) operando dentro del Aegis Neural Kernel.
Tu objetivo es ejecutar procesos con precisión y claridad.
No saludes. No pidas disculpas. No uses frases de relleno.
Responde directamente a la instrucción del usuario.

REGLAS DE EJECUCIÓN:
1. Si necesitas usar una herramienta, detén tu generación e inserta una Syscall.
2. Formato de Syscall: [SYS_CALL_PLUGIN("nombre_plugin", {"clave": "valor"})]
3. Solo puedes usar los plugins listados a continuación.
4. Si no hay plugins disponibles o la instrucción no requiere herramientas, responde directamente en lenguaje natural.
"#;

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
                        .execute_with_decision(decision, &instruction, &pid)
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

        let final_prompt = self.build_prompt(&instruction).await;
        driver.generate_stream(&final_prompt, None).await
    }

    async fn execute_with_decision(
        &self,
        decision: RoutingDecision,
        instruction: &str,
        pid: &str,
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

        let final_prompt = self.build_prompt(instruction).await;

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
    ///
    /// CORE-123: El tag [USER_PROCESS_INSTRUCTION] fue removido como separador
    /// de prompt — el modelo lo interpretaba como señal para generar una syscall
    /// en lugar de responder en lenguaje natural.
    /// La instrucción del usuario se inyecta directamente después del system prompt.
    async fn build_prompt(&self, instruction: &str) -> String {
        let tool_prompt = self
            .plugin_manager
            .read()
            .await
            .get_available_tools_prompt();
        let mcp_tool_prompt = self.mcp_registry.generate_system_prompt().await;

        // Si no hay tools disponibles, prompt limpio sin sección de syscalls
        // para evitar que el LLM genere syscalls innecesarias
        if tool_prompt.trim().is_empty() && mcp_tool_prompt.trim().is_empty() {
            format!("{}\n\n{}", SYSTEM_PROMPT_MASTER, instruction)
        } else {
            format!(
                "{}\n\nHERRAMIENTAS DISPONIBLES:\n{}\n{}\n\nINSTRUCCIÓN:\n{}",
                SYSTEM_PROMPT_MASTER, tool_prompt, mcp_tool_prompt, instruction
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

        let stream_res = hal.route_and_execute(shared_pcb).await?;
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

        let stream_res = hal.route_and_execute(shared_pcb).await?;
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

        let prompt = hal.build_prompt("hola").await;

        // Sin tools, el prompt NO debe contener el tag que confunde al LLM
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
}
