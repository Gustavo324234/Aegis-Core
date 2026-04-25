use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use tokio::time::Duration;

use crate::router::siren::SirenEngine;

pub struct ElevenLabsDriver {
    api_key: String,
    voice_id: String,
    client: Client,
}

impl ElevenLabsDriver {
    pub fn new(api_key: String, voice_id: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to build reqwest client: {}", e))?;
        let resolved_voice = if voice_id.is_empty() {
            "21m00Tcm4TlvDq8ikWAM".to_string() // Rachel — default multilingual
        } else {
            voice_id
        };
        Ok(Self {
            api_key,
            voice_id: resolved_voice,
            client,
        })
    }
}

#[async_trait]
impl SirenEngine for ElevenLabsDriver {
    fn id(&self) -> &str {
        "elevenlabs"
    }

    /// Requests PCM 22050 Hz 16-bit mono directly so the TTSPlayer can decode it natively.
    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        let url = format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=pcm_22050",
            self.voice_id
        );

        let body = serde_json::json!({
            "text": text,
            "model_id": "eleven_multilingual_v2",
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75
            }
        });

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .header("Accept", "audio/pcm")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("ElevenLabs network error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("ElevenLabs API error {}: {}", status, body));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read ElevenLabs response: {}", e))?;

        Ok(bytes.to_vec())
    }
}
