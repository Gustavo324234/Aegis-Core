use crate::router::catalog::{ModelCatalog, ModelEntry, TaskScores, ToolUseSupport};
use crate::router::key_pool::KeyPool;
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
        // Sync non-OpenRouter provider models (ollama, custom, etc.) from BOTH global
        // and tenant key pools (CORE-362) so discovered models survive restarts.
        // Unredacted because the Ollama prune path below probes the live API with
        // real credentials.
        let mut all_keys = self.key_pool.list_global_keys_unredacted().await;
        all_keys.extend(self.key_pool.list_all_tenant_keys_unredacted().await);
        for key in &all_keys {
            if key.provider == "openrouter" {
                continue;
            }

            // Dynamic discovery: probe provider's live models via /models
            let models_to_register: Option<Vec<String>> =
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
                            "CatalogSyncer: discovery probe failed — using stored active_models"
                        );
                        key.active_models.clone()
                    }
                };

            if let Some(models) = &models_to_register {
                if !models.is_empty() {
                    self.key_pool
                        .update_key_active_models(&key.key_id, models.clone())
                        .await;
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

        // 2. Fetch live OpenRouter catalog (public keyless endpoint, attaches auth if key available).
        // This synchronizes real-world pricing, context windows, tool support, and auto-discovers
        // free models and all active models.
        let mut req = self
            .client
            .get("https://openrouter.ai/api/v1/models")
            .timeout(std::time::Duration::from_secs(15));

        if let Some(openrouter_key) = self.key_pool.get_openrouter_key().await {
            req = req.header("Authorization", format!("Bearer {}", openrouter_key));
        }

        let response = match req.send().await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                warn!(
                    "CatalogSyncer: OpenRouter public models API returned status: {}",
                    resp.status()
                );
                return Ok(());
            }
            Err(e) => {
                warn!("CatalogSyncer: OpenRouter public models API fetch failed (retaining cached catalog): {}", e);
                return Ok(());
            }
        };

        let model_list: OpenRouterModelList = match response.json().await {
            Ok(list) => list,
            Err(e) => {
                warn!(
                    "CatalogSyncer: failed to parse OpenRouter model list JSON: {}",
                    e
                );
                return Ok(());
            }
        };

        let mut current = self.catalog.all_entries().await;
        let has_openrouter_key = self.key_pool.has_openrouter_key().await;
        let mut updated_count = 0usize;
        let mut added_free_count = 0usize;

        for remote in model_list.data {
            let parsed_input =
                parse_pricing_per_mtok(remote.pricing.as_ref().and_then(|p| p.prompt.as_ref()));
            let parsed_output =
                parse_pricing_per_mtok(remote.pricing.as_ref().and_then(|p| p.completion.as_ref()));

            let supports_tools = remote
                .supported_parameters
                .as_ref()
                .map(|p| p.iter().any(|s| s == "tools"))
                .unwrap_or(false);

            let supports_json = remote
                .supported_parameters
                .as_ref()
                .map(|p| p.iter().any(|s| s == "json_mode" || s == "response_format"))
                .unwrap_or(false);

            let context_window = remote.context_length.unwrap_or(4096);
            let is_free = parsed_input == Some(0.0)
                && parsed_output == Some(0.0)
                && remote.id != "openrouter/auto";

            if let Some(existing) = current.iter_mut().find(|e| e.model_id == remote.id) {
                // Skip local models — never overwrite from remote
                if existing.is_local {
                    continue;
                }
                // Preserve catalog pricing when OpenRouter reports negative/variable rates (e.g. "-1")
                if let (Some(cost_input), Some(cost_output)) = (parsed_input, parsed_output) {
                    existing.cost_input_per_mtok = cost_input;
                    existing.cost_output_per_mtok = cost_output;
                }
                existing.context_window = context_window;
                existing.supports_tools = supports_tools;
                existing.supports_json_mode = supports_json;
                existing.display_name = remote.name;
                if supports_tools && existing.tool_use_support == ToolUseSupport::Unknown {
                    existing.tool_use_support = ToolUseSupport::Supported;
                }
                if is_free {
                    existing.free_tier_eligible = true;
                } else if existing.provider == "openrouter" {
                    existing.free_tier_eligible = false;
                }
                updated_count += 1;
            } else if is_free || has_openrouter_key {
                let (Some(cost_input), Some(cost_output)) = (parsed_input, parsed_output) else {
                    continue;
                };
                // New model not in local catalog — add if it's free or if we have an OpenRouter key
                let scores = infer_task_scores(&remote.id);
                let provider = remote.id.split('/').next().unwrap_or("unknown").to_string();
                current.push(ModelEntry {
                    model_id: remote.id,
                    provider,
                    display_name: if is_free && !remote.name.contains("(free)") {
                        format!("{} (free)", remote.name)
                    } else {
                        remote.name
                    },
                    context_window,
                    cost_input_per_mtok: cost_input,
                    cost_output_per_mtok: cost_output,
                    supports_tools,
                    supports_json_mode: supports_json,
                    task_scores: scores,
                    is_local: false,
                    avg_latency_ms: if is_free { Some(2500) } else { None },
                    free_tier_rpm: if is_free { Some(20) } else { None },
                    free_tier_rpd: if is_free { Some(200) } else { None },
                    free_tier_eligible: is_free,
                    tool_use_support: if supports_tools {
                        ToolUseSupport::Supported
                    } else {
                        ToolUseSupport::Unknown
                    },
                });
                if is_free {
                    added_free_count += 1;
                }
            }
        }

        info!(
            "CatalogSyncer: OpenRouter sync complete. Updated {} models, added {} new free-tier models.",
            updated_count, added_free_count
        );

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
        // Avoid duplicate registration if a prefixed version of the model is already in the catalog.
        let prefixed_id = format!("{}/{}", provider, model_id);
        if catalog.find(&prefixed_id).await.is_some() {
            continue;
        }
        let scores = infer_task_scores(model_id);
        let is_free_discovered =
            provider == "ollama" || provider == "ollama_cloud" || model_id.ends_with(":free");
        let (cost_in, cost_out, avg_lat) = if is_free_discovered {
            (0.0, 0.0, Some(1500))
        } else {
            // Do not falsely mark uncurated cloud /models entries (e.g. Gemini previews)
            // as $0.00/1500ms; otherwise they outrank vetted flash models in models.yaml.
            (0.25, 1.0, Some(2500))
        };
        catalog
            .add_entry(ModelEntry {
                model_id: model_id.to_string(),
                provider: provider.to_string(),
                display_name: format!("{} ({})", model_id, provider),
                context_window: 32_768,
                cost_input_per_mtok: cost_in,
                cost_output_per_mtok: cost_out,
                supports_tools: true,
                supports_json_mode: true,
                tool_use_support: ToolUseSupport::Unknown,
                is_local: provider == "ollama",
                avg_latency_ms: avg_lat,
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

fn parse_pricing_per_mtok(val: Option<&serde_json::Value>) -> Option<f64> {
    let raw = match val? {
        serde_json::Value::Number(n) => n.as_f64()?,
        serde_json::Value::String(s) => s.parse::<f64>().ok()?,
        _ => return None,
    };
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    Some(raw * 1_000_000.0)
}

/// Parses a single OpenRouter model from JSON, returning a ModelEntry if it is free-tier
/// (`pricing.prompt == 0.0` and non-negative non-paid `pricing.completion`).
pub fn parse_openrouter_free_model(model: &serde_json::Value) -> Option<ModelEntry> {
    let model_id = match model["id"].as_str() {
        Some(id) if !id.is_empty() && id != "openrouter/auto" => id.to_string(),
        _ => return None,
    };

    let prompt_price_mtok =
        parse_pricing_per_mtok(model.get("pricing").and_then(|p| p.get("prompt")))?;
    if prompt_price_mtok != 0.0 {
        return None;
    }
    if let Some(completion_val) = model.get("pricing").and_then(|p| p.get("completion")) {
        if parse_pricing_per_mtok(Some(completion_val)) != Some(0.0) {
            return None;
        }
    }

    let context = model["context_length"].as_u64().unwrap_or(131_072) as u32;
    let name = model["name"].as_str().unwrap_or(&model_id).to_string();

    let supports_tools = model["supported_parameters"]
        .as_array()
        .map(|arr| arr.iter().any(|v| v.as_str() == Some("tools")))
        .unwrap_or(false);
    let supports_json = model["supported_parameters"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|v| v.as_str() == Some("json_mode") || v.as_str() == Some("response_format"))
        })
        .unwrap_or(false);

    let scores = infer_task_scores(&model_id);

    Some(ModelEntry {
        model_id,
        provider: "openrouter".to_string(),
        display_name: format!("{} (free)", name),
        context_window: context,
        cost_input_per_mtok: 0.0,
        cost_output_per_mtok: 0.0,
        supports_tools,
        supports_json_mode: supports_json,
        tool_use_support: if supports_tools {
            ToolUseSupport::Supported
        } else {
            ToolUseSupport::Unknown
        },
        is_local: false,
        avg_latency_ms: Some(2500),
        free_tier_rpm: Some(20),
        free_tier_rpd: Some(200),
        free_tier_eligible: true,
        task_scores: scores,
    })
}

/// Fetches the OpenRouter model list with the given key, filters to free-tier
/// models (pricing.prompt == "0" or 0.0), and adds any that are not yet in the catalog.
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
        if let Some(entry) = parse_openrouter_free_model(model) {
            if catalog.find(&entry.model_id).await.is_some() {
                continue;
            }
            catalog.add_entry(entry).await;
            added += 1;
        }
    }

    Ok(added)
}

fn infer_task_scores(model_id: &str) -> TaskScores {
    let id_lower = model_id.to_lowercase();

    // 1. Specialized Coding Models
    let is_coder = id_lower.contains("coder")
        || id_lower.contains("codestral")
        || id_lower.contains("code-llama")
        || id_lower.contains("qwen2.5-coder")
        || id_lower.contains("qwen3-coder")
        || id_lower.contains("starcoder")
        || id_lower.contains("north-mini-code");

    if is_coder {
        let is_large_coder = id_lower.contains("32b")
            || id_lower.contains("70b")
            || id_lower.contains("72b")
            || id_lower.contains("codestral");
        return if is_large_coder {
            TaskScores {
                chat: 4,
                coding: 5,
                planning: 5,
                analysis: 5,
                summarization: 4,
                extraction: 5,
            }
        } else {
            TaskScores {
                chat: 3,
                coding: 5,
                planning: 4,
                analysis: 4,
                summarization: 3,
                extraction: 4,
            }
        };
    }

    // 2. Frontier / SOTA Reasoning & Foundation Models (Elo ~1350+)
    let is_frontier = id_lower.contains("claude")
        || id_lower.contains("gpt-4")
        || id_lower.contains("gpt-5")
        || id_lower.contains("/o1")
        || id_lower.starts_with("o1")
        || id_lower.contains("/o3")
        || id_lower.starts_with("o3")
        || id_lower.contains("/o4")
        || id_lower.starts_with("o4")
        || id_lower.contains("gemini-2.5")
        || id_lower.contains("gemini-3")
        || id_lower.contains("deepseek-r")
        || id_lower.contains("deepseek-v3")
        || id_lower.contains("deepseek-v4")
        || id_lower.contains("grok-3")
        || id_lower.contains("grok-4")
        || id_lower.contains("gpt-oss-120b")
        || id_lower.contains("gpt-oss")
        || id_lower.contains("llama-3.3")
        || id_lower.contains("llama-4")
        || id_lower.contains("qwen3")
        || id_lower.contains("qwen-2.5")
        || id_lower.contains("gemma-4")
        || id_lower.contains("gemma4")
        || id_lower.contains("mistral-large")
        || id_lower.contains("kimi")
        || id_lower.contains("openrouter/free")
        || id_lower.contains("nemotron-3-ultra")
        || id_lower.contains("nemotron-3.5")
        || id_lower.contains("ernie-5.5");

    if is_frontier {
        return TaskScores {
            chat: 5,
            coding: 5,
            planning: 5,
            analysis: 5,
            summarization: 4,
            extraction: 5,
        };
    }

    // 3. Compact / Edge Models (8b, 7b, 3b, small, etc.)
    let is_compact = id_lower.contains("8b")
        || id_lower.contains("7b")
        || id_lower.contains("3b")
        || id_lower.contains("1b")
        || id_lower.contains("1.7b")
        || id_lower.contains("2.6b")
        || id_lower.contains("mini")
        || id_lower.contains("nano")
        || id_lower.contains("small")
        || id_lower.contains("lfm")
        || id_lower.contains("flint")
        || id_lower.contains("inkling");

    if is_compact {
        return TaskScores {
            chat: 3,
            coding: 3,
            planning: 3,
            analysis: 3,
            summarization: 3,
            extraction: 3,
        };
    }

    // Default
    TaskScores {
        chat: 3,
        coding: 3,
        planning: 3,
        analysis: 3,
        summarization: 3,
        extraction: 3,
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
    async fn test_sync_public_openrouter_catalog() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let syncer = CatalogSyncer::new(catalog.clone(), key_pool);

        let before = catalog.all_entries().await.len();
        syncer.sync_once().await?;
        let after = catalog.all_entries().await.len();

        // Public OpenRouter sync discovers free models even without an OpenRouter key
        assert!(
            after >= before,
            "Catalog should retain or expand with free models from public directory"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_register_provider_models_skips_prefixed_duplicates() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);

        // Add a prefixed model to the catalog
        catalog
            .add_entry(ModelEntry {
                model_id: "ollama_cloud/gpt-oss:120b-cloud".to_string(),
                provider: "ollama_cloud".to_string(),
                display_name: "GPT-OSS 120B (Ollama Cloud)".to_string(),
                context_window: 128000,
                cost_input_per_mtok: 0.8,
                cost_output_per_mtok: 2.0,
                supports_tools: true,
                supports_json_mode: true,
                tool_use_support: ToolUseSupport::Unknown,
                is_local: false,
                avg_latency_ms: Some(1800),
                free_tier_rpm: None,
                free_tier_rpd: None,
                free_tier_eligible: true,
                task_scores: TaskScores {
                    chat: 4,
                    coding: 4,
                    planning: 4,
                    analysis: 4,
                    summarization: 4,
                    extraction: 4,
                },
            })
            .await;

        let before = catalog.all_entries().await.len();

        // Registering the bare model ID under the same provider
        let added = register_provider_models(
            "ollama_cloud",
            &["gpt-oss:120b-cloud".to_string()],
            &catalog,
        )
        .await;

        let after = catalog.all_entries().await.len();

        assert_eq!(
            added, 0,
            "Should add 0 new models because it's a prefixed duplicate"
        );
        assert_eq!(before, after, "Catalog length should remain unchanged");

        // Registering a completely new model ID should work
        let added_new = register_provider_models(
            "ollama_cloud",
            &["completely-new-model:7b".to_string()],
            &catalog,
        )
        .await;

        assert_eq!(added_new, 1, "Should add 1 new model");
        assert_eq!(catalog.all_entries().await.len(), before + 1);

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
            "gemini-3.5-flash",
            "gemini-3.6-flash",
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

    #[test]
    fn test_parse_openrouter_free_model_pricing() {
        // 1. String price "0" (free)
        let model_str_zero = serde_json::json!({
            "id": "meta-llama/llama-3-8b-instruct:free",
            "name": "Llama 3 8B Instruct (Free)",
            "context_length": 8192,
            "pricing": {
                "prompt": "0",
                "completion": "0"
            }
        });
        let entry =
            parse_openrouter_free_model(&model_str_zero).expect("Should parse successfully");
        assert_eq!(entry.model_id, "meta-llama/llama-3-8b-instruct:free");
        assert_eq!(entry.cost_input_per_mtok, 0.0);
        assert_eq!(entry.cost_output_per_mtok, 0.0);
        assert_eq!(entry.display_name, "Llama 3 8B Instruct (Free) (free)");

        // 2. Numeric price 0.0 (free)
        let model_num_zero = serde_json::json!({
            "id": "meta-llama/llama-3-8b-instruct:free",
            "name": "Llama 3 8B Instruct (Free)",
            "context_length": 8192,
            "pricing": {
                "prompt": 0.0,
                "completion": 0.0
            }
        });
        let entry2 =
            parse_openrouter_free_model(&model_num_zero).expect("Should parse successfully");
        assert_eq!(entry2.model_id, "meta-llama/llama-3-8b-instruct:free");

        // 3. String price "0.0005" (paid) - should be skipped
        let model_str_paid = serde_json::json!({
            "id": "meta-llama/llama-3-8b-instruct",
            "name": "Llama 3 8B Instruct",
            "pricing": {
                "prompt": "0.0005",
                "completion": "0.0005"
            }
        });
        assert!(parse_openrouter_free_model(&model_str_paid).is_none());

        // 4. Numeric price 0.0005 (paid) - should be skipped
        let model_num_paid = serde_json::json!({
            "id": "meta-llama/llama-3-8b-instruct",
            "name": "Llama 3 8B Instruct",
            "pricing": {
                "prompt": 0.0005,
                "completion": 0.0005
            }
        });
        assert!(parse_openrouter_free_model(&model_num_paid).is_none());

        // 5. Missing price - defaults to paid (skipped)
        let model_missing_price = serde_json::json!({
            "id": "meta-llama/llama-3-8b-instruct",
            "name": "Llama 3 8B Instruct"
        });
        assert!(parse_openrouter_free_model(&model_missing_price).is_none());

        // 6. Negative dynamic pricing ("-1" e.g. openrouter/auto) - must NEVER be treated as free
        let model_negative_price = serde_json::json!({
            "id": "openrouter/auto",
            "name": "Auto Router",
            "pricing": {
                "prompt": "-1",
                "completion": "-1"
            }
        });
        assert!(parse_openrouter_free_model(&model_negative_price).is_none());
        assert_eq!(
            parse_pricing_per_mtok(
                model_negative_price
                    .get("pricing")
                    .and_then(|p| p.get("prompt"))
            ),
            None
        );

        let model_negative_completion = serde_json::json!({
            "id": "vendor/weird-model:free",
            "name": "Weird Model",
            "pricing": {
                "prompt": "0",
                "completion": "-1"
            }
        });
        assert!(parse_openrouter_free_model(&model_negative_completion).is_none());
    }
}
