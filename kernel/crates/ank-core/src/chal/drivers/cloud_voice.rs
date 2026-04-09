use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::{multipart, Client};
use tokio::time::Duration;

use crate::router::siren::SirenEngine;

#[async_trait]
pub trait VoiceDriver: Send + Sync {
    async fn transcribe(&self, audio_data: Vec<u8>) -> Result<String>;
    async fn synthesize(&self, text: String) -> Result<Vec<u8>>;
}

#[async_trait]
impl SirenEngine for CloudVoiceDriver {
    fn id(&self) -> &str {
        "cloud"
    }

    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        VoiceDriver::synthesize(self, text).await
    }
}

pub struct CloudVoiceDriver {
    api_url: String,
    api_key: String,
    client: Client,
}

impl CloudVoiceDriver {
    pub fn new(api_url: String, api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to build reqwest client: {}", e))?;
        Ok(Self {
            api_url,
            api_key,
            client,
        })
    }
}

#[async_trait]
impl VoiceDriver for CloudVoiceDriver {
    async fn transcribe(&self, audio_data: Vec<u8>) -> Result<String> {
        let file_part = multipart::Part::bytes(audio_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| anyhow!("Failed to create multipart file: {}", e))?;

        let form = multipart::Form::new()
            .part("file", file_part)
            .text("model", "whisper-1")
            .text("response_format", "text");

        let url = format!("{}/v1/audio/transcriptions", self.api_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Transcription API error {}: {}", status, body));
        }

        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read text: {}", e))?;
        Ok(text)
    }

    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        let url = format!("{}/v1/audio/speech", self.api_url);
        let body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": "alloy",
            "response_format": "mp3"
        });

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Synthesis API error {}: {}", status, body));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read bytes: {}", e))?;
        Ok(bytes.to_vec())
    }
}
