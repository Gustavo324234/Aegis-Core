use crate::router::catalog::{ModelCatalog, ModelEntry, TaskScores, ToolUseSupport};
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
        // Sync non-OpenRouter provider models (ollama, custom, etc.) from global key pool.
        // This ensures their models survive restarts and appear in the catalog immediately.
        // Unredacted because the Ollama prune path below probes the live API with
        // real credentials.
        let global_keys = self.key_pool.list_global_keys_unredacted().await;
        for key in &global_keys {
            if key.provider == "openrouter" {
                continue;
            }

            // Ollama's /models listing advertises subscription-gated models the
            // key can't actually call (free tier → 403 on the big models). For
            // ollama we re-discover with a live callability probe so the catalog
            // only gets models that return 2xx; for every other provider the
            // stored active_models list is authoritative. On discovery failure we
            // fall back to the stored list so a network blip never empties the
            // catalog.
            let models_to_register: Option<Vec<String>> = if key.provider == "ollama"
                || key.provider == "ollama_cloud"
            {
                match crate::router::discovery::fetch_provider_models(
                    &key.provider,
                    key.api_url.as_deref(),
                    &key.api_key,
                )
                .await
                {
                    Ok(discovered) if !discovered.is_empty() => {
                        Some(discovered.into_iter().map(|d| d.model_id).collect())
                    }
                    Ok(_) => key.active_models.clone(),
                    Err(e) => {
                        warn!(
                            provider = %key.provider,
                            error = %e,
                            "CatalogSyncer: ollama discovery probe failed — using stored active_models"
                        );
                        key.active_models.clone()
                    }
                }
            } else {
                key.active_models.clone()
            };

            if let Some(models) = &models_to_register {
                if !models.is_empty() {
                    let n = register_provider_models(&key.provider, models, &self.catalog).await;
                    if n > 0 {
                        info!(
                            "CatalogSyncer: registered {} models from {} key pool entry",
                            n, key.provider
                        );
                    }
                }
            }
        }

        // Only sync OpenRouter catalog if we have an OpenRouter key
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
                    free_tier_rpm: None,
                    free_tier_rpd: None,
                    free_tier_eligible: true,
                    tool_use_support: crate::router::catalog::ToolUseSupport::Unknown,
                });
            }
        }

        self.catalog.replace_all(current).await;
        Ok(())
    }
}

/// Register models declared in `active_models` of a non-OpenRouter provider key
/// (e.g. ollama, custom, lmstudio) into the catalog so CognitiveRouter can route
/// to them. Called both on key registration and during startup sync so that
/// Ollama/custom model entries survive server restarts.
/// Returns the number of new entries added.
pub async fn register_provider_models(
    provider: &str,
    models: &[String],
    catalog: &Arc<ModelCatalog>,
) -> usize {
    // CORE-FIX: canonicalise the provider id so the entries we add to the
    // catalog match what `decide()` / `entry_api_url()` / `ProviderKind`
    // expect downstream. Without this, a key registered as "google" would
    // produce ModelEntry rows the router can't route to (it looks up
    // "gemini").
    let provider = crate::router::normalize_provider_id(provider);
    let mut added = 0usize;
    for raw in models {
        let model_id = raw.trim();
        if model_id.is_empty() {
            continue;
        }
        if catalog.find(model_id).await.is_some() {
            continue;
        }
        let is_large = ["70b", "671b", "405b", "72b", "32b", "34b"]
            .iter()
            .any(|tag| model_id.contains(tag));
        let scores = if is_large {
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
        };
        catalog
            .add_entry(ModelEntry {
                model_id: model_id.to_string(),
                provider: provider.to_string(),
                display_name: format!("{} ({})", model_id, provider),
                context_window: 32_768,
                cost_input_per_mtok: 0.0,
                cost_output_per_mtok: 0.0,
                supports_tools: true,
                supports_json_mode: true,
                tool_use_support: ToolUseSupport::Unknown,
                is_local: false,
                avg_latency_ms: Some(3000),
                free_tier_rpm: None,
                free_tier_rpd: None,
                free_tier_eligible: true,
                task_scores: scores,
            })
            .await;
        info!(
            "Catalog: registered {} model '{}' from key pool",
            provider, model_id
        );
        added += 1;
    }
    added
}

/// Fetches the OpenRouter model list with the given key, filters to free-tier
/// models (pricing.prompt == "0"), and adds any that are not yet in the catalog.
/// Returns the number of new entries added.
pub async fn sync_openrouter_free_models(
    api_key: &str,
    catalog: &Arc<ModelCatalog>,
) -> anyhow::Result<usize> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://openrouter.ai/api/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let models = resp["data"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("no data array in OpenRouter response"))?;

    let mut added = 0usize;
    for model in models {
        let prompt_price = model["pricing"]["prompt"].as_str().unwrap_or("1");
        if prompt_price != "0" {
            continue;
        }

        let model_id = match model["id"].as_str() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => continue,
        };

        if catalog.find(&model_id).await.is_some() {
            continue;
        }

        let context = model["context_length"].as_u64().unwrap_or(131_072) as u32;
        let name = model["name"].as_str().unwrap_or(&model_id).to_string();

        let entry = ModelEntry {
            model_id: model_id.clone(),
            provider: "openrouter".to_string(),
            display_name: format!("{} (free)", name),
            context_window: context,
            cost_input_per_mtok: 0.0,
            cost_output_per_mtok: 0.0,
            supports_tools: false,
            supports_json_mode: false,
            tool_use_support: ToolUseSupport::Unknown,
            is_local: false,
            avg_latency_ms: Some(2500),
            free_tier_rpm: Some(20),
            free_tier_rpd: Some(200),
            free_tier_eligible: true,
            task_scores: TaskScores {
                chat: 3,
                coding: 3,
                planning: 3,
                analysis: 3,
                summarization: 3,
                extraction: 3,
            },
        };

        catalog.add_entry(entry).await;
        added += 1;
    }

    Ok(added)
}

fn infer_task_scores(model_id: &str) -> TaskScores {
    let id_lower = model_id.to_lowercase();
    // CORE-FIX: recognise the current generation of frontier models. The old
    // heuristic only matched "claude" and "gpt-4", so anything coming through
    // discovery — Gemini 2.5/3.x, DeepSeek R, Llama 4, Qwen3, … — fell to
    // score 3 across the board and lost the CMR ranking to bundled defaults.
    // That was the root cause of "I added a Gemini key but gemini-2.5-pro
    // never gets picked".
    //
    // The check is fuzzy on purpose so the next obvious version (gemini-3,
    // claude-5, gpt-5, …) keeps scoring strong without another patch.
    let is_strong = id_lower.contains("claude")           // Claude 3+, 4+, etc.
        || id_lower.contains("gpt-4")                     // GPT-4, 4o, 4.1
        || id_lower.contains("gpt-5")                     // future GPT-5
        || id_lower.contains("/o3")                       // OpenAI o3 (reasoning)
        || id_lower.starts_with("o3")
        || id_lower.contains("/o4")
        || id_lower.starts_with("o4")
        || id_lower.contains("gemini-2.5")
        || id_lower.contains("gemini-3")                  // 3.x preview families
        || id_lower.contains("grok-3")
        || id_lower.contains("grok-4")
        || id_lower.contains("deepseek-r")                // R1, R2 reasoning
        || id_lower.contains("deepseek-v3")
        || id_lower.contains("qwen3")
        || id_lower.contains("qwen-2.5")
        || id_lower.contains("llama-4")
        || id_lower.contains("llama-3.3");
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
    use crate::router::catalog::ModelProfile;
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
        async fn save_voice_fingerprint(
            &self,
            _tenant_id: &str,
            _fingerprint: &[f32],
            _threshold: f32,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_voice_fingerprint(
            &self,
            _tenant_id: &str,
        ) -> anyhow::Result<Option<(Vec<f32>, f32)>> {
            Ok(None)
        }
        async fn delete_voice_fingerprint(&self, _tenant_id: &str) -> anyhow::Result<()> {
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

    #[test]
    fn test_infer_task_scores_modern_gemini() {
        // CORE-FIX: gemini-2.5/3.x must score 5 — they're frontier models.
        // The old heuristic missed them and they ranked equal to llama-3.1-8b.
        for id in &[
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.5-flash-lite",
            "gemini-3-flash-preview",
            "gemini-3.1-pro-preview",
        ] {
            let scores = infer_task_scores(id);
            assert_eq!(scores.coding, 5, "{} should score 5 for coding", id);
            assert_eq!(scores.analysis, 5, "{} should score 5 for analysis", id);
        }
    }

    #[test]
    fn test_infer_task_scores_modern_reasoning() {
        for id in &["openai/o3-mini", "deepseek/deepseek-r1", "x-ai/grok-4-fast"] {
            let scores = infer_task_scores(id);
            assert_eq!(scores.coding, 5, "{} should score 5 for coding", id);
        }
    }
}
