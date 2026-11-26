use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::{multipart, Client};
use tokio::time::Duration;

use crate::router::siren::SirenEngine;

/// GroqSttEngine: Transcripción STT gratuita via Groq API (Whisper large-v3-turbo).
/// Sin modelos locales, sin recursos, solo requiere una API key gratuita de Groq.
/// TTS no soportado — engine solo STT.
pub struct GroqSttEngine {
    api_key: String,
    client: Client,
}

impl GroqSttEngine {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| anyhow!("Failed to build reqwest client: {}", e))?;
        Ok(Self { api_key, client })
    }

    /// Transcribes pre-encoded audio bytes (WAV, M4A, WebM, OGG) using Groq Whisper Turbo.
    pub async fn transcribe_encoded(
        &self,
        audio_bytes: Vec<u8>,
        filename: &str,
        mime: &str,
        lang_hint: Option<&str>,
    ) -> Result<String> {
        if audio_bytes.is_empty() {
            return Ok(String::new());
        }

        let file_part = multipart::Part::bytes(audio_bytes)
            .file_name(filename.to_string())
            .mime_str(mime)
            .map_err(|e| anyhow!("MIME error: {}", e))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", "whisper-large-v3-turbo")
            .text("response_format", "text");

        if let Some(lang) = lang_hint.filter(|l| !l.trim().is_empty()) {
            let short = lang.split(['-', '_']).next().unwrap_or(lang).trim();
            if !short.is_empty() {
                form = form.text("language", short.to_string());
            }
        }

        let response = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| anyhow!("Groq network error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Groq STT error {}: {}", status, body));
        }

        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read Groq response: {}", e))?;

        Ok(text.trim().to_string())
    }

    /// Transcribe PCM 16kHz 16-bit mono a texto usando Groq Whisper API con hint opcional de idioma.
    pub async fn transcribe_with_lang(
        &self,
        audio: Vec<u8>,
        lang_hint: Option<&str>,
    ) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }
        let wav_data = pcm_to_wav(&audio, 16000);
        self.transcribe_encoded(wav_data, "audio.wav", "audio/wav", lang_hint)
            .await
    }
}

#[async_trait]
impl SirenEngine for GroqSttEngine {
    fn id(&self) -> &str {
        "groq"
    }

    async fn synthesize(&self, _text: String) -> Result<Vec<u8>> {
        Err(anyhow!(
            "GroqSttEngine: TTS no soportado. Usá ElevenLabs o Voxtral para síntesis."
        ))
    }

    /// Transcribe PCM 16kHz 16-bit mono a texto usando Groq Whisper API.
    async fn transcribe(&self, audio: Vec<u8>) -> Result<String> {
        self.transcribe_with_lang(audio, None).await
    }
}

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
