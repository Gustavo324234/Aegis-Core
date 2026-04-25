use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{info, warn};

use crate::router::siren::SirenEngine;

/// WhisperLocalEngine: Transcripción STT local usando whisper.cpp CLI.
/// TTS no soportado — este engine es solo STT.
pub struct WhisperLocalEngine {
    model_path: PathBuf,
}

impl WhisperLocalEngine {
    /// Busca el modelo activo en AEGIS_DATA_DIR y verifica que existe.
    /// Retorna None si no hay modelo descargado todavía.
    pub fn from_env() -> Option<Self> {
        let data_dir = std::env::var("AEGIS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));

        let models_dir = data_dir.join("models");
        let active_path = models_dir.join("active_model.txt");

        let model_id = std::fs::read_to_string(&active_path)
            .ok()
            .map(|s| s.trim().to_string())
            // backwards-compat: si existe ggml-base.bin sin active_model.txt
            .or_else(|| {
                let legacy = models_dir.join("ggml-base.bin");
                if legacy.exists() {
                    Some("base".to_string())
                } else {
                    None
                }
            })?;

        let model_path = models_dir.join(format!("ggml-{}.bin", model_id));
        if !model_path.exists() {
            warn!(
                "WhisperLocalEngine: model file not found at {:?}",
                model_path
            );
            return None;
        }

        info!(
            model = %model_id,
            path = ?model_path,
            "WhisperLocalEngine: model detected."
        );
        Some(Self { model_path })
    }
}

#[async_trait]
impl SirenEngine for WhisperLocalEngine {
    fn id(&self) -> &str {
        "whisper-local"
    }

    /// Whisper es solo STT — no sintetiza voz.
    async fn synthesize(&self, _text: String) -> Result<Vec<u8>> {
        Err(anyhow!(
            "WhisperLocalEngine: TTS no soportado. Usá ElevenLabs o Voxtral para síntesis."
        ))
    }

    /// Transcribe PCM 16kHz 16-bit mono a texto usando whisper-cli.
    async fn transcribe(&self, audio: Vec<u8>) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        // 1. Envolver PCM en WAV y escribir a archivo temporal
        let wav_data = pcm_to_wav(&audio, 16000);
        let tmp_id = uuid::Uuid::new_v4().to_string();
        let tmp_wav = std::env::temp_dir().join(format!("aegis_stt_{}.wav", tmp_id));

        tokio::fs::write(&tmp_wav, &wav_data)
            .await
            .map_err(|e| anyhow!("No se pudo escribir WAV temporal: {}", e))?;

        // 2. Llamar whisper-cli
        let result = run_whisper_cli(&self.model_path, &tmp_wav).await;

        // 3. Limpiar temp
        tokio::fs::remove_file(&tmp_wav).await.ok();

        match result {
            Ok(text) => {
                let trimmed = text.trim().to_string();
                info!(
                    chars = trimmed.len(),
                    "WhisperLocalEngine: Transcripción completada."
                );
                Ok(trimmed)
            }
            Err(e) => {
                warn!("WhisperLocalEngine: transcripción fallida: {}", e);
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Envuelve bytes PCM 16-bit mono en un header WAV estándar.
fn pcm_to_wav(pcm: &[u8], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_len = pcm.len() as u32;

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36u32 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

/// Ejecuta whisper-cli como subprocess. Prueba "whisper-cli" y luego "whisper" como fallback.
async fn run_whisper_cli(
    model_path: &std::path::Path,
    wav_path: &std::path::Path,
) -> Result<String> {
    let args = [
        "-m",
        model_path.to_str().unwrap_or(""),
        "-f",
        wav_path.to_str().unwrap_or(""),
        "-nt", // sin timestamps
        "-l",
        "auto", // auto-detectar idioma
        "-np",  // sin barra de progreso
    ];

    // Intentar whisper-cli primero (versiones modernas de whisper.cpp)
    for cmd in &["whisper-cli", "whisper"] {
        let result = tokio::process::Command::new(cmd).args(args).output().await;

        match result {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                return Ok(text);
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(anyhow!(
                    "whisper-cli falló (exit {}): {}",
                    out.status,
                    stderr.trim()
                ));
            }
            // Binario no encontrado — probar el siguiente
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(anyhow!("Error al ejecutar whisper-cli: {}", e)),
        }
    }

    Err(anyhow!(
        "whisper-cli no encontrado. Instalá whisper.cpp: https://github.com/ggerganov/whisper.cpp"
    ))
}
