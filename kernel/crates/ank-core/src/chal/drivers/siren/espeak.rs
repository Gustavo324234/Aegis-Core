use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::info;

use crate::router::siren::SirenEngine;

/// EspeakEngine: TTS local usando espeak-ng CLI.
/// STT no soportado — este engine es solo TTS.
/// Requiere: apt install espeak-ng
pub struct EspeakEngine {
    voice: String,
    speed: u32,
    sample_rate: u32,
}

impl EspeakEngine {
    pub fn new(voice: String, speed: u32, sample_rate: u32) -> Self {
        Self {
            voice,
            speed,
            sample_rate,
        }
    }

    pub fn default_es() -> Self {
        Self {
            voice: "es".to_string(),
            speed: 150,
            sample_rate: 22050,
        }
    }

    pub fn default_en() -> Self {
        Self {
            voice: "en".to_string(),
            speed: 150,
            sample_rate: 22050,
        }
    }

    /// Retorna true si espeak-ng está instalado en el sistema.
    pub fn is_available() -> bool {
        std::process::Command::new("which")
            .arg("espeak-ng")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl SirenEngine for EspeakEngine {
    fn id(&self) -> &str {
        "espeak"
    }

    /// Sintetiza texto a PCM 16-bit little-endian mono a 22050 Hz usando espeak-ng CLI.
    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let clean_text = clean_for_tts(&text);

        // --stdout: output PCM raw a stdout
        // -b 1: encoding 16-bit little-endian
        // espeak-ng nativo opera a 22050 Hz; no hay flag de sample-rate en CLI.
        let output = tokio::process::Command::new("espeak-ng")
            .args([
                "-v",
                &self.voice,
                "-s",
                &self.speed.to_string(),
                "--stdout",
                "-b",
                "1",
                &clean_text,
            ])
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow!("espeak-ng no encontrado. Instalá con: apt install espeak-ng")
                } else {
                    anyhow!("Error ejecutando espeak-ng: {}", e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("espeak-ng falló: {}", stderr.trim()));
        }

        // stdout contiene PCM raw 16-bit LE mono; algunas versiones incluyen header WAV.
        let pcm = strip_wav_header_if_present(output.stdout);

        info!(
            bytes = pcm.len(),
            voice = %self.voice,
            sample_rate = self.sample_rate,
            "EspeakEngine: síntesis completada."
        );

        Ok(pcm)
    }

    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String> {
        Err(anyhow!(
            "EspeakEngine: STT no soportado. Usá WhisperLocalEngine o GroqSttEngine."
        ))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Limpia markdown del texto para que espeak-ng lo lea naturalmente.
fn clean_for_tts(text: &str) -> String {
    let mut result = text.to_string();

    // Remover bloques de código completos
    while let Some(start) = result.find("```") {
        if let Some(end) = result[start + 3..].find("```") {
            result.replace_range(start..start + 3 + end + 3, "");
        } else {
            break;
        }
    }

    result = result
        .replace("**", "")
        .replace('*', "")
        .replace("__", "")
        .replace('`', "");

    // Convertir headers markdown a frases terminadas en punto
    let cleaned: Vec<String> = result
        .lines()
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .collect();
    result = cleaned.join(". ");

    // Eliminar URLs
    result = result
        .split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://"))
        .collect::<Vec<_>>()
        .join(" ");

    result.trim().to_string()
}

/// Si el output comienza con "RIFF" (header WAV de 44 bytes), lo saltea.
fn strip_wav_header_if_present(data: Vec<u8>) -> Vec<u8> {
    if data.len() > 44 && data.starts_with(b"RIFF") {
        data[44..].to_vec()
    } else {
        data
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_for_tts_strips_markdown() {
        let input = "**Hola** `mundo`\n## Título";
        let result = clean_for_tts(input);
        assert!(!result.contains("**"), "should strip bold markers");
        assert!(!result.contains('`'), "should strip inline code");
        assert!(!result.contains("##"), "should strip heading markers");
        assert!(result.contains("Hola"), "should keep text content");
        assert!(result.contains("Título"), "should keep heading text");
    }

    #[test]
    fn test_clean_for_tts_strips_urls() {
        let input = "Visita https://example.com para más info";
        let result = clean_for_tts(input);
        assert!(!result.contains("https://"));
    }

    #[test]
    fn test_strip_wav_header_riff() {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0u8; 40]);
        data.extend_from_slice(&[1u8, 2u8, 3u8]);
        let stripped = strip_wav_header_if_present(data);
        assert_eq!(stripped, vec![1u8, 2u8, 3u8]);
    }

    #[test]
    fn test_strip_wav_header_no_riff() {
        let data = vec![0u8, 1u8, 2u8, 3u8];
        let out = strip_wav_header_if_present(data.clone());
        assert_eq!(out, data);
    }

    #[tokio::test]
    #[ignore = "requiere espeak-ng instalado en el sistema"]
    async fn test_espeak_synthesize() {
        let engine = EspeakEngine::default_es();
        let pcm = engine.synthesize("Hola mundo".to_string()).await.unwrap();
        assert!(!pcm.is_empty(), "El PCM no debería estar vacío");
    }

    #[tokio::test]
    async fn test_espeak_synthesize_empty_text() {
        let engine = EspeakEngine::default_es();
        let pcm = engine.synthesize("".to_string()).await.unwrap();
        assert!(pcm.is_empty(), "Texto vacío debe retornar Vec vacío");
    }

    #[tokio::test]
    async fn test_espeak_transcribe_returns_error() {
        let engine = EspeakEngine::default_es();
        let result = engine.transcribe(vec![]).await;
        assert!(result.is_err(), "STT debe retornar error en EspeakEngine");
    }
}
