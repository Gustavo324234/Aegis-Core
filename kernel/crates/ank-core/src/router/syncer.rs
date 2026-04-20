use crate::router::catalog::{ModelCatalog, ModelEntry, TaskScores};
use crate::router::key_pool::KeyPool;
use anyhow::Context;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{info, warn};

pub struct CatalogSyncer {
    catalog: Arc<ModelCatalog>,
    key_pool: Arc<KeyPool>,
    client: Client,
}

#[derive(Deserialize)]
struct OpenRouterModelList {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
    name: String,
    context_length: Option<u32>,
    pricing: Option<OpenRouterPricing>,
    supported_parameters: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OpenRouterPricing {
    prompt: Option<serde_json::Value>,
    completion: Option<serde_json::Value>,
}

impl CatalogSyncer {
    pub fn new(catalog: Arc<ModelCatalog>, key_pool: Arc<KeyPool>) -> Self {
        Self {
            catalog,
            key_pool,
            client: Client::new(),
        }
    }

    pub fn start_background_sync(self: Arc<Self>) {
        tokio::spawn(async move {
            // Initial sync at startup
            if let Err(e) = self.sync_once().await {
                warn!(
                    "CatalogSyncer: Initial sync failed (using bundled catalog): {}",
                    e
                );
            } else {
                info!("CatalogSyncer: Initial sync completed.");
            }
            // Then every 24h
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(24 * 60 * 60)).await;
                if let Err(e) = self.sync_once().await {
                    warn!("CatalogSyncer: Periodic sync failed: {}", e);
                } else {
                    info!("CatalogSyncer: Periodic sync completed.");
                }
            }
        });
    }

    pub async fn sync_now(&self) -> anyhow::Result<()> {
        self.sync_once().await
    }

    async fn sync_once(&self) -> anyhow::Result<()> {
        // Only sync if we have an OpenRouter key
        if !self.key_pool.has_openrouter_key().await {
            return Ok(());
        }

        let response = self
            .client
            .get("https://openrouter.ai/api/v1/models")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("Failed to fetch OpenRouter model list")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("OpenRouter API returned non-success status: {}", status);
        }

        let model_list: OpenRouterModelList = response
            .json()
            .await
            .context("Failed to parse OpenRouter model list JSON")?;

        let mut current = self.catalog.all_entries().await;

        for remote in model_list.data {
            let cost_input = remote
                .pricing
                .as_ref()
                .and_then(|p| p.prompt.as_ref())
                .and_then(|v| match v {
                    serde_json::Value::Number(n) => n.as_f64(),
                    serde_json::Value::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                })
                .unwrap_or(0.0)
                * 1_000_000.0;

            let cost_output = remote
                .pricing
                .as_ref()
                .and_then(|p| p.completion.as_ref())
                .and_then(|v| match v {
                    serde_json::Value::Number(n) => n.as_f64(),
                    serde_json::Value::String(s) => s.parse::<f64>().ok(),
                    _ => None,
                })
                .unwrap_or(0.0)
                * 1_000_000.0;

            let supports_tools = remote
                .supported_parameters
                .as_ref()
                .map(|p| p.iter().any(|s| s == "tools"))
                .unwrap_or(false);

            let supports_json = remote
                .supported_parameters
                .as_ref()
                .map(|p| p.iter().any(|s| s == "json_mode"))
                .unwrap_or(false);

            let context_window = remote.context_length.unwrap_or(4096);

            if let Some(existing) = current.iter_mut().find(|e| e.model_id == remote.id) {
                // Skip local models — never overwrite from remote
                if existing.is_local {
                    continue;
                }
                // Update pricing and capabilities, preserve task_scores
                existing.cost_input_per_mtok = cost_input;
                existing.cost_output_per_mtok = cost_output;
                existing.context_window = context_window;
                existing.supports_tools = supports_tools;
                existing.supports_json_mode = supports_json;
                existing.display_name = remote.name;
            } else {
                // New model not in local catalog — infer scores by heuristic
                let scores = infer_task_scores(&remote.id);
                let provider = remote.id.split('/').next().unwrap_or("unknown").to_string();
                current.push(ModelEntry {
                    model_id: remote.id,
                    provider,
                    display_name: remote.name,
                    context_window,
                    cost_input_per_mtok: cost_input,
                    cost_output_per_mtok: cost_output,
                    supports_tools,
                    supports_json_mode: supports_json,
                    task_scores: scores,
                    is_local: false,
                    avg_latency_ms: None,
                });
            }
        }

        self.catalog.replace_all(current).await;
        Ok(())
    }
}

fn infer_task_scores(model_id: &str) -> TaskScores {
    let id_lower = model_id.to_lowercase();
    let is_strong = id_lower.contains("claude") || id_lower.contains("gpt-4");
    if is_strong {
        TaskScores {
            chat: 5,
            coding: 5,
            planning: 5,
            analysis: 5,
            summarization: 4,
            extraction: 5,
        }
    } else {
        TaskScores {
            chat: 3,
            coding: 3,
            planning: 3,
            analysis: 3,
            summarization: 3,
            extraction: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use crate::scheduler::persistence::{StatePersistor, VoiceProfile};

    struct NoopPersistor;

    #[async_trait::async_trait]
    impl StatePersistor for NoopPersistor {
        async fn save_pcb(&self, _: &PCB) -> anyhow::Result<()> {
            Ok(())
        }
        async fn delete_pcb(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn load_all_pcbs(&self) -> anyhow::Result<Vec<PCB>> {
            Ok(vec![])
        }
        async fn flush(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_voice_profile(
            &self,
            _tenant_id: &str,
        ) -> anyhow::Result<Option<VoiceProfile>> {
            Ok(None)
        }
        async fn update_voice_profile(&self, _profile: VoiceProfile) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_no_sync_without_openrouter_key() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let syncer = CatalogSyncer::new(catalog.clone(), key_pool);

        let before = catalog.all_entries().await.len();
        // sync_once should be a no-op when no openrouter key
        syncer.sync_once().await?;
        let after = catalog.all_entries().await.len();

        assert_eq!(
            before, after,
            "Catalog should not change without OpenRouter key"
        );
        Ok(())
    }

    #[test]
    fn test_infer_task_scores_strong() {
        let scores = infer_task_scores("anthropic/claude-sonnet-4-6");
        assert_eq!(scores.coding, 5);
        assert_eq!(scores.analysis, 5);
    }

    #[test]
    fn test_infer_task_scores_small() {
        let scores = infer_task_scores("meta-llama/llama-3.1-8b-instruct");
        assert_eq!(scores.coding, 3);
    }
}
