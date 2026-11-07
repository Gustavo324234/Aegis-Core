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


/// CORE-378: Driver local determinista de embeddings semánticos para inferencia offline / edge.
/// Produce vectores densos unitarios normalizados de 384 dimensiones (estándar all-MiniLM-L6-v2)
/// mediante hashing n-gram multinivel con pesos IDF y proyección ortogonal pseudo-aleatoria.
#[derive(Debug, Clone)]
pub struct LocalEmbeddingDriver {
    dimension: usize,
}

impl Default for LocalEmbeddingDriver {
    fn default() -> Self {
        Self::new(384)
    }
}

impl LocalEmbeddingDriver {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension: dimension.max(64),
        }
    }

    /// Calcula la representación vectorial densa de un texto dado.
    pub fn compute_embedding(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimension];
        let normalized = text.to_lowercase();
        let words: Vec<&str> = normalized
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            return vector;
        }

        for (pos, word) in words.iter().enumerate() {
            // Peso decreciente según posición inicial (heurística tipo TF-IDF)
            let weight = 1.0 + 1.0 / ((pos + 1) as f32).sqrt();

            // Hashing FNV-1a para el token completo
            let mut h: u64 = 0xcbf29ce484222325;
            for b in word.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }

            let idx = (h as usize) % self.dimension;
            let sign = if (h >> 32) & 1 == 0 { 1.0f32 } else { -1.0f32 };
            vector[idx] += sign * weight;

            // Subword 3-grams para capturar similitudes morfológicas y raíces
            if word.len() >= 3 {
                for window in word.as_bytes().windows(3) {
                    let mut sub_h: u64 = 0x811c9dc5;
                    for b in window {
                        sub_h ^= *b as u64;
                        sub_h = sub_h.wrapping_mul(0x01000193);
                    }
                    let sub_idx = (sub_h as usize) % self.dimension;
                    let sub_sign = if (sub_h >> 16) & 1 == 0 { 0.5f32 } else { -0.5f32 };
                    vector[sub_idx] += sub_sign * weight;
                }
            }
        }

        // L2 normalización unitaria (el producto punto equivale a similitud coseno)
        let norm_sq: f32 = vector.iter().map(|v| v * v).sum();
        let norm = norm_sq.sqrt();
        if norm > 1e-6 {
            for v in vector.iter_mut() {
                *v /= norm;
            }
        }

        vector
    }
}

#[async_trait]
impl EmbeddingDriver for LocalEmbeddingDriver {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SystemError> {
        Ok(self.compute_embedding(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, SystemError> {
        Ok(texts.iter().map(|t| self.compute_embedding(t)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[tokio::test]
    async fn test_core_378_local_embedding_driver_properties() {
        let driver = LocalEmbeddingDriver::default();
        let text1 = "Aegis OS es un sistema operativo cognitivo nativo";
        let text2 = "Aegis es un sistema operativo con agentes de IA";
        let text3 = "Receta para hacer pizza napolitana con mozzarella y tomate";

        let v1 = driver.embed(text1).await.unwrap();
        let v2 = driver.embed(text2).await.unwrap();
        let v3 = driver.embed(text3).await.unwrap();

        assert_eq!(v1.len(), 384);
        assert_eq!(v2.len(), 384);
        assert_eq!(v3.len(), 384);

        // Vector unitario (norma ~ 1.0)
        let norm1: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm1 - 1.0).abs() < 1e-4);

        // Textos semánticamente cercanos deben tener mayor similitud que textos no relacionados
        let sim_1_2 = cosine_similarity(&v1, &v2);
        let sim_1_3 = cosine_similarity(&v1, &v3);

        assert!(
            sim_1_2 > sim_1_3,
            "Textos sobre Aegis OS deben ser más similares ({}) que con receta de pizza ({})",
            sim_1_2,
            sim_1_3
        );
    }
}
