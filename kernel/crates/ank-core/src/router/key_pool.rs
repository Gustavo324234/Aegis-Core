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
    /// Si es true, esta clave usa el nivel gratuito del proveedor.
    /// El router prioriza claves gratuitas y cae a las pagas cuando
    /// todas las gratuitas están rate-limitadas.
    #[serde(default)]
    pub is_free_tier: bool,
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

/// Check whether a stored model-ID (from active_models, as returned by the
/// provider's /v1/models endpoint) matches a catalog model-ID.
///
/// The catalog prefixes model IDs with the provider name:
///   catalog → "groq/llama-3.3-70b-versatile"
///   Groq API → "llama-3.3-70b-versatile"   (no prefix)
///   OpenRouter → "meta-llama/llama-3.3-70b-instruct"  (already has a slash)
///
/// Matching rules (in order):
///   1. Exact match.
///   2. The stored ID equals the part after the first "/" in the catalog ID.
///      e.g. "llama-3.3-70b-versatile" matches "groq/llama-3.3-70b-versatile".
///   3. The catalog ID ends with the stored ID (handles nested prefixes).
fn model_id_matches(catalog_id: &str, stored_id: &str) -> bool {
    if catalog_id == stored_id {
        return true;
    }
    // Strip the leading "provider/" prefix from the catalog ID and compare.
    if let Some(bare) = catalog_id.split_once('/').map(|(_, s)| s) {
        if bare == stored_id {
            return true;
        }
    }
    // Also accept if the catalog ID ends with "/" + stored_id (nested prefixes).
    if catalog_id.ends_with(&format!("/{}", stored_id)) {
        return true;
    }
    false
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
                match serde_json::from_str::<ApiKeyEntry>(&pcb.memory_pointers.l1_instruction) {
                    Ok(entry) => {
                        global.push(entry);
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            pid = %pcb.pid,
                            "CORE-213: Failed to deserialize global ApiKeyEntry from persisted PCB"
                        );
                    }
                }
            } else if let Some(rest) = pcb.pid.strip_prefix("keypool:tenant:") {
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let tid = parts[0].to_string();
                    match serde_json::from_str::<ApiKeyEntry>(&pcb.memory_pointers.l1_instruction) {
                        Ok(entry) => {
                            tenants.entry(tid).or_default().push(entry);
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                pid = %pcb.pid,
                                tenant_id = %tid,
                                "CORE-213: Failed to deserialize tenant ApiKeyEntry from persisted PCB"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        pid = %pcb.pid,
                        "CORE-213: Persisted tenant key PCB has invalid pid format (expected 'keypool:tenant:{{tid}}:{{key_id}}')"
                    );
                }
            }
        }
        Ok(())
    }

    /// Get an available key for a provider and model.
    /// Priority order:
    ///   1. Tenant free-tier keys  (gratuitas primero — se agotan antes de usar pagas)
    ///   2. Global free-tier keys
    ///   3. Tenant paid keys
    ///   4. Global paid keys
    pub async fn get_available_key(
        &self,
        provider: &str,
        model_id: &str,
        tenant_id: &str,
    ) -> Option<ApiKeyEntry> {
        let model_matches = |k: &&ApiKeyEntry| -> bool {
            k.provider == provider
                && k.is_available()
                && k.active_models
                    .as_ref()
                    .map(|m| m.iter().any(|s| model_id_matches(model_id, s)))
                    .unwrap_or(true)
        };

        let tenant_keys = self.tenant_keys.read().await;
        let global_keys = self.global_keys.read().await;

        let tenant = tenant_keys
            .get(tenant_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Collect candidates in priority order: tenant-free, global-free, tenant-paid, global-paid
        let buckets: [Vec<&ApiKeyEntry>; 4] = [
            tenant
                .iter()
                .filter(|k| k.is_free_tier && model_matches(k))
                .collect(),
            global_keys
                .iter()
                .filter(|k| k.is_free_tier && model_matches(k))
                .collect(),
            tenant
                .iter()
                .filter(|k| !k.is_free_tier && model_matches(k))
                .collect(),
            global_keys
                .iter()
                .filter(|k| !k.is_free_tier && model_matches(k))
                .collect(),
        ];

        for bucket in &buckets {
            if !bucket.is_empty() {
                let idx = self.next_rr_index(provider).await;
                return Some(bucket[idx % bucket.len()].clone());
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

    /// CORE-FIX: like `get_available_key` but skips any key whose `key_id` is
    /// in `exclude_ids`. Used by the chal layer to rotate through alternate
    /// keys when the previous one returned an empty stream — empty responses
    /// don't (and shouldn't) mark the key globally rate-limited, so we need
    /// a way to ask for "any *other* available key" within a single request
    /// without polluting the global state.
    pub async fn get_available_key_excluding(
        &self,
        provider: &str,
        model_id: &str,
        tenant_id: &str,
        exclude_ids: &std::collections::HashSet<String>,
    ) -> Option<ApiKeyEntry> {
        let model_matches = |k: &&ApiKeyEntry| -> bool {
            k.provider == provider
                && k.is_available()
                && !exclude_ids.contains(&k.key_id)
                && k.active_models
                    .as_ref()
                    .map(|m| m.iter().any(|s| model_id_matches(model_id, s)))
                    .unwrap_or(true)
        };

        let tenant_keys = self.tenant_keys.read().await;
        let global_keys = self.global_keys.read().await;

        let tenant = tenant_keys
            .get(tenant_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Priority order: tenant-free, global-free, tenant-paid, global-paid
        let buckets: [Vec<&ApiKeyEntry>; 4] = [
            tenant
                .iter()
                .filter(|k| k.is_free_tier && model_matches(k))
                .collect(),
            global_keys
                .iter()
                .filter(|k| k.is_free_tier && model_matches(k))
                .collect(),
            tenant
                .iter()
                .filter(|k| !k.is_free_tier && model_matches(k))
                .collect(),
            global_keys
                .iter()
                .filter(|k| !k.is_free_tier && model_matches(k))
                .collect(),
        ];

        for bucket in &buckets {
            if !bucket.is_empty() {
                let idx = self.next_rr_index(provider).await;
                return Some(bucket[idx % bucket.len()].clone());
            }
        }
        None
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

    /// Like `list_global_keys` but WITHOUT redacting `api_key`. Crate-internal
    /// only — used by the CatalogSyncer to run live discovery probes that need
    /// real credentials. MUST NOT be exposed through the HTTP layer.
    pub(crate) async fn list_global_keys_unredacted(&self) -> Vec<ApiKeyEntry> {
        self.global_keys.read().await.clone()
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

    /// Returns true if at least one non-free-tier (paid) key is available for this provider+model.
    /// When a paid key exists, free-tier rate limits can be safely ignored.
    pub async fn has_paid_key(&self, provider: &str, model_id: &str) -> bool {
        let model_ok = |k: &ApiKeyEntry| -> bool {
            k.provider == provider
                && !k.is_free_tier
                && k.is_available()
                && k.active_models
                    .as_ref()
                    .map(|m| m.iter().any(|s| model_id_matches(model_id, s)))
                    .unwrap_or(true)
        };

        let global = self.global_keys.read().await;
        if global.iter().any(&model_ok) {
            return true;
        }
        drop(global);
        let tenants = self.tenant_keys.read().await;
        tenants.values().any(|keys| keys.iter().any(&model_ok))
    }

    /// Check if at least one key is available for a given provider and model
    pub async fn has_key_for_model(&self, provider: &str, model_id: &str) -> bool {
        let global = self.global_keys.read().await;
        if global.iter().any(|k| {
            k.provider == provider
                && k.is_available()
                && k.active_models
                    .as_ref()
                    .map(|m| m.iter().any(|s| model_id_matches(model_id, s)))
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
                        .map(|m| m.iter().any(|s| model_id_matches(model_id, s)))
                        .unwrap_or(true)
            })
        })
    }

    /// Returns the full (unredacted) entry for a given key_id.
    /// If tenant_id is Some, searches tenant keys first, then global.
    /// Used by update handlers to preserve the existing api_key when the caller omits it.
    pub async fn get_raw_key_by_id(
        &self,
        key_id: &str,
        tenant_id: Option<&str>,
    ) -> Option<ApiKeyEntry> {
        if let Some(tid) = tenant_id {
            let tenants = self.tenant_keys.read().await;
            if let Some(entry) = tenants
                .get(tid)
                .and_then(|keys| keys.iter().find(|k| k.key_id == key_id).cloned())
            {
                return Some(entry);
            }
        }
        let global = self.global_keys.read().await;
        global.iter().find(|k| k.key_id == key_id).cloned()
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

    pub(crate) async fn list_tenant_keys_unredacted(&self, tenant_id: &str) -> Vec<ApiKeyEntry> {
        let tenants = self.tenant_keys.read().await;
        tenants.get(tenant_id).cloned().unwrap_or_default()
    }

    pub async fn export_keys_encrypted(
        &self,
        tenant_id: Option<&str>,
        password: &str,
    ) -> anyhow::Result<EncryptedKeysBackup> {
        let keys = match tenant_id {
            Some(tid) => self.list_tenant_keys_unredacted(tid).await,
            None => self.list_global_keys_unredacted().await,
        };

        let mut export_keys = keys;
        for k in export_keys.iter_mut() {
            k.rate_limited_until = None;
        }

        let plaintext_json = serde_json::to_string(&export_keys)
            .context("Failed to serialize keys for export")?;

        use argon2::password_hash::rand_core::{OsRng, RngCore};
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        use base64::{prelude::BASE64_STANDARD, Engine};

        let mut salt_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut salt_bytes);
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);

        let mut key_bytes = [0u8; 32];
        let argon2 = argon2::Argon2::default();
        argon2.hash_password_into(password.as_bytes(), &salt_bytes, &mut key_bytes)
            .map_err(|e| anyhow::anyhow!("Argon2 derivation failed: {}", e))?;

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("AES-GCM key initialization failed: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext_bytes = cipher.encrypt(nonce, plaintext_json.as_bytes())
            .map_err(|e| anyhow::anyhow!("AES-GCM encryption failed: {}", e))?;

        let salt = BASE64_STANDARD.encode(salt_bytes);
        let nonce = BASE64_STANDARD.encode(nonce_bytes);
        let ciphertext = BASE64_STANDARD.encode(ciphertext_bytes);

        Ok(EncryptedKeysBackup {
            salt,
            nonce,
            ciphertext,
        })
    }

    pub async fn import_keys_encrypted(
        &self,
        tenant_id: Option<&str>,
        password: &str,
        backup: EncryptedKeysBackup,
    ) -> anyhow::Result<usize> {
        use base64::{prelude::BASE64_STANDARD, Engine};
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };

        let salt_bytes = BASE64_STANDARD.decode(&backup.salt)
            .context("Invalid base64 in salt")?;
        let nonce_bytes = BASE64_STANDARD.decode(&backup.nonce)
            .context("Invalid base64 in nonce")?;
        let ciphertext_bytes = BASE64_STANDARD.decode(&backup.ciphertext)
            .context("Invalid base64 in ciphertext")?;

        let mut key_bytes = [0u8; 32];
        let argon2 = argon2::Argon2::default();
        argon2.hash_password_into(password.as_bytes(), &salt_bytes, &mut key_bytes)
            .map_err(|e| anyhow::anyhow!("Argon2 derivation failed: {}", e))?;

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| anyhow::anyhow!("AES-GCM key initialization failed: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext_bytes = cipher.decrypt(nonce, ciphertext_bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("Decryption failed: check if the password is correct"))?;

        let plaintext_json = String::from_utf8(plaintext_bytes)
            .context("Decrypted data is not valid UTF-8")?;

        let imported_keys: Vec<ApiKeyEntry> = serde_json::from_str(&plaintext_json)
            .context("Decrypted JSON is not a valid list of API keys")?;

        let count = imported_keys.len();

        for key in imported_keys {
            match tenant_id {
                Some(tid) => {
                    self.add_tenant_key(tid, key).await?;
                }
                None => {
                    self.add_global_key(key).await?;
                }
            }
        }

        Ok(count)
    }

    fn make_pcb(&self, pid: &str, json: &str) -> crate::pcb::PCB {
        let mut pcb = crate::pcb::PCB::new(pid.to_string(), 0, json.to_string());
        pcb.pid = pid.to_string();
        pcb
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EncryptedKeysBackup {
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
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
        async fn save_voice_fingerprint(
            &self,
            _tenant_id: &str,
            _fingerprint: &[f32],
            _threshold: f32,
        ) -> Result<()> {
            Ok(())
        }
        async fn get_voice_fingerprint(&self, _tenant_id: &str) -> Result<Option<(Vec<f32>, f32)>> {
            Ok(None)
        }
        async fn delete_voice_fingerprint(&self, _tenant_id: &str) -> Result<()> {
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
                active_models: None,
                is_free_tier: false,
            };
            pool.add_global_key(entry).await?;
        }

        // Mark keys 1 and 2 as rate limited
        let future = Utc::now() + chrono::Duration::hours(1);
        pool.mark_rate_limited("key-1", future).await;
        pool.mark_rate_limited("key-2", future).await;

        // Only key-3 should be returned
        let key = pool
            .get_available_key("anthropic", "any-model", "tenant-x")
            .await;
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
            active_models: None,
            is_free_tier: false,
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
                active_models: None,
                is_free_tier: false,
            },
        )
        .await?;

        let key = pool
            .get_available_key("openai", "any-model", "tenant-a")
            .await;
        assert!(key.is_some());
        assert_eq!(
            key.map(|k| k.key_id),
            Some("tenant-a-1".to_string()),
            "Tenant key should take priority"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_groq_prefix_mismatch_resolved() -> anyhow::Result<()> {
        // Reproduces the production bug: catalog model_id = "groq/llama-3.3-70b-versatile"
        // but Groq's /v1/models returns "llama-3.3-70b-versatile" (no prefix).
        // The stored active_models should still match via model_id_matches().
        let pool = KeyPool::new(Arc::new(NoopPersistor));

        pool.add_global_key(ApiKeyEntry {
            key_id: "groq-key".to_string(),
            provider: "groq".to_string(),
            api_key: "gsk_test".to_string(),
            api_url: None,
            label: None,
            is_active: true,
            rate_limited_until: None,
            // Groq API returns bare IDs without the "groq/" prefix
            active_models: Some(vec![
                "llama-3.3-70b-versatile".to_string(),
                "mixtral-8x7b-32768".to_string(),
            ]),
            is_free_tier: false,
        })
        .await?;

        // Catalog uses "groq/llama-3.3-70b-versatile" — must still find a key
        assert!(
            pool.has_key_for_model("groq", "groq/llama-3.3-70b-versatile")
                .await,
            "Prefixed catalog model_id should match bare stored active_model"
        );
        assert!(
            pool.get_available_key("groq", "groq/llama-3.3-70b-versatile", "default")
                .await
                .is_some(),
            "get_available_key should resolve the key despite prefix mismatch"
        );
        // A model NOT in the list should still be rejected
        assert!(
            !pool.has_key_for_model("groq", "groq/unknown-model").await,
            "Model not in active_models should be rejected"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_export_import_encrypted() -> anyhow::Result<()> {
        let pool = KeyPool::new(Arc::new(NoopPersistor));

        pool.add_global_key(ApiKeyEntry {
            key_id: "global-1".to_string(),
            provider: "openai".to_string(),
            api_key: "sk-global".to_string(),
            api_url: None,
            label: Some("Global Key".to_string()),
            is_active: true,
            rate_limited_until: Some(Utc::now() + chrono::Duration::hours(1)),
            active_models: None,
            is_free_tier: false,
        })
        .await?;

        let password = "super_secure_password";
        let backup = pool.export_keys_encrypted(None, password).await?;

        // Verify the export format
        assert!(!backup.salt.is_empty());
        assert!(!backup.nonce.is_empty());
        assert!(!backup.ciphertext.is_empty());

        // Create a new empty pool
        let pool2 = KeyPool::new(Arc::new(NoopPersistor));

        // Decryption with incorrect password should fail
        let bad_import = pool2.import_keys_encrypted(None, "wrong_password", backup.clone()).await;
        assert!(bad_import.is_err(), "Decryption with wrong password must fail");

        // Decryption with correct password should succeed
        let count = pool2.import_keys_encrypted(None, password, backup).await?;
        assert_eq!(count, 1);

        // Verify imported keys
        let imported = pool2.list_global_keys_unredacted().await;
        assert_eq!(imported.len(), 1);
        let key = &imported[0];
        assert_eq!(key.key_id, "global-1");
        assert_eq!(key.api_key, "sk-global");
        // State fields like cooldowns/rate limit until should be stripped (None)
        assert!(key.rate_limited_until.is_none(), "Rate limited until should be stripped in export");

        Ok(())
    }
}
