use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

use crate::router::siren::SirenEngine;

const WHISPER_CLI_TIMEOUT: Duration = Duration::from_secs(60);

/// Valida que un identificador de modelo Whisper sea un slug seguro sin path traversal.
pub fn is_safe_whisper_model_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

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

        if !is_safe_whisper_model_id(&model_id) {
            warn!(
                model = %model_id,
                "WhisperLocalEngine: invalid or unsafe model_id in active_model.txt; ignoring."
            );
            return None;
        }

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
    let data_len = u32::try_from(pcm.len()).unwrap_or(u32::MAX - 36);

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&36u32.saturating_add(data_len).to_le_bytes());
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

/// Ejecuta whisper-cli como subprocess con timeout estricto y kill_on_drop.
/// Prueba "whisper-cli" y luego "whisper" como fallback.
async fn run_whisper_cli(
    model_path: &std::path::Path,
    wav_path: &std::path::Path,
) -> Result<String> {
    let model_str = model_path
        .to_str()
        .ok_or_else(|| anyhow!("Ruta de modelo Whisper inválida (no UTF-8)"))?;
    let wav_str = wav_path
        .to_str()
        .ok_or_else(|| anyhow!("Ruta de archivo WAV temporal inválida (no UTF-8)"))?;

    let args = [
        "-m", model_str, "-f", wav_str, "-nt", // sin timestamps
        "-l", "auto", // auto-detectar idioma
        "-np",  // sin barra de progreso
    ];

    // Intentar whisper-cli primero (versiones modernas de whisper.cpp)
    for bin in &["whisper-cli", "whisper"] {
        let mut command = tokio::process::Command::new(bin);
        command.args(args).kill_on_drop(true);

        let timed = tokio::time::timeout(WHISPER_CLI_TIMEOUT, command.output()).await;
        match timed {
            Err(_) => {
                return Err(anyhow!(
                    "{} excedió el timeout de {}s",
                    bin,
                    WHISPER_CLI_TIMEOUT.as_secs()
                ));
            }
            Ok(Ok(out)) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                return Ok(text);
            }
            Ok(Ok(out)) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(anyhow!(
                    "{} falló (exit {}): {}",
                    bin,
                    out.status,
                    stderr.trim()
                ));
            }
            // Binario no encontrado — probar el siguiente
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(Err(e)) => return Err(anyhow!("Error al ejecutar {}: {}", bin, e)),
        }
    }

    Err(anyhow!(
        "whisper-cli no encontrado. Instalá whisper.cpp: https://github.com/ggerganov/whisper.cpp"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_whisper_model_id() {
        assert!(is_safe_whisper_model_id("base"));
        assert!(is_safe_whisper_model_id("large-v3"));
        assert!(is_safe_whisper_model_id("small.en_q5"));
        assert!(!is_safe_whisper_model_id(""));
        assert!(!is_safe_whisper_model_id("../etc/passwd"));
        assert!(!is_safe_whisper_model_id("..\\windows\\win"));
        assert!(!is_safe_whisper_model_id("C:\\model"));
        assert!(!is_safe_whisper_model_id("sub/model"));
    }

    #[test]
    fn test_pcm_to_wav_header_integrity() {
        let pcm = vec![0u8; 320];
        let wav = pcm_to_wav(&pcm, 16000);
        assert_eq!(wav.len(), 44 + 320);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
    }
}
