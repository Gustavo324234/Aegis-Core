use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Trait de Motor: Define la interfaz para motores de síntesis de voz (TTS).
#[async_trait]
pub trait SirenEngine: Send + Sync {
    /// Identificador único del motor (e.g., "voxtral", "mock").
    fn id(&self) -> &str;

    /// Sintetiza texto a audio PCM (bytes).
    async fn synthesize(&self, text: String) -> Result<Vec<u8>>;

    /// Transcribe audio PCM a texto (STT).
    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String> {
        // Path mínimo (LIM-004): Si el motor no tiene STT real, devolvemos success
        // con un placeholder para no romper el pipeline de chat.
        Ok("[audio received - STT pending]".to_string())
    }

    /// Clona una voz basada una muestra de audio y devuelve el VoiceID.
    async fn clone_voice(&self, _sample: Vec<u8>) -> Result<String> {
        Err(anyhow::anyhow!(
            "Voice cloning not supported by this engine"
        ))
    }
}

/// Mock de Compatibilidad: Mueve la lógica de ruido actual de tts.rs.
pub struct MockSirenEngine;

#[async_trait]
impl SirenEngine for MockSirenEngine {
    fn id(&self) -> &str {
        "mock"
    }

    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        info!("MockSirenEngine: Synthesizing '{}'", text);
        // Generar 1/4 segundo de PCM (22050Hz, 16-bit simulado con 8-bit noise para compatibilidad)
        let audio_len = 22050 * 2 / 4;
        let mut mock_audio = vec![0u8; audio_len];

        for (i, sample) in mock_audio.iter_mut().enumerate() {
            *sample = (i % 256) as u8;
        }
        Ok(mock_audio)
    }
}

use crate::StatePersistor;

/// SirenRouter: Resuelve el motor de voz basado en preferencias del tenant.
pub struct SirenRouter {
    engines: RwLock<HashMap<String, Arc<dyn SirenEngine>>>,
    persistence: Arc<dyn StatePersistor>,
}

impl SirenRouter {
    pub fn new(persistence: Arc<dyn StatePersistor>) -> Self {
        let mut engines: HashMap<String, Arc<dyn SirenEngine>> = HashMap::new();
        engines.insert("mock".to_string(), Arc::new(MockSirenEngine));

        // Auto-Register VoxtralDriver if environment variable is present (SRE requirement)
        if let Ok(voxtral) = crate::chal::drivers::VoxtralDriver::from_env() {
            engines.insert("voxtral".to_string(), Arc::new(voxtral));
            info!("SirenRouter: VoxtralDriver detected in environment and registered.");
        }

        // Auto-Register WhisperLocalEngine if a model is downloaded
        if let Some(whisper) = crate::chal::drivers::WhisperLocalEngine::from_env() {
            engines.insert("whisper-local".to_string(), Arc::new(whisper));
            info!("SirenRouter: WhisperLocalEngine detected and registered.");
        }

        Self {
            engines: RwLock::new(engines),
            persistence,
        }
    }

    /// Registra un nuevo motor en el router.
    pub async fn register_engine(&self, engine: Arc<dyn SirenEngine>) {
        let mut engines = self.engines.write().await;
        engines.insert(engine.id().to_string(), engine);
    }

    /// Resuelve el motor basado en el tenant_id con lógica de auto-fallback (SRE Goal).
    pub async fn resolve(&self, tenant_id: &str) -> Result<Arc<dyn SirenEngine>> {
        let profile = self
            .persistence
            .get_voice_profile(tenant_id)
            .await
            .ok()
            .flatten();

        let engines = self.engines.read().await;

        // 1. Intentar motor preferido del perfil
        if let Some(ref profile) = profile {
            // ElevenLabs: crear driver dinámicamente desde settings_json del tenant
            if profile.engine_id == "elevenlabs" {
                if let Ok(settings) =
                    serde_json::from_str::<serde_json::Value>(&profile.settings_json)
                {
                    if let Some(api_key) = settings["api_key"].as_str() {
                        if !api_key.is_empty() {
                            match crate::chal::drivers::ElevenLabsDriver::new(
                                api_key.to_string(),
                                profile.voice_id.clone(),
                            ) {
                                Ok(driver) => return Ok(Arc::new(driver)),
                                Err(e) => {
                                    warn!("SirenRouter: Failed to create ElevenLabsDriver: {}", e)
                                }
                            }
                        }
                    }
                }
                warn!("SirenRouter: ElevenLabs selected but no valid api_key in profile. Falling back.");
            } else if let Some(engine) = engines.get(&profile.engine_id) {
                return Ok(engine.clone());
            } else {
                warn!(
                    "SirenRouter: Profile found but engine '{}' not registered. Falling back.",
                    profile.engine_id
                );
            }
        }

        // 2. Fallback Automático: Intentar Voxtral si está registrado (Local First)
        if let Some(voxtral) = engines.get("voxtral") {
            return Ok(voxtral.clone());
        }

        // 3. Última instancia: Mock (Garantiza que el stream no se rompa)
        engines
            .get("mock")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("SirenRouter: No default 'mock' engine registered"))
    }

    /// Procesa audio crudo para un tenant eligiendo el STT engine configurado.
    /// Prioridad: groq (cloud) → whisper-local → engine del perfil → fallback mock.
    pub async fn process_audio(&self, tenant_id: &str, pcm_data: Vec<u8>) -> Result<String> {
        let profile = self
            .persistence
            .get_voice_profile(tenant_id)
            .await
            .ok()
            .flatten();

        let settings = profile.as_ref().and_then(|p| {
            serde_json::from_str::<serde_json::Value>(&p.settings_json).ok()
        });

        let stt_provider = settings
            .as_ref()
            .and_then(|s| s["stt_provider"].as_str())
            .unwrap_or("browser");

        info!(
            "SirenRouter: STT provider='{}', audio={} bytes",
            stt_provider,
            pcm_data.len()
        );

        match stt_provider {
            "groq" => {
                if let Some(api_key) = settings
                    .as_ref()
                    .and_then(|s| s["stt_api_key"].as_str())
                    .filter(|k| !k.is_empty())
                    .map(|s| s.to_string())
                {
                    match crate::chal::drivers::GroqSttEngine::new(api_key) {
                        Ok(engine) => return engine.transcribe(pcm_data).await,
                        Err(e) => warn!("SirenRouter: GroqSttEngine init failed: {}", e),
                    }
                } else {
                    warn!("SirenRouter: Groq STT selected but stt_api_key is empty.");
                }
            }
            "local" => {
                let engines = self.engines.read().await;
                if let Some(whisper) = engines.get("whisper-local") {
                    return whisper.transcribe(pcm_data).await;
                }
                warn!("SirenRouter: Local STT selected but WhisperLocalEngine not registered.");
            }
            // "browser" → el frontend maneja STT, el audio no debería llegar aquí.
            // Devolvemos vacío en lugar de el placeholder roto.
            _ => return Ok(String::new()),
        }

        // Último fallback si algo falló arriba
        let engines = self.engines.read().await;
        if let Some(whisper) = engines.get("whisper-local") {
            return whisper.transcribe(pcm_data).await;
        }

        Ok(String::new())
    }
}

// Removed Default impl as it requires persistence layer now.
