pub mod catalog;
pub mod key_pool;
pub mod siren;
pub mod syncer;

pub use siren::{SirenEngine, SirenRouter};

use crate::chal::SystemError;
use crate::pcb::{TaskType, PCB};
use crate::scheduler::ModelPreference;
pub use catalog::{ModelCatalog, ModelEntry};
pub use key_pool::KeyPool;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
    pub fallback_chain: Vec<FallbackDecision>,
}

/// A fallback candidate (no nested fallback_chain to avoid infinite recursion)
#[derive(Debug, Clone)]
pub struct FallbackDecision {
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
}

pub struct CognitiveRouter {
    catalog: Arc<ModelCatalog>,
    key_pool: Arc<KeyPool>,
}

impl CognitiveRouter {
    pub fn new(catalog: Arc<ModelCatalog>, key_pool: Arc<KeyPool>) -> Self {
        Self { catalog, key_pool }
    }

    /// Delegate key management to the underlying KeyPool
    pub async fn add_global_key(&self, entry: key_pool::ApiKeyEntry) -> anyhow::Result<()> {
        self.key_pool.add_global_key(entry).await
    }

    pub async fn add_tenant_key(
        &self,
        tenant_id: &str,
        entry: key_pool::ApiKeyEntry,
    ) -> anyhow::Result<()> {
        self.key_pool.add_tenant_key(tenant_id, entry).await
    }

    pub async fn list_global_keys(&self) -> Vec<key_pool::ApiKeyEntry> {
        self.key_pool.list_global_keys().await
    }

    pub async fn list_tenant_keys(&self, tenant_id: &str) -> Vec<key_pool::ApiKeyEntry> {
        self.key_pool.list_tenant_keys(tenant_id).await
    }

    pub async fn delete_key(&self, key_id: &str, tenant_id: Option<&str>) -> anyhow::Result<()> {
        self.key_pool.delete_key(key_id, tenant_id).await
    }

    pub async fn list_models_for_catalog(&self) -> Vec<ModelEntry> {
        self.catalog.all_entries().await
    }

    pub async fn decide(&self, pcb: &PCB) -> Result<RoutingDecision, SystemError> {
        let task_type = pcb.task_type;
        let model_pref = pcb.model_pref;
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        // Step 1: Get candidates from catalog
        let all_candidates = self.catalog.get_candidates(task_type).await;

        // Step 2: Filter by model preference
        let filtered: Vec<ModelEntry> = all_candidates
            .into_iter()
            .filter(|e| match model_pref {
                ModelPreference::LocalOnly => e.is_local,
                ModelPreference::CloudOnly => !e.is_local,
                ModelPreference::HybridSmart => true,
            })
            .collect();

        if filtered.is_empty() {
            return Err(SystemError::ModelNotFound(format!(
                "No models available for task_type={:?} with model_pref={:?}",
                task_type, model_pref
            )));
        }

        // Step 3: Filter by key availability and compute scores
        let mut scored: Vec<(f64, ModelEntry)> = Vec::new();

        for entry in filtered {
            let has_key = self
                .key_pool
                .has_key_for_model(&entry.provider, &entry.model_id)
                .await
                || entry.is_local; // local models don't need a key

            if !has_key {
                continue;
            }

            let score = self.compute_score(&entry, task_type, &scored);
            scored.push((score, entry));
        }

        if scored.is_empty() {
            return Err(SystemError::HardwareFailure(
                "No available keys for any candidate model".to_string(),
            ));
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Build routing decision from top candidate
        let (_, primary) = &scored[0];
        let primary_key = self.resolve_key(primary, tenant_id).await.ok_or_else(|| {
            SystemError::HardwareFailure(format!(
                "Key for provider '{}' became unavailable",
                primary.provider
            ))
        })?;

        let fallback_chain: Vec<FallbackDecision> = scored
            .iter()
            .skip(1)
            .take(2)
            .map(|(_, entry)| {
                // Best-effort fallback resolution — reuse primary key if same provider
                FallbackDecision {
                    model_id: entry.model_id.clone(),
                    provider: entry.provider.clone(),
                    api_url: entry_api_url(entry),
                    api_key: primary_key.api_key.clone(),
                }
            })
            .collect();

        Ok(RoutingDecision {
            model_id: primary.model_id.clone(),
            provider: primary.provider.clone(),
            api_url: primary_key
                .api_url
                .clone()
                .unwrap_or_else(|| entry_api_url(primary)),
            api_key: primary_key.api_key.clone(),
            fallback_chain,
        })
    }

    fn compute_score(
        &self,
        entry: &ModelEntry,
        task_type: TaskType,
        already_scored: &[(f64, ModelEntry)],
    ) -> f64 {
        let quality = entry.score_for(task_type) as f64 / 5.0;
        let avail = 1.0_f64; // Available (we already filtered unavailable)

        // cost_inv: lower cost = higher score. Normalize within candidates seen so far.
        let total_cost = entry.cost_input_per_mtok + entry.cost_output_per_mtok;
        let max_cost = already_scored
            .iter()
            .map(|(_, e)| e.cost_input_per_mtok + e.cost_output_per_mtok)
            .fold(total_cost, f64::max);

        let cost_inv = if max_cost > 0.0 {
            1.0 - (total_cost / max_cost)
        } else {
            1.0
        };

        // speed_inv: lower latency = higher score. Normalize within candidates seen so far.
        let latency = entry.avg_latency_ms.unwrap_or(1500) as f64;
        let max_latency = already_scored
            .iter()
            .map(|(_, e)| e.avg_latency_ms.unwrap_or(1500) as f64)
            .fold(latency, f64::max);

        let speed_inv = if max_latency > 0.0 {
            1.0 - (latency / max_latency)
        } else {
            1.0
        };

        quality * 0.40 + avail * 0.30 + cost_inv * 0.20 + speed_inv * 0.10
    }

    async fn resolve_key(
        &self,
        entry: &ModelEntry,
        tenant_id: &str,
    ) -> Option<key_pool::ApiKeyEntry> {
        if entry.is_local {
            // Local models don't need a key — return a dummy
            return Some(key_pool::ApiKeyEntry {
                key_id: "local".to_string(),
                provider: entry.provider.clone(),
                api_key: String::new(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
                active_models: None,
            });
        }
        self.key_pool
            .get_available_key(&entry.provider, &entry.model_id, tenant_id)
            .await
    }
}

fn entry_api_url(entry: &ModelEntry) -> String {
    // Default API URLs per provider
    match entry.provider.as_str() {
        // Compatible OpenAI — requiere key propia
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
        "ollama" => "http://localhost:11434/v1/chat/completions".to_string(),
        // Compatible OpenAI via OpenRouter — requiere key de OpenRouter
        "anthropic" | "deepseek" | "mistral" | "qwen" => {
            "https://openrouter.ai/api/v1/chat/completions".to_string()
        }
        // Google: compatible OpenAI via endpoint beta
        "google" => {
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions".to_string()
        }
        // OpenRouter: hub universal
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        // Fallback seguro
        _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use crate::router::catalog::{ModelCatalog, ModelProfile};
    use crate::router::key_pool::ApiKeyEntry;
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
    async fn test_decide_returns_decision_for_chat() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));

        // Add an anthropic key
        key_pool
            .add_global_key(ApiKeyEntry {
                key_id: "test-1".to_string(),
                provider: "anthropic".to_string(),
                api_key: "sk-ant-test".to_string(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
                active_models: None,
            })
            .await?;

        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new("test".to_string(), 5, "Hello".to_string());
        pcb.task_type = TaskType::Chat;
        pcb.model_pref = ModelPreference::CloudOnly;

        let decision = router.decide(&pcb).await?;
        assert!(!decision.model_id.is_empty());
        assert!(!decision.api_key.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_local_only_never_returns_cloud() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));

        // No keys needed for local models
        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new("test".to_string(), 5, "Hello".to_string());
        pcb.task_type = TaskType::Chat;
        pcb.model_pref = ModelPreference::LocalOnly;

        let decision = router.decide(&pcb).await?;
        // The model returned should be local
        assert!(!decision.model_id.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_no_keys_returns_error() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));

        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new("test".to_string(), 5, "Hello".to_string());
        pcb.task_type = TaskType::Coding;
        pcb.model_pref = ModelPreference::CloudOnly;

        let result = router.decide(&pcb).await;
        assert!(result.is_err(), "Should fail with no keys configured");
        Ok(())
    }
}
