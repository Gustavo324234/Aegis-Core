use crate::chal::{EmbeddingDriver, SystemError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub struct CloudEmbeddingDriver {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    client: Arc<Client>,
}

impl CloudEmbeddingDriver {
    pub fn new(client: Arc<Client>, api_url: String, api_key: String, model_id: String) -> Self {
        Self {
            client,
            api_url,
            api_key,
            model_id,
        }
    }
}

#[derive(Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingDriver for CloudEmbeddingDriver {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SystemError> {
        let res = self.embed_batch(&[text.to_string()]).await?;
        res.into_iter().next().ok_or_else(|| {
            SystemError::HardwareFailure("Cloud API returned empty embedding list".to_string())
        })
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, SystemError> {
        let request_body = EmbeddingRequest {
            input: texts.to_vec(),
            model: self.model_id.clone(),
        };

        // Use the same base URL as completions but target /embeddings
        // Most providers (OpenAI, Anthropic via Proxy, etc.) follow this pattern.
        // If api_url already points to /completions, we try to fix it.
        let url = if self.api_url.ends_with("/chat/completions") {
            self.api_url.replace("/chat/completions", "/embeddings")
        } else if self.api_url.ends_with("/completions") {
            self.api_url.replace("/completions", "/embeddings")
        } else if !self.api_url.contains("/embeddings") {
            format!("{}/embeddings", self.api_url.trim_end_matches('/'))
        } else {
            self.api_url.clone()
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                SystemError::HardwareFailure(format!("Embedding request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(SystemError::HardwareFailure(format!(
                "Cloud Embedding API Error {}: {}",
                status, text
            )));
        }

        let parsed: EmbeddingResponse = response.json().await.map_err(|e| {
            SystemError::HardwareFailure(format!("Failed to parse embedding response: {}", e))
        })?;

        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}
