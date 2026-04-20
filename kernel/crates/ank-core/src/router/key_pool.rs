use crate::scheduler::persistence::StatePersistor;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub key_id: String,
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub api_url: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub rate_limited_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub active_models: Option<Vec<String>>,
}

// Custom Debug to redact the api_key
impl std::fmt::Debug for ApiKeyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyEntry")
            .field("key_id", &self.key_id)
            .field("provider", &self.provider)
            .field("api_key", &"***REDACTED***")
            .field("label", &self.label)
            .field("is_active", &self.is_active)
            .field("rate_limited_until", &self.rate_limited_until)
            .finish()
    }
}

impl ApiKeyEntry {
    pub fn is_available(&self) -> bool {
        if !self.is_active {
            return false;
        }
        match self.rate_limited_until {
            Some(until) => Utc::now() > until,
            None => true,
        }
    }

    /// Returns a copy with api_key redacted — safe for listing/logging
    pub fn redacted(&self) -> Self {
        let mut copy = self.clone();
        copy.api_key = "***".to_string();
        copy
    }
}

pub struct KeyPool {
    global_keys: Arc<RwLock<Vec<ApiKeyEntry>>>,
    tenant_keys: Arc<RwLock<HashMap<String, Vec<ApiKeyEntry>>>>,
    persistence: Arc<dyn StatePersistor>,
    // Round-robin index per provider (provider_string -> AtomicUsize)
    rr_index: Arc<RwLock<HashMap<String, Arc<AtomicUsize>>>>,
}

impl KeyPool {
    pub fn new(persistence: Arc<dyn StatePersistor>) -> Self {
        Self {
            global_keys: Arc::new(RwLock::new(Vec::new())),
            tenant_keys: Arc::new(RwLock::new(HashMap::new())),
            persistence,
            rr_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load persisted keys from the DB on startup.
    /// Keys are stored as synthetic PCBs with special PIDs — we reuse StatePersistor.
    pub async fn load(&self) -> anyhow::Result<()> {
        let all_pcbs = self.persistence.load_all_pcbs().await?;
        let mut global = self.global_keys.write().await;
        let mut tenants = self.tenant_keys.write().await;
        for pcb in all_pcbs {
            // We encode KeyPool entries as PCBs where:
            // - pid = "keypool:global:{key_id}" or "keypool:tenant:{tid}:{key_id}"
            // - l1_instruction = JSON of ApiKeyEntry
            if let Some(rest) = pcb.pid.strip_prefix("keypool:global:") {
                let _ = rest; // key_id embedded in pid
                if let Ok(entry) =
                    serde_json::from_str::<ApiKeyEntry>(&pcb.memory_pointers.l1_instruction)
                {
                    global.push(entry);
                }
            } else if let Some(rest) = pcb.pid.strip_prefix("keypool:tenant:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let tid = parts[0].to_string();
                    if let Ok(entry) =
                        serde_json::from_str::<ApiKeyEntry>(&pcb.memory_pointers.l1_instruction)
                    {
                        tenants.entry(tid).or_default().push(entry);
                    }
                }
            }
        }
        Ok(())
    }

    /// Get an available key for a provider and model, checking tenant override first, then global pool.
    pub async fn get_available_key(
        &self,
        provider: &str,
        model_id: &str,
        tenant_id: &str,
    ) -> Option<ApiKeyEntry> {
        // 1. Try tenant override
        {
            let tenants = self.tenant_keys.read().await;
            if let Some(keys) = tenants.get(tenant_id) {
                let available: Vec<&ApiKeyEntry> = keys
                    .iter()
                    .filter(|k| {
                        k.provider == provider
                            && k.is_available()
                            && k.active_models
                                .as_ref()
                                .map(|m| m.contains(&model_id.to_string()))
                                .unwrap_or(true)
                    })
                    .collect();
                if !available.is_empty() {
                    let idx = self.next_rr_index(provider).await;
                    return Some(available[idx % available.len()].clone());
                }
            }
        }
        // 2. Try global pool
        {
            let global = self.global_keys.read().await;
            let available: Vec<&ApiKeyEntry> = global
                .iter()
                .filter(|k| {
                    k.provider == provider
                        && k.is_available()
                        && k.active_models
                            .as_ref()
                            .map(|m| m.contains(&model_id.to_string()))
                            .unwrap_or(true)
                })
                .collect();
            if !available.is_empty() {
                let idx = self.next_rr_index(provider).await;
                return Some(available[idx % available.len()].clone());
            }
        }
        None
    }

    async fn next_rr_index(&self, provider: &str) -> usize {
        let mut map = self.rr_index.write().await;
        let counter = map
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)));
        counter.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn mark_rate_limited(&self, key_id: &str, until: DateTime<Utc>) {
        {
            let mut global = self.global_keys.write().await;
            for k in global.iter_mut() {
                if k.key_id == key_id {
                    k.rate_limited_until = Some(until);
                    return;
                }
            }
        }
        let mut tenants = self.tenant_keys.write().await;
        for keys in tenants.values_mut() {
            for k in keys.iter_mut() {
                if k.key_id == key_id {
                    k.rate_limited_until = Some(until);
                    return;
                }
            }
        }
    }

    pub async fn add_global_key(&self, entry: ApiKeyEntry) -> anyhow::Result<()> {
        let json = serde_json::to_string(&entry).context("Failed to serialize ApiKeyEntry")?;
        // Store as a synthetic PCB
        let pcb = self.make_pcb(&format!("keypool:global:{}", entry.key_id), &json);
        self.persistence.save_pcb(&pcb).await?;
        let mut global = self.global_keys.write().await;
        // Replace if exists, otherwise push
        if let Some(existing) = global.iter_mut().find(|k| k.key_id == entry.key_id) {
            *existing = entry;
        } else {
            global.push(entry);
        }
        Ok(())
    }

    pub async fn add_tenant_key(&self, tenant_id: &str, entry: ApiKeyEntry) -> anyhow::Result<()> {
        let json = serde_json::to_string(&entry).context("Failed to serialize ApiKeyEntry")?;
        let pcb = self.make_pcb(
            &format!("keypool:tenant:{}:{}", tenant_id, entry.key_id),
            &json,
        );
        self.persistence.save_pcb(&pcb).await?;
        let mut tenants = self.tenant_keys.write().await;
        let keys = tenants.entry(tenant_id.to_string()).or_default();
        if let Some(existing) = keys.iter_mut().find(|k| k.key_id == entry.key_id) {
            *existing = entry;
        } else {
            keys.push(entry);
        }
        Ok(())
    }

    pub async fn list_global_keys(&self) -> Vec<ApiKeyEntry> {
        let global = self.global_keys.read().await;
        global.iter().map(|k| k.redacted()).collect()
    }

    pub async fn list_tenant_keys(&self, tenant_id: &str) -> Vec<ApiKeyEntry> {
        let tenants = self.tenant_keys.read().await;
        tenants
            .get(tenant_id)
            .map(|keys| keys.iter().map(|k| k.redacted()).collect())
            .unwrap_or_default()
    }

    pub async fn delete_key(&self, key_id: &str, tenant_id: Option<&str>) -> anyhow::Result<()> {
        match tenant_id {
            None => {
                // Delete from global
                let mut global = self.global_keys.write().await;
                global.retain(|k| k.key_id != key_id);
                self.persistence
                    .delete_pcb(&format!("keypool:global:{}", key_id))
                    .await?;
            }
            Some(tid) => {
                let mut tenants = self.tenant_keys.write().await;
                if let Some(keys) = tenants.get_mut(tid) {
                    keys.retain(|k| k.key_id != key_id);
                }
                self.persistence
                    .delete_pcb(&format!("keypool:tenant:{}:{}", tid, key_id))
                    .await?;
            }
        }
        Ok(())
    }

    /// Check if at least one key is available for a given provider and model
    pub async fn has_key_for_model(&self, provider: &str, model_id: &str) -> bool {
        let global = self.global_keys.read().await;
        if global.iter().any(|k| {
            k.provider == provider
                && k.is_available()
                && k.active_models
                    .as_ref()
                    .map(|m| m.contains(&model_id.to_string()))
                    .unwrap_or(true)
        }) {
            return true;
        }
        drop(global);
        let tenants = self.tenant_keys.read().await;
        tenants.values().any(|keys| {
            keys.iter().any(|k| {
                k.provider == provider
                    && k.is_available()
                    && k.active_models
                        .as_ref()
                        .map(|m| m.contains(&model_id.to_string()))
                        .unwrap_or(true)
            })
        })
    }

    /// Check if there's an OpenRouter key available (used by CatalogSyncer)
    pub async fn has_openrouter_key(&self) -> bool {
        let global = self.global_keys.read().await;
        if global
            .iter()
            .any(|k| k.provider == "openrouter" && k.is_available())
        {
            return true;
        }
        drop(global);
        let tenants = self.tenant_keys.read().await;
        tenants.values().any(|keys| {
            keys.iter()
                .any(|k| k.provider == "openrouter" && k.is_available())
        })
    }

    fn make_pcb(&self, pid: &str, json: &str) -> crate::pcb::PCB {
        let mut pcb = crate::pcb::PCB::new(pid.to_string(), 0, json.to_string());
        pcb.pid = pid.to_string();
        pcb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use crate::scheduler::persistence::{StatePersistor, VoiceProfile};
    use anyhow::Result;

    struct NoopPersistor;

    #[async_trait::async_trait]
    impl StatePersistor for NoopPersistor {
        async fn save_pcb(&self, _pcb: &PCB) -> Result<()> {
            Ok(())
        }
        async fn delete_pcb(&self, _pid: &str) -> Result<()> {
            Ok(())
        }
        async fn load_all_pcbs(&self) -> Result<Vec<PCB>> {
            Ok(vec![])
        }
        async fn flush(&self) -> Result<()> {
            Ok(())
        }
        async fn get_voice_profile(&self, _tenant_id: &str) -> Result<Option<VoiceProfile>> {
            Ok(None)
        }
        async fn update_voice_profile(&self, _profile: VoiceProfile) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_rate_limited_keys_not_returned() -> anyhow::Result<()> {
        let pool = KeyPool::new(Arc::new(NoopPersistor));

        // Add 3 keys for the same provider
        for i in 1..=3u8 {
            let entry = ApiKeyEntry {
                key_id: format!("key-{}", i),
                provider: "anthropic".to_string(),
                api_key: format!("sk-test-{}", i),
                api_url: None,
                label: Some(format!("Key {}", i)),
                is_active: true,
                rate_limited_until: None,
            };
            pool.add_global_key(entry).await?;
        }

        // Mark keys 1 and 2 as rate limited
        let future = Utc::now() + chrono::Duration::hours(1);
        pool.mark_rate_limited("key-1", future).await;
        pool.mark_rate_limited("key-2", future).await;

        // Only key-3 should be returned
        let key = pool.get_available_key("anthropic", "tenant-x").await;
        assert!(key.is_some(), "Should return the available key");
        assert_eq!(key.map(|k| k.key_id), Some("key-3".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_tenant_override_priority() -> anyhow::Result<()> {
        let pool = KeyPool::new(Arc::new(NoopPersistor));

        // Global key
        pool.add_global_key(ApiKeyEntry {
            key_id: "global-1".to_string(),
            provider: "openai".to_string(),
            api_key: "sk-global".to_string(),
            api_url: None,
            label: None,
            is_active: true,
            rate_limited_until: None,
        })
        .await?;

        // Tenant override
        pool.add_tenant_key(
            "tenant-a",
            ApiKeyEntry {
                key_id: "tenant-a-1".to_string(),
                provider: "openai".to_string(),
                api_key: "sk-tenant".to_string(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
            },
        )
        .await?;

        let key = pool.get_available_key("openai", "tenant-a").await;
        assert!(key.is_some());
        assert_eq!(
            key.map(|k| k.key_id),
            Some("tenant-a-1".to_string()),
            "Tenant key should take priority"
        );

        Ok(())
    }
}
