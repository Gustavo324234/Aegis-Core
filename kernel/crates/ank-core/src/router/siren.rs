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
        Ok("[audio received - STT pending]".to_string())
    }

    /// Clona una voz basada una muestra de audio y devuelve el VoiceID.
    async fn clone_voice(&self, _sample: Vec<u8>) -> Result<String> {
        Err(anyhow::anyhow!(
            "Voice cloning not supported by this engine"
        ))
    }
}

/// Mock de Compatibilidad
pub struct MockSirenEngine;

#[async_trait]
impl SirenEngine for MockSirenEngine {
    fn id(&self) -> &str {
        "mock"
    }

    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        info!("MockSirenEngine: Synthesizing '{}'", text);
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

        if let Ok(voxtral) = crate::chal::drivers::VoxtralDriver::from_env() {
            engines.insert("voxtral".to_string(), Arc::new(voxtral));
            info!("SirenRouter: VoxtralDriver detected in environment and registered.");
        }

        if let Some(whisper) = crate::chal::drivers::WhisperLocalEngine::from_env() {
            engines.insert("whisper-local".to_string(), Arc::new(whisper));
            info!("SirenRouter: WhisperLocalEngine detected and registered.");
        }

        if crate::chal::drivers::EspeakEngine::is_available() {
            let voice = std::env::var("AEGIS_TTS_VOICE").unwrap_or_else(|_| "es".to_string());
            let speed = std::env::var("AEGIS_TTS_SPEED")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(150u32);
            let engine = crate::chal::drivers::EspeakEngine::new(voice, speed, 22050);
            engines.insert("espeak".to_string(), Arc::new(engine));
            info!("SirenRouter: EspeakEngine registered (espeak-ng found in PATH).");
        }

        Self {
            engines: RwLock::new(engines),
            persistence,
        }
    }

    pub async fn register_engine(&self, engine: Arc<dyn SirenEngine>) {
        let mut engines = self.engines.write().await;
        engines.insert(engine.id().to_string(), engine);
    }

    /// Intenta construir un ElevenLabsDriver desde un perfil dado.
    /// Extrae la función para reutilizarla en el fallback a root.
    fn try_elevenlabs_from_profile(
        engine_id: &str,
        settings_json: &str,
        voice_id: &str,
    ) -> Option<Arc<dyn SirenEngine>> {
        if engine_id != "elevenlabs" {
            return None;
        }
        let settings = serde_json::from_str::<serde_json::Value>(settings_json).ok()?;
        let api_key = settings["api_key"].as_str().filter(|k| !k.is_empty())?;
        match crate::chal::drivers::ElevenLabsDriver::new(api_key.to_string(), voice_id.to_string())
        {
            Ok(driver) => Some(Arc::new(driver) as Arc<dyn SirenEngine>),
            Err(e) => {
                warn!("SirenRouter: ElevenLabsDriver creation failed: {}", e);
                None
            }
        }
    }

    /// Resuelve el motor de voz para un tenant.
    ///
    /// Prioridad:
    ///   1. Perfil propio del tenant (si tiene engine_id + api_key válidos)
    ///   2. Perfil de root/admin (fallback global — siempre intentado si el tenant
    ///      no tiene perfil propio o su perfil no tiene credenciales)
    ///   3. Voxtral local (si está registrado)
    ///   4. Espeak local (si está registrado)
    ///   5. Mock (garantía de que el stream no se rompe)
    pub async fn resolve(&self, tenant_id: &str) -> Result<Arc<dyn SirenEngine>> {
        let profile = self
            .persistence
            .get_voice_profile(tenant_id)
            .await
            .ok()
            .flatten();

        let engines = self.engines.read().await;

        // ── 1. Perfil propio del tenant ───────────────────────────────────────
        if let Some(ref p) = profile {
            // Motor registrado (voxtral, espeak, whisper-local, etc.)
            if p.engine_id != "elevenlabs" {
                if let Some(engine) = engines.get(&p.engine_id) {
                    return Ok(engine.clone());
                }
                warn!(
                    "SirenRouter: Engine '{}' not registered for tenant '{}'. Trying admin profile.",
                    p.engine_id, tenant_id
                );
            } else {
                // ElevenLabs: intentar con las credenciales del tenant
                if let Some(driver) =
                    Self::try_elevenlabs_from_profile(&p.engine_id, &p.settings_json, &p.voice_id)
                {
                    info!(
                        "SirenRouter: Using tenant ElevenLabs key for '{}'",
                        tenant_id
                    );
                    return Ok(driver);
                }
                warn!(
                    "SirenRouter: Tenant '{}' has ElevenLabs but no valid api_key. Trying admin profile fallback while preserving tenant voice_id.",
                    tenant_id
                );
                // Fallback: intentar con la key global configurada en el perfil de root/admin pero preservando el voice_id del tenant
                if let Ok(Some(admin_profile)) = self.persistence.get_voice_profile("root").await {
                    if admin_profile.engine_id == "elevenlabs" {
                        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&admin_profile.settings_json) {
                            if let Some(api_key) = settings["api_key"].as_str() {
                                if !api_key.is_empty() {
                                    let chosen_voice_id = if p.voice_id.is_empty() {
                                        admin_profile.voice_id.clone()
                                    } else {
                                        p.voice_id.clone()
                                    };
                                    if let Ok(driver) = crate::chal::drivers::ElevenLabsDriver::new(
                                        api_key.to_string(),
                                        chosen_voice_id,
                                    ) {
                                        info!(
                                            "SirenRouter: Using admin ElevenLabs key for tenant '{}' with voice_id '{}'",
                                            tenant_id,
                                            if p.voice_id.is_empty() { &admin_profile.voice_id } else { &p.voice_id }
                                        );
                                        return Ok(Arc::new(driver));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            info!(
                "SirenRouter: No voice profile for tenant '{}'. Trying admin profile.",
                tenant_id
            );
        }

        // ── 2. Perfil de root/admin (fallback global) ─────────────────────────
        // Se intenta siempre que el tenant no tenga configuración propia válida.
        // Permite que el admin configure una voz global sin que cada tenant
        // tenga que configurarla individualmente.
        if let Ok(Some(admin_profile)) = self.persistence.get_voice_profile("root").await {
            if let Some(driver) = Self::try_elevenlabs_from_profile(
                &admin_profile.engine_id,
                &admin_profile.settings_json,
                &admin_profile.voice_id,
            ) {
                info!(
                    "SirenRouter: Using admin ElevenLabs profile for tenant '{}'",
                    tenant_id
                );
                return Ok(driver);
            }

            // Motor registrado del admin (voxtral, espeak, etc.)
            if admin_profile.engine_id != "elevenlabs" {
                if let Some(engine) = engines.get(&admin_profile.engine_id) {
                    info!(
                        "SirenRouter: Using admin engine '{}' for tenant '{}'",
                        admin_profile.engine_id, tenant_id
                    );
                    return Ok(engine.clone());
                }
            }
        }

        // ── 3. Voxtral local ──────────────────────────────────────────────────
        if let Some(voxtral) = engines.get("voxtral") {
            return Ok(voxtral.clone());
        }

        // ── 4. Espeak local ───────────────────────────────────────────────────
        if let Some(espeak) = engines.get("espeak") {
            return Ok(espeak.clone());
        }

        // ── 5. Mock (última garantía) ─────────────────────────────────────────
        warn!(
            "SirenRouter: All engines failed for tenant '{}'. Using Mock.",
            tenant_id
        );
        engines
            .get("mock")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("SirenRouter: No default 'mock' engine registered"))
    }

    /// Procesa audio crudo para un tenant eligiendo el STT engine configurado.
    pub async fn process_audio(&self, tenant_id: &str, pcm_data: Vec<u8>) -> Result<String> {
        let profile = self
            .persistence
            .get_voice_profile(tenant_id)
            .await
            .ok()
            .flatten();

        let settings = profile
            .as_ref()
            .and_then(|p| serde_json::from_str::<serde_json::Value>(&p.settings_json).ok());

        // ── Speaker verification (si hay fingerprint guardado) ────────────────
        if let Ok(Some((stored_fp, threshold))) =
            self.persistence.get_voice_fingerprint(tenant_id).await
        {
            let (accepted, score) = crate::speaker_id::verify(&pcm_data, &stored_fp, threshold);
            info!(
                "SirenRouter: speaker_verification score={:.3} threshold={:.3} accepted={}",
                score, threshold, accepted
            );
            if !accepted {
                return Err(anyhow::anyhow!(
                    "SPEAKER_MISMATCH: voz no reconocida (score={:.2}, umbral={:.2})",
                    score,
                    threshold
                ));
            }
        }

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
            _ => return Ok(String::new()),
        }

        let engines = self.engines.read().await;
        if let Some(whisper) = engines.get("whisper-local") {
            return whisper.transcribe(pcm_data).await;
        }

        Ok(String::new())
    }
}
