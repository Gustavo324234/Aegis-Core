pub mod catalog;
pub mod discovery;
pub mod key_pool;
pub mod modules;
pub mod rate_tracker;
pub mod siren;
pub mod syncer;

pub use siren::{SirenEngine, SirenRouter};

use crate::chal::SystemError;
use crate::pcb::{RoutingPolicy, TaskType, PCB};
use crate::scheduler::ModelPreference;
pub use catalog::{ModelCatalog, ModelEntry, ToolUseSupport};
pub use key_pool::KeyPool;
pub use rate_tracker::{ModelOutcomes, ModelUsageTracker};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// CORE-FIX (B4): TTL of the sticky-routing cache. Within this window, the
/// same (tenant, task_type, model_pref) triple reuses the previous decision
/// to keep conversations on a consistent model. Long enough to span a typical
/// reply turn-around, short enough that price/rate-limit changes catch up.
const STICKY_DECISION_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
    pub key_id: Option<String>,
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

/// Key for the sticky-routing cache. The triple identifies the conversation
/// intent — same tenant, same task class, same hardware preference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StickyKey {
    tenant_id: String,
    task_type: TaskType,
    model_pref: ModelPreference,
}

/// CORE-322: pid of the synthetic PCB that stores the tracker's durable
/// state (same persistence channel the KeyPool uses for API keys).
const TRACKER_STATE_PID: &str = "router_tracker:global";

/// CORE-322: routing telemetry aggregate for the `/api/router/stats`
/// endpoint. Contains NO secrets — model/provider ids and counters only.
#[derive(Debug, serde::Serialize)]
pub struct RouterStats {
    pub outcomes: Vec<ModelOutcomeStat>,
    pub observed_latency_ms: HashMap<String, u32>,
    pub open_provider_circuits: HashMap<String, u64>,
    pub open_model_circuits: HashMap<String, u64>,
    pub sticky_decisions: Vec<StickyDecisionStat>,
}

#[derive(Debug, serde::Serialize)]
pub struct ModelOutcomeStat {
    pub model_id: String,
    pub successes: u64,
    pub failures: u64,
    pub success_rate: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct StickyDecisionStat {
    pub tenant_id: String,
    pub task_type: String,
    pub model_pref: String,
    pub model_id: String,
    pub provider: String,
    pub age_secs: u64,
}

pub struct CognitiveRouter {
    catalog: Arc<ModelCatalog>,
    key_pool: Arc<KeyPool>,
    tracker: Arc<ModelUsageTracker>,
    /// CORE-FIX (B4): cache of the last routing decision per conversation
    /// intent. Keeps consecutive turns on the same model so the persona/style
    /// stays consistent. Invalidated on failure (see `invalidate_sticky`).
    sticky: Arc<RwLock<HashMap<StickyKey, (Instant, RoutingDecision)>>>,
    pub modules: Arc<RwLock<HashMap<String, modules::ModuleManifest>>>,
}

impl CognitiveRouter {
    pub fn new(catalog: Arc<ModelCatalog>, key_pool: Arc<KeyPool>) -> Self {
        Self {
            catalog,
            key_pool,
            tracker: Arc::new(ModelUsageTracker::new()),
            sticky: Arc::new(RwLock::new(HashMap::new())),
            modules: Arc::new(RwLock::new(HashMap::new())),
        }
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

    /// Returns the full (unredacted) entry for a key_id. Used by update handlers
    /// to preserve the existing api_key when the request omits it.
    pub async fn get_raw_key_by_id(
        &self,
        key_id: &str,
        tenant_id: Option<&str>,
    ) -> Option<key_pool::ApiKeyEntry> {
        self.key_pool.get_raw_key_by_id(key_id, tenant_id).await
    }

    /// Dynamic Domain Module loading interface.
    pub async fn load_modules(&self, path: &std::path::Path) -> anyhow::Result<usize> {
        let loaded = modules::load_modules_from_dir(path)?;
        let count = loaded.len();
        let mut registry = self.modules.write().await;
        *registry = loaded;
        Ok(count)
    }

    /// Dynamic Domain Module prompt injection interface.
    pub async fn generate_modules_prompt(
        &self,
        prompt: &str,
        tenant_id: &str,
        session_key: &str,
    ) -> String {
        let registry = self.modules.read().await;
        modules::generate_system_prompt_for_modules(&registry, prompt, tenant_id, session_key)
    }

    pub async fn list_models_for_catalog(&self) -> Vec<ModelEntry> {
        self.catalog.all_entries().await
    }

    pub async fn last_synced(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.catalog.last_synced().await
    }

    pub async fn decide(&self, pcb: &PCB) -> Result<RoutingDecision, SystemError> {
        let task_type = pcb.task_type;
        let model_pref = pcb.model_pref;
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        // CORE-FIX (B4): If we have a recent sticky decision for this exact
        // intent and no model_override was forced, reuse it. Keeps a
        // conversation on the same model across turns instead of letting
        // the CMR flip between Claude / GPT / Gemini mid-chat.
        if pcb.model_override.is_none() {
            let sticky_key = StickyKey {
                tenant_id: tenant_id.to_string(),
                task_type,
                model_pref,
            };
            let cached = {
                let sticky = self.sticky.read().await;
                sticky.get(&sticky_key).cloned()
            };
            if let Some((stamped_at, decision)) = cached {
                if stamped_at.elapsed() < STICKY_DECISION_TTL {
                    // Re-validate health before reusing — provider circuit AND
                    // per-model circuit (CORE-319: a model returning 401s or
                    // empty streams must not stay pinned for the whole TTL).
                    if !self
                        .tracker
                        .is_provider_circuit_open(&decision.provider)
                        .await
                        && !self.tracker.is_model_circuit_open(&decision.model_id).await
                    {
                        info!(
                            model = %decision.model_id,
                            age_ms = stamped_at.elapsed().as_millis() as u64,
                            "CMR: sticky-routing cache hit — reusing previous decision"
                        );
                        return Ok(decision);
                    }
                }
                // Expired or unhealthy — drop the entry.
                self.sticky.write().await.remove(&sticky_key);
            }
        }

        // CORE-299: Si el PCB tiene un model_override, bypassear el CMR y resolver directo.
        if let Some(ref model_id) = pcb.model_override {
            if let Some(entry) = self.catalog.find(model_id).await {
                let key = self.resolve_key(&entry, tenant_id).await.ok_or_else(|| {
                    SystemError::HardwareFailure(format!(
                        "No key available for model_override '{}'",
                        model_id
                    ))
                })?;
                return Ok(RoutingDecision {
                    model_id: bare_model_id(&entry.model_id, &entry.provider),
                    provider: entry.provider.clone(),
                    api_url: normalize_chat_api_url(
                        &entry.provider,
                        key.api_url.clone().unwrap_or_else(|| entry_api_url(&entry)),
                    ),
                    api_key: key.api_key.clone(),
                    key_id: Some(key.key_id.clone()),
                    fallback_chain: vec![],
                });
            }
            warn!(
                "model_override '{}' not found in catalog, falling back to CMR",
                model_id
            );
        }

        // Step 1: Get candidates from catalog
        let all_candidates = self.catalog.get_candidates(task_type).await;

        // CORE-305: Detect trivial/light tasks and override preference to LocalOnly
        // to bypass cloud enclaves if healthy local models are available.
        let mut model_pref = model_pref;
        if crate::chal::autocorrect::is_light_task(&pcb.memory_pointers.l1_instruction, task_type) {
            let mut has_healthy_local = false;
            for entry in &all_candidates {
                if entry.is_local
                    && !self.tracker.is_provider_circuit_open(&entry.provider).await
                    && !self.model_circuit_open_merged(entry).await
                {
                    has_healthy_local = true;
                    break;
                }
            }
            if has_healthy_local {
                info!("CMR: Light task detected — overriding model preference to LocalOnly");
                model_pref = ModelPreference::LocalOnly;
            }
        }

        // Step 2: Filter by model preference.
        // CORE-FIX: If LocalOnly produces no candidates (Ollama not running, no local
        // model registered, etc.), fall back to HybridSmart instead of hard-failing.
        // Hard-failing leaves the user staring at "no model available" even when cloud
        // keys are configured — silently misleading. The downgrade is logged at WARN.
        let (filtered, effective_pref): (Vec<ModelEntry>, ModelPreference) = {
            let primary: Vec<ModelEntry> = all_candidates
                .iter()
                .filter(|e| match model_pref {
                    ModelPreference::LocalOnly => e.is_local,
                    ModelPreference::CloudOnly => !e.is_local,
                    ModelPreference::HybridSmart => true,
                })
                .cloned()
                .collect();

            if primary.is_empty() && matches!(model_pref, ModelPreference::LocalOnly) {
                warn!(
                    "LocalOnly preference has no candidates for task_type={:?} \
                     — falling back to HybridSmart so the request can still complete",
                    task_type
                );
                (all_candidates, ModelPreference::HybridSmart)
            } else {
                (primary, model_pref)
            }
        };

        if filtered.is_empty() {
            return Err(SystemError::ModelNotFound(format!(
                "No models available for task_type={:?} with model_pref={:?}",
                task_type, effective_pref
            )));
        }

        // Step 3: Filter by key availability AND circuit breaker.
        // CORE-FIX (B3): if a provider has 3+ failures in the last 30s, skip
        // all its models — better to fall through to another provider than
        // keep banging on the broken one.
        let mut available: Vec<ModelEntry> = Vec::new();
        let mut skipped_by_breaker: Vec<String> = Vec::new();
        // CORE-FIX: track which providers we skipped so we can surface a
        // "retry in Ns" countdown if everything ends up gated.
        let mut providers_in_cooldown: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for entry in filtered {
            let has_key = self
                .key_pool
                .has_key_for_model(&entry.provider, &entry.model_id)
                .await
                || entry.is_local;
            if !has_key {
                continue;
            }
            // CORE-FIX (F): proactively exclude paid-only models when the
            // tenant has ONLY a free-tier key for the provider. Without this
            // the router happily ranks gemini-2.5-pro #1, calls it, and eats a
            // 429 `limit: 0` every single request (the model isn't on the free
            // plan). has_paid_key tells us whether a paid key exists; if not
            // and the model isn't free-tier-eligible, skip it up front so a
            // free-tier-eligible sibling (gemini-2.5-flash) wins instead.
            if !entry.free_tier_eligible && !entry.is_local {
                let has_paid = self
                    .key_pool
                    .has_paid_key(&entry.provider, &entry.model_id)
                    .await;
                if !has_paid {
                    skipped_by_breaker.push(entry.model_id.clone());
                    continue;
                }
            }
            if self.tracker.is_provider_circuit_open(&entry.provider).await {
                // CORE-324: half-open — once the provider has been quiet for
                // a while, let one candidate through as a canary instead of
                // blocking everything until the failure window slides.
                if !self
                    .tracker
                    .provider_circuit_allows_probe(&entry.provider)
                    .await
                {
                    skipped_by_breaker.push(entry.model_id.clone());
                    providers_in_cooldown.insert(entry.provider.clone());
                    continue;
                }
                info!(
                    provider = %entry.provider,
                    model = %entry.model_id,
                    "CMR: half-open probe — letting one candidate through an open circuit"
                );
            }
            // CORE-FIX (D): per-model circuit. The provider may be perfectly
            // healthy overall (e.g. ollama_cloud responds for gpt-oss:120b)
            // but a specific model on it keeps returning HTTP 200 with zero
            // content (cogito-2.1:671b from the smoke test). Skip just that
            // model so the router promotes a sibling instead of falling
            // through to a different provider. CORE-319: queried under BOTH
            // id spaces — the chal layer records under the bare API id.
            if self.model_circuit_open_merged(&entry).await {
                skipped_by_breaker.push(entry.model_id.clone());
                continue;
            }
            available.push(entry);
        }

        if !skipped_by_breaker.is_empty() {
            warn!(
                skipped = ?skipped_by_breaker,
                "CMR: skipped models whose provider has open circuit (3+ recent failures)"
            );
        }

        if available.is_empty() {
            // CORE-FIX (inspired by OpenClaw's per-profile cooldown tracking):
            // if everything got gated, surface the soonest cooldown expiry so
            // the WS layer can render a "retry in Ns" countdown instead of a
            // generic error.
            let mut soonest: Option<u64> = None;
            for provider in &providers_in_cooldown {
                if let Some(secs) = self
                    .tracker
                    .provider_cooldown_remaining_secs(provider)
                    .await
                {
                    soonest = Some(soonest.map_or(secs, |cur| cur.min(secs)));
                }
            }

            let msg = match soonest {
                Some(secs) => format!(
                    "No available keys for any candidate model. Provider cooldown active — retry in {}s.",
                    secs
                ),
                None => "No available keys for any candidate model.".to_string(),
            };
            return Err(SystemError::HardwareFailure(msg));
        }

        // Step 4: Compute global max cost and latency for fair normalization,
        // then compute per-candidate rate-limit capacity factors for free-tier models.
        // Two passes to avoid the order-dependent bias of incremental normalization.
        let max_cost = available
            .iter()
            .map(|e| task_weighted_cost(e, task_type))
            .fold(0.0_f64, f64::max);
        let max_latency = {
            let mut ml = 0.0_f64;
            for e in &available {
                let obs = self.observed_latency_merged(e).await;
                let lat = obs.unwrap_or(e.avg_latency_ms.unwrap_or(2000)) as f64;
                if lat > ml {
                    ml = lat;
                }
            }
            ml
        };

        let history_chars: usize = pcb
            .message_history
            .iter()
            .map(|m| m.content.as_ref().map(|s| s.len()).unwrap_or(0))
            .sum();
        let inlined_chars: usize = pcb.inlined_context.values().map(|v| v.len()).sum();
        let total_chars = pcb.memory_pointers.l1_instruction.len() + history_chars + inlined_chars;
        let estimated_tokens = (total_chars / 4).max(1);

        // For each candidate: if no paid key exists, apply free-tier capacity factor.
        // Models at capacity (factor == 0.0) are hard-excluded; approaching limit
        // get a proportional score penalty so the router prefers models with headroom.
        let mut scored: Vec<(f64, ModelEntry)> = Vec::with_capacity(available.len());
        for entry in available {
            let has_paid = self
                .key_pool
                .has_paid_key(&entry.provider, &entry.model_id)
                .await
                || entry.is_local;

            let capacity = if has_paid {
                1.0_f64 // Paid key → no meaningful rate limit
            } else {
                // CORE-320: each free key carries its own provider-side quota
                // and the KeyPool rotates round-robin between them, so N usable
                // free keys give N× the model's effective RPM/RPD. Without the
                // scaling, a tenant with 3 Gemini free keys was self-throttled
                // to a third of its real capacity.
                let free_keys = self
                    .key_pool
                    .count_available_free_keys(&entry.provider, &entry.model_id, tenant_id)
                    .await
                    .max(1) as u32;
                self.tracker
                    .capacity_factor(
                        &entry.model_id,
                        entry.free_tier_rpm.map(|v| v.saturating_mul(free_keys)),
                        entry.free_tier_rpd.map(|v| v.saturating_mul(free_keys)),
                    )
                    .await
            };

            if capacity == 0.0 {
                // Hard-exclude: free-tier exhausted for this model
                continue;
            }

            let ctx = ScoreCtx {
                prompt: &pcb.memory_pointers.l1_instruction,
                max_cost,
                max_latency,
                observed_latency: self.observed_latency_merged(&entry).await,
                recent_errors: self.recent_errors_merged(&entry).await,
                estimated_tokens,
                routing_policy: pcb.routing_policy,
                outcomes: self.outcomes_merged(&entry).await,
            };
            let base = self.compute_score(&entry, task_type, &ctx);
            // Soft penalty: multiply by sqrt(capacity) so a model at 50% headroom
            // scores ~70% of its base, still competitive but deprioritised.
            let mut adjusted = base * capacity.sqrt();
            // CORE-FIX (B): multi-step, token-heavy task types (Planning, Code)
            // burn a free tier's per-minute budget fast — the ReAct loop fires
            // several calls in quick succession (the smoke test blew Groq's 12k
            // TPM this way). Deprioritise free-tier models for these so a
            // paid/local alternative wins when one exists. `has_paid` already
            // folds in is_local, so !has_paid means a genuine free-tier key; when
            // ONLY free keys exist every candidate gets the same factor and the
            // best free model is still chosen.
            if !has_paid && matches!(task_type, TaskType::Planning | TaskType::Code) {
                adjusted *= 0.6;
            }
            scored.push((adjusted, entry));
        }

        if scored.is_empty() {
            return Err(SystemError::HardwareFailure(
                "All candidate models are rate-limited or have no available keys".to_string(),
            ));
        }

        // Sort by adjusted score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Step 5: Resolve key for the primary candidate and record the request.
        let (_, primary) = &scored[0];
        let primary_key = self.resolve_key(primary, tenant_id).await.ok_or_else(|| {
            SystemError::HardwareFailure(format!(
                "Key for provider '{}' became unavailable",
                primary.provider
            ))
        })?;

        // Record this request only if the resolved key is free-tier.
        if primary_key.is_free_tier {
            self.tracker.record_request(&primary.model_id).await;
        }

        // CORE-FIX (C): each fallback must use a key resolved for ITS provider.
        // The previous version reused `primary_key.api_key` for every fallback,
        // which produced 401 Unauthorized any time the fallback was on a
        // different provider than the primary (e.g. OpenAI primary → Anthropic
        // fallback was sending the OpenAI token to Anthropic's endpoint).
        // Skip fallbacks for which no key is available rather than including
        // a guaranteed-401 entry that would just waste a round trip.
        // CORE-FIX (A): prefer a fallback on a DIFFERENT provider than the
        // primary. A provider-wide outage (Gemini returning empty streams, a
        // Groq TPM 429 storm) otherwise takes down the whole chain, because the
        // next-best models are usually siblings on the same provider. Stable
        // partition puts different-provider candidates first while preserving
        // score order within each group; we then take the first 2 with a key.
        let primary_provider = primary.provider.clone();
        let mut fallback_candidates: Vec<&ModelEntry> =
            scored.iter().skip(1).map(|(_, e)| e).collect();
        fallback_candidates.sort_by_key(|e| e.provider == primary_provider);

        let mut fallback_chain: Vec<FallbackDecision> = Vec::new();
        for entry in fallback_candidates {
            if fallback_chain.len() >= 2 {
                break;
            }
            let fb_key = match self.resolve_key(entry, tenant_id).await {
                Some(k) => k,
                None => {
                    warn!(
                        provider = %entry.provider,
                        model = %entry.model_id,
                        "skipping fallback: no api key available for provider"
                    );
                    continue;
                }
            };
            let api_url = normalize_chat_api_url(
                &entry.provider,
                fb_key
                    .api_url
                    .clone()
                    .unwrap_or_else(|| entry_api_url(entry)),
            );
            fallback_chain.push(FallbackDecision {
                model_id: bare_model_id(&entry.model_id, &entry.provider),
                provider: entry.provider.clone(),
                api_url,
                api_key: fb_key.api_key.clone(),
            });
        }

        let api_model_id = bare_model_id(&primary.model_id, &primary.provider);

        // CORE-FIX (D1): Structured log with the top-3 scoring breakdown so we
        // can answer "why did the router pick X over Y?" after the fact. Without
        // this the only signal is the chosen model — which makes the CMR a
        // black box when its decisions surprise us.
        let top_breakdown: Vec<(String, f64)> = scored
            .iter()
            .take(3)
            .map(|(score, e)| (e.model_id.clone(), (*score * 1000.0).round() / 1000.0))
            .collect();
        let fallback_ids: Vec<String> = fallback_chain.iter().map(|f| f.model_id.clone()).collect();
        info!(
            catalog_id = %primary.model_id,
            api_model_id = %api_model_id,
            provider = %primary.provider,
            task_type = ?task_type,
            model_pref = ?effective_pref,
            tenant = %tenant_id,
            candidates_considered = scored.len(),
            top3 = ?top_breakdown,
            fallback_chain = ?fallback_ids,
            "CognitiveRouter: routing decision"
        );

        let decision = RoutingDecision {
            model_id: api_model_id,
            provider: primary.provider.clone(),
            api_url: normalize_chat_api_url(
                &primary.provider,
                primary_key
                    .api_url
                    .clone()
                    .unwrap_or_else(|| entry_api_url(primary)),
            ),
            api_key: primary_key.api_key.clone(),
            key_id: Some(primary_key.key_id.clone()),
            fallback_chain,
        };

        // CORE-FIX (B4): cache this decision for the next turn from the same tenant.
        // Skipped for model_override flows (those bypass the CMR entirely).
        if pcb.model_override.is_none() {
            // CORE-319: key by the ORIGINAL request preference — the lookup at
            // the top of decide() uses pcb.model_pref, so inserting under
            // effective_pref (light-task override, LocalOnly downgrade) made
            // the cache unable to ever hit for exactly those flows.
            let sticky_key = StickyKey {
                tenant_id: tenant_id.to_string(),
                task_type,
                model_pref: pcb.model_pref,
            };
            let mut sticky = self.sticky.write().await;
            // CORE-319: sweep expired entries so ephemeral tenants don't
            // accumulate in the map forever.
            sticky.retain(|_, (stamped_at, _)| stamped_at.elapsed() < STICKY_DECISION_TTL);
            sticky.insert(sticky_key, (Instant::now(), decision.clone()));
        }

        Ok(decision)
    }

    /// CORE-FIX (B4): Drop the sticky cache entry for this tenant so the next
    /// `decide()` call re-evaluates from scratch. Call this when the previously
    /// chosen model fails so we don't pin the conversation to a broken model.
    pub async fn invalidate_sticky(&self, tenant_id: &str) {
        let mut sticky = self.sticky.write().await;
        sticky.retain(|k, _| k.tenant_id != tenant_id);
    }

    /// CORE-319 (test-only): whether a sticky entry exists for this intent.
    /// Lets regression tests assert the cache is keyed by the ORIGINAL
    /// request preference, not the effective (possibly overridden) one.
    #[cfg(test)]
    pub(crate) async fn sticky_contains(
        &self,
        tenant_id: &str,
        task_type: TaskType,
        model_pref: ModelPreference,
    ) -> bool {
        let key = StickyKey {
            tenant_id: tenant_id.to_string(),
            task_type,
            model_pref,
        };
        self.sticky.read().await.contains_key(&key)
    }

    pub fn tracker_ref(&self) -> &Arc<ModelUsageTracker> {
        &self.tracker
    }

    /// CORE-322: restore the tracker's durable state (free-tier daily
    /// counters + reliability outcomes) from disk and keep persisting it in
    /// the background. Without this a restart silently reset the RPD
    /// counters — the router could blow a free tier's daily quota right
    /// after boot — and wiped the observed-reliability signal.
    pub fn spawn_tracker_persistence(
        &self,
        persistor: Arc<dyn crate::scheduler::persistence::StatePersistor>,
    ) {
        let tracker = Arc::clone(&self.tracker);
        tokio::spawn(async move {
            // Restore once at boot.
            match persistor.load_all_pcbs().await {
                Ok(pcbs) => {
                    if let Some(pcb) = pcbs.into_iter().find(|p| p.pid == TRACKER_STATE_PID) {
                        match serde_json::from_str::<rate_tracker::TrackerSnapshot>(
                            &pcb.memory_pointers.l1_instruction,
                        ) {
                            Ok(snap) => {
                                tracker.restore(snap).await;
                                info!("CORE-322: router tracker state restored from disk");
                            }
                            Err(e) => warn!(
                                error = %e,
                                "CORE-322: persisted tracker state is corrupt — starting fresh"
                            ),
                        }
                    }
                }
                Err(e) => warn!(error = %e, "CORE-322: could not load tracker state"),
            }

            // Persist on a fixed cadence, only when something changed.
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                if !tracker.take_dirty() {
                    continue;
                }
                let snap = tracker.snapshot().await;
                match serde_json::to_string(&snap) {
                    Ok(json) => {
                        let mut pcb = crate::pcb::PCB::new(TRACKER_STATE_PID.to_string(), 0, json);
                        pcb.pid = TRACKER_STATE_PID.to_string();
                        if let Err(e) = persistor.save_pcb(&pcb).await {
                            warn!(error = %e, "CORE-322: failed to persist tracker state");
                        }
                    }
                    Err(e) => warn!(error = %e, "CORE-322: failed to serialize tracker state"),
                }
            }
        });
    }

    /// CORE-322: telemetry aggregate for `/api/router/stats`. Secrets-free.
    pub async fn stats(&self) -> RouterStats {
        let mut outcomes: Vec<ModelOutcomeStat> = self
            .tracker
            .all_outcomes()
            .await
            .into_iter()
            .map(|(model_id, o)| ModelOutcomeStat {
                success_rate: o.success_rate(),
                successes: o.successes,
                failures: o.failures,
                model_id,
            })
            .collect();
        outcomes.sort_by(|a, b| a.model_id.cmp(&b.model_id));

        let sticky_decisions: Vec<StickyDecisionStat> = {
            let sticky = self.sticky.read().await;
            sticky
                .iter()
                .map(|(k, (stamped_at, d))| StickyDecisionStat {
                    tenant_id: k.tenant_id.clone(),
                    task_type: format!("{:?}", k.task_type),
                    model_pref: format!("{:?}", k.model_pref),
                    model_id: d.model_id.clone(),
                    provider: d.provider.clone(),
                    age_secs: stamped_at.elapsed().as_secs(),
                })
                .collect()
        };

        RouterStats {
            outcomes,
            observed_latency_ms: self.tracker.all_observed_latencies().await,
            open_provider_circuits: self.tracker.open_provider_circuits().await,
            open_model_circuits: self.tracker.open_model_circuits().await,
            sticky_decisions,
        }
    }

    // ─── CORE-319: dual id-space tracker queries ─────────────────────────
    //
    // The chal layer records runtime signals (latency, errors, outcomes,
    // per-model circuits) under the API model id it actually called —
    // `RoutingDecision::model_id`, i.e. WITHOUT the "provider/" prefix —
    // while the catalog and decide() work with the prefixed catalog id.
    // For every prefixed model (groq/…, anthropic/…, openai/…) the two ids
    // differ and a single-id query silently misses the recorded signals,
    // leaving the router blind to its own telemetry. These helpers query
    // both id spaces and merge.

    async fn observed_latency_merged(&self, entry: &ModelEntry) -> Option<u32> {
        match self.tracker.observed_latency_ms(&entry.model_id).await {
            Some(v) => Some(v),
            None => {
                let api_id = bare_model_id(&entry.model_id, &entry.provider);
                if api_id != entry.model_id {
                    self.tracker.observed_latency_ms(&api_id).await
                } else {
                    None
                }
            }
        }
    }

    async fn recent_errors_merged(&self, entry: &ModelEntry) -> u32 {
        let mut errors = self.tracker.recent_errors(&entry.model_id).await;
        let api_id = bare_model_id(&entry.model_id, &entry.provider);
        if api_id != entry.model_id {
            errors += self.tracker.recent_errors(&api_id).await;
        }
        errors
    }

    async fn model_circuit_open_merged(&self, entry: &ModelEntry) -> bool {
        if self.tracker.is_model_circuit_open(&entry.model_id).await {
            return true;
        }
        let api_id = bare_model_id(&entry.model_id, &entry.provider);
        api_id != entry.model_id && self.tracker.is_model_circuit_open(&api_id).await
    }

    async fn outcomes_merged(&self, entry: &ModelEntry) -> ModelOutcomes {
        let mut merged = self
            .tracker
            .outcomes_for(&entry.model_id)
            .await
            .unwrap_or_default();
        let api_id = bare_model_id(&entry.model_id, &entry.provider);
        if api_id != entry.model_id {
            if let Some(o) = self.tracker.outcomes_for(&api_id).await {
                merged.successes += o.successes;
                merged.failures += o.failures;
            }
        }
        merged
    }

    pub fn catalog_ref(&self) -> Arc<ModelCatalog> {
        self.catalog.clone()
    }

    /// CORE-FIX: expose the underlying key pool so the chal layer can request
    /// a fresh key for the same (provider, model) after one gets marked as
    /// rate-limited. Used to rotate Gemini keys when the first one returns
    /// RESOURCE_EXHAUSTED instead of immediately failing over to a different
    /// model.
    pub fn key_pool_ref(&self) -> Arc<KeyPool> {
        Arc::clone(&self.key_pool)
    }

    fn compute_score(&self, entry: &ModelEntry, task_type: TaskType, ctx: &ScoreCtx<'_>) -> f64 {
        let (prompt, max_cost, max_latency, observed_latency, recent_errors) = (
            ctx.prompt,
            ctx.max_cost,
            ctx.max_latency,
            ctx.observed_latency,
            ctx.recent_errors,
        );
        // ── 1. Quality (40%) ─────────────────────────────────────────
        let base_quality = entry.score_for(task_type) as f64 / 5.0;
        let content_boost = detect_content_type(prompt, task_type);
        let quality = (base_quality * (1.0 + content_boost)).min(1.0);

        // ── 2. Cost (25%) ────────────────────────────────────────────
        // CORE-324: task-weighted — long-output tasks amplify output cost.
        let total_cost = task_weighted_cost(entry, task_type);
        let cost_inv = if max_cost > 0.0 {
            1.0 - (total_cost / max_cost)
        } else {
            1.0
        };

        // ── 3. Speed (20%) ───────────────────────────────────────────
        let effective_latency =
            observed_latency.unwrap_or(entry.avg_latency_ms.unwrap_or(2000)) as f64;
        let speed_inv = if max_latency > 0.0 {
            1.0 - (effective_latency / max_latency).min(1.0)
        } else {
            1.0
        };

        // ── 4. Context fit (15%) ─────────────────────────────────────
        let estimated_tokens = ctx.estimated_tokens;
        let context_fit = if entry.context_window as usize > estimated_tokens * 4 {
            1.0
        } else if entry.context_window as usize > estimated_tokens * 2 {
            0.7
        } else if entry.context_window as usize > estimated_tokens {
            0.3
        } else {
            0.0
        };

        // ── 5. Error penalty ─────────────────────────────────────────
        let error_penalty = (recent_errors as f64 * 0.10).min(0.30);

        // ── 6. Oversize penalty for chat-trivial prompts ─────────────
        // CORE-FIX: for short, conversational prompts, route to a fast
        // model — not the most powerful one. Without this penalty the
        // scorer happily picked cogito-2.1:671b or deepseek-v3.1:671b
        // for "hola" because their cost_inv was 1.0 (free via
        // ollama_cloud) and their quality was 5/5.
        //
        // A model is "oversize" when its id advertises ≥120B parameters
        // (heuristic: contains "671b" / "405b" / "120b" / "180b" /
        // "236b" etc.) AND the task is plain Chat AND the prompt is
        // short (< 400 chars). In that case we knock the score down
        // 30% — the lighter Gemini/Claude Flash tier wins instead.
        let oversize_penalty = {
            let lower_id = entry.model_id.to_lowercase();
            let is_giant = ["671b", "405b", "236b", "180b", "120b", "70b"]
                .iter()
                .any(|tag| lower_id.contains(tag));
            let is_chat_trivial = matches!(task_type, TaskType::Chat) && prompt.len() < 400;
            if is_giant && is_chat_trivial {
                0.30
            } else {
                0.0
            }
        };

        // ── 7. Policy and Task-aware weights ──────────────────────────
        let (w_quality, w_cost, w_speed, w_fit) = match ctx.routing_policy {
            RoutingPolicy::CostOptimized => (0.15, 0.60, 0.10, 0.15),
            RoutingPolicy::QualityOptimized => (0.70, 0.05, 0.10, 0.15),
            RoutingPolicy::LatencyOptimized => (0.15, 0.10, 0.60, 0.15),
            RoutingPolicy::Balanced => match task_type {
                TaskType::Chat => (0.30, 0.20, 0.35, 0.15),
                TaskType::Code | TaskType::Planning => (0.55, 0.15, 0.15, 0.15),
                TaskType::Analysis => (0.55, 0.15, 0.15, 0.15),
                _ => (0.40, 0.25, 0.20, 0.15),
            },
        };

        let raw =
            quality * w_quality + cost_inv * w_cost + speed_inv * w_speed + context_fit * w_fit;

        // CORE-305: Tiny Model Penalty (Asymmetric Offloading)
        // If a model is local and its ID contains "1b", "2b", or "3b", and either the task
        // is heavy (Coding, Planning, Analysis) or the prompt is complex (>300 chars),
        // apply a 90% score penalty.
        let tiny_penalty_factor = {
            if entry.is_local {
                let lower_id = entry.model_id.to_lowercase();
                let is_tiny = ["1b", "2b", "3b"].iter().any(|tag| lower_id.contains(tag));
                let is_heavy_task = matches!(
                    task_type,
                    TaskType::Code | TaskType::Planning | TaskType::Analysis
                );
                let is_complex_prompt = prompt.chars().count() > 300;
                if is_tiny && (is_heavy_task || is_complex_prompt) {
                    0.10 // 90% penalty -> keeps 10% of score
                } else {
                    1.0
                }
            } else {
                1.0
            }
        };

        // ── 8. Chat vs Subagents model routing adjustment ─────────────
        // "para chat tiene que usar modelos de respaldo y para subagentes los buenos"
        let chat_subagent_factor = {
            let is_chat = matches!(task_type, TaskType::Chat);
            let is_backup = entry.is_local
                || entry.provider == "ollama_cloud"
                || entry.cost_input_per_mtok == 0.0
                || entry.model_id.ends_with(":free");
            // CORE-320: fold in the runtime tool-use probe (CORE-237). A model
            // whose tool_use_support was observed as Degraded has PROVEN it
            // can't drive the ReAct loop, regardless of what the static
            // supports_tools flag claims.
            let effective_tools = entry.supports_tools
                && entry.tool_use_support != crate::router::catalog::ToolUseSupport::Degraded;

            if is_chat {
                if is_backup {
                    2.0 // Boost backup models for Chat so they always win
                } else {
                    0.05 // Heavily penalize premium models for Chat to keep them as last-resort fallbacks
                }
            } else {
                // For subagents/specialist tasks (Coding, Planning, Analysis, etc.)
                if is_backup {
                    if effective_tools {
                        0.5 // Minor penalty for tool-supporting backup models (viable fallbacks)
                    } else {
                        0.01 // Heavy penalty for backup models without tool support (subagents need tools)
                    }
                } else if entry.tool_use_support == crate::router::catalog::ToolUseSupport::Degraded
                {
                    // CORE-320: a premium model that FAILED the live tool probe
                    // is as useless to a subagent as a no-tools backup model.
                    // (Static supports_tools=false premium models keep the
                    // boost — Analysis/Planning don't always need tools and
                    // absence of evidence isn't a proven failure.)
                    0.05
                } else {
                    2.0 // Boost premium/good models for subagents
                }
            }
        };

        // ── 9. CORE-320: observed reliability ─────────────────────────
        // success/failure outcomes were tracked since D2 but never consumed
        // by the scorer. With ≥ 3 samples, scale by 0.5 + 0.5·success_rate:
        // a fully stable model keeps its score, a permanently failing one
        // keeps half — hard exclusion remains the circuit breakers' job.
        // Under 3 samples there is no signal: no data must not mean penalty.
        let reliability_factor = {
            let total = ctx.outcomes.successes + ctx.outcomes.failures;
            match ctx.outcomes.success_rate() {
                Some(rate) if total >= 3 => 0.5 + 0.5 * rate,
                _ => 1.0,
            }
        };

        (raw * (1.0 - error_penalty)
            * (1.0 - oversize_penalty)
            * tiny_penalty_factor
            * chat_subagent_factor
            * reliability_factor)
            .max(0.0)
    }

    /// Busca una entrada en el catálogo por model_id (CORE-237).
    pub async fn catalog_find(&self, model_id: &str) -> Option<ModelEntry> {
        self.catalog.find(model_id).await
    }

    /// Actualiza el estado de tool_use_support de un modelo en el catálogo (CORE-237).
    /// CORE-319: delega al update in-place del catálogo. El patrón anterior
    /// (all_entries + replace_all) perdía entradas agregadas concurrentemente
    /// por el CatalogSyncer y falsificaba `last_synced`.
    pub async fn update_tool_use_support(
        &self,
        model_id: &str,
        support: crate::router::catalog::ToolUseSupport,
    ) {
        if !self.catalog.update_tool_support(model_id, support).await {
            warn!(
                model = %model_id,
                "update_tool_use_support: model not found in catalog"
            );
        }
    }

    pub async fn mark_key_rate_limited(&self, key_id: &str, until: chrono::DateTime<chrono::Utc>) {
        self.key_pool.mark_rate_limited(key_id, until).await;
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
                is_free_tier: false,
            });
        }
        self.key_pool
            .get_available_key(&entry.provider, &entry.model_id, tenant_id)
            .await
    }
}

struct ScoreCtx<'a> {
    prompt: &'a str,
    max_cost: f64,
    max_latency: f64,
    observed_latency: Option<u32>,
    recent_errors: u32,
    estimated_tokens: usize,
    routing_policy: RoutingPolicy,
    /// CORE-320: merged success/failure outcomes (catalog + API id spaces).
    outcomes: ModelOutcomes,
}

/// Canonicalises a provider identifier so downstream matches (catalog lookups,
/// key pool, ToolRegistry, discovery, circuit breaker) never silently disagree
/// on casing, punctuation, or branding aliases.
///
/// Inspired by OpenClaw's `normalizeProviderId` helper. Aegis previously hit
/// real bugs from this: `"gemini"` vs `"google"` matched different code paths,
/// and `"openai"` vs `"OPENAI"` failed sticky-cache lookups silently.
///
/// Always returns lowercase, no punctuation/whitespace, and maps common
/// aliases to a single canonical id:
/// - `"google"`, `"google ai"`, `"googleai"`, `"google-ai-studio"` → `"gemini"`
/// - `"claude"` → `"anthropic"`
/// - `"open-router"`, `"open_router"` → `"openrouter"`
/// - `"grok"` → `"xai"`
/// - `"ollama-cloud"`, `"ollamacloud"` → `"ollama_cloud"`
///
/// Anything else falls through as lowercased-with-stripped-punctuation.
pub fn normalize_provider_id(raw: &str) -> String {
    let lowered = raw.trim().to_lowercase();
    let stripped: String = lowered
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .collect();
    match stripped.as_str() {
        // Gemini family
        "google" | "googleai" | "googleaistudio" | "gemini" | "gemininative" => {
            "gemini".to_string()
        }
        // Anthropic — accept Claude branding
        "anthropic" | "claude" => "anthropic".to_string(),
        // OpenAI
        "openai" | "gpt" => "openai".to_string(),
        // OpenRouter
        "openrouter" => "openrouter".to_string(),
        // xAI (also referred to as Grok)
        "xai" | "grok" => "xai".to_string(),
        // Ollama variants
        "ollama" => "ollama".to_string(),
        "ollamacloud" => "ollama_cloud".to_string(),
        // Mistral
        "mistral" | "mistralai" => "mistral".to_string(),
        // DeepSeek
        "deepseek" => "deepseek".to_string(),
        // Groq / Qwen — no aliases, just normalised casing
        "groq" => "groq".to_string(),
        "qwen" => "qwen".to_string(),
        // Unknown — return the stripped lowercase form so downstream behaves
        // consistently for whatever the user typed.
        _ => stripped,
    }
}

/// CORE-324: blended per-Mtok cost of a model for a given task type.
/// Code/Planning/Creative turns generate several times more output tokens
/// than input, so a flat in+out sum systematically understated expensive-
/// output models (e.g. Opus at $75/Mtok out vs $15 in). Weight output 3×
/// for long-output tasks; everything else keeps the flat sum.
fn task_weighted_cost(entry: &ModelEntry, task: TaskType) -> f64 {
    let out_weight = match task {
        TaskType::Code | TaskType::Planning | TaskType::Creative => 3.0,
        _ => 1.0,
    };
    entry.cost_input_per_mtok + entry.cost_output_per_mtok * out_weight
}

/// Analyses the prompt with lexical signals and returns a boost (0.0–0.30)
/// when the detected content type matches the declared task_type.
fn detect_content_type(prompt: &str, task_type: TaskType) -> f64 {
    let lower = prompt.to_lowercase();

    let code_signals = [
        "```",
        "fn ",
        "def ",
        "function ",
        "import ",
        "class ",
        "let ",
        "const ",
        "var ",
        "=>",
        "{}",
    ];
    let code_score: f64 = code_signals.iter().filter(|s| lower.contains(*s)).count() as f64
        / code_signals.len() as f64;

    let analysis_signals = [
        "analiza",
        "analyze",
        "compare",
        "compara",
        "diferencia",
        "¿por qué",
        "why",
        "explica",
        "explain",
        "cuál es mejor",
    ];
    let analysis_score: f64 = analysis_signals
        .iter()
        .filter(|s| lower.contains(*s))
        .count() as f64
        / analysis_signals.len() as f64;

    let planning_signals = [
        "plan",
        "roadmap",
        "pasos",
        "steps",
        "cómo hacer",
        "how to",
        "estrategia",
        "strategy",
        "prioridad",
    ];
    let planning_score: f64 = planning_signals
        .iter()
        .filter(|s| lower.contains(*s))
        .count() as f64
        / planning_signals.len() as f64;

    let boost = match task_type {
        TaskType::Code => code_score,
        TaskType::Analysis => analysis_score,
        TaskType::Planning => planning_score,
        _ => 0.0,
    };

    (boost * 0.30_f64).min(0.30)
}

/// Returns the model ID as expected by the provider's native API.
/// Strips the leading "provider/" prefix for APIs that don't use it.
/// OpenRouter is the exception — it expects the full "org/model" format.
fn bare_model_id(model_id: &str, provider: &str) -> String {
    match provider {
        // OpenRouter expects the full "org/model" format natively
        "openrouter" => model_id.to_string(),
        // All other providers call their own native API, which expects the
        // model id WITHOUT the synthetic "provider/" prefix. But only strip
        // the prefix when it actually IS the provider id — some real model ids
        // contain a slash that the provider expects verbatim (e.g. groq serves
        // "qwen/qwen3-32b"). Stripping unconditionally turned that into
        // "qwen3-32b" → 404 model_not_found.
        _ => match model_id.split_once('/') {
            Some((prefix, bare))
                if normalize_provider_id(prefix) == normalize_provider_id(provider) =>
            {
                bare.to_string()
            }
            _ => model_id.to_string(),
        },
    }
}

fn entry_api_url(entry: &ModelEntry) -> String {
    // Default API URLs per provider
    match entry.provider.as_str() {
        // Compatible OpenAI — requiere key propia
        "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
        "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
        // CORE-FIX: both Ollama local and cloud use the OpenAI-compat shim at
        // /v1/chat/completions because the cloud driver speaks OpenAI's SSE
        // format. The native /api/chat endpoint returns NDJSON without the
        // `data: ` prefix and our parser silently produces 0 tokens.
        "ollama" => "http://localhost:11434/v1/chat/completions".to_string(),
        "ollama_cloud" => "https://ollama.com/v1/chat/completions".to_string(),
        // Compatible OpenAI via OpenRouter — requiere key de OpenRouter
        "anthropic" | "deepseek" | "mistral" | "qwen" => {
            "https://openrouter.ai/api/v1/chat/completions".to_string()
        }
        // Google Gemini: compatible OpenAI via endpoint beta
        "google" | "gemini" => {
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions".to_string()
        }
        // OpenRouter: hub universal
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        // Fallback seguro
        _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
    }
}

/// CORE-FIX (F): normalise a stored/user-supplied chat URL so it points at the
/// OpenAI-compatible endpoint the CloudProxyDriver actually speaks.
///
/// Tenants who linked Ollama (cloud or local) BEFORE the protocol fix have a
/// stored `api_url` ending in `/api/chat` — Ollama's *native* NDJSON endpoint.
/// The driver sends OpenAI-format requests and parses `data: ` SSE, so hitting
/// `/api/chat` yields 401/400/empty. We rewrite `…/api/chat` →
/// `…/v1/chat/completions` for ollama providers at decision-build time, which
/// is non-destructive (the stored key is left untouched; only the request URL
/// is corrected). No-op for every other provider/URL.
fn normalize_chat_api_url(provider: &str, url: String) -> String {
    let p = normalize_provider_id(provider);
    if p == "ollama" || p == "ollama_cloud" {
        if let Some(base) = url.strip_suffix("/api/chat") {
            return format!("{}/v1/chat/completions", base.trim_end_matches('/'));
        }
    }
    url
}

#[cfg(test)]
mod normalize_url_tests {
    use super::normalize_chat_api_url;

    #[test]
    fn rewrites_ollama_cloud_api_chat() {
        assert_eq!(
            normalize_chat_api_url("ollama_cloud", "https://ollama.com/api/chat".into()),
            "https://ollama.com/v1/chat/completions"
        );
    }

    #[test]
    fn rewrites_ollama_local_api_chat() {
        assert_eq!(
            normalize_chat_api_url("ollama", "http://localhost:11434/api/chat".into()),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn leaves_correct_ollama_url_untouched() {
        let good = "https://ollama.com/v1/chat/completions".to_string();
        assert_eq!(normalize_chat_api_url("ollama_cloud", good.clone()), good);
    }

    #[test]
    fn ignores_non_ollama_providers() {
        // A different provider that happens to end in /api/chat is left alone.
        let url = "https://example.com/api/chat".to_string();
        assert_eq!(normalize_chat_api_url("openai", url.clone()), url);
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
                is_free_tier: false,
            })
            .await?;

        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new(
            "test".to_string(),
            5,
            "Could you please provide a detailed explanation of the cognitive routing architecture in Aegis Core?".to_string(),
        );
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
        pcb.task_type = TaskType::Code;
        pcb.model_pref = ModelPreference::CloudOnly;

        let result = router.decide(&pcb).await;
        assert!(result.is_err(), "Should fail with no keys configured");
        Ok(())
    }

    /// CORE-FIX: provider id aliases must canonicalise so catalog / key pool /
    /// tool registry / discovery all agree on the same string.
    #[test]
    fn test_normalize_provider_id_canonical() {
        // exact already-canonical
        assert_eq!(normalize_provider_id("openai"), "openai");
        assert_eq!(normalize_provider_id("anthropic"), "anthropic");
        assert_eq!(normalize_provider_id("gemini"), "gemini");
    }

    #[test]
    fn test_normalize_provider_id_case_and_punctuation() {
        assert_eq!(normalize_provider_id("OpenAI"), "openai");
        assert_eq!(normalize_provider_id("OPENAI"), "openai");
        assert_eq!(normalize_provider_id("open-router"), "openrouter");
        assert_eq!(normalize_provider_id("open_router"), "openrouter");
        assert_eq!(normalize_provider_id("  ollama-cloud  "), "ollama_cloud");
    }

    #[test]
    fn test_normalize_provider_id_aliases() {
        // Google AI → Gemini family
        assert_eq!(normalize_provider_id("google"), "gemini");
        assert_eq!(normalize_provider_id("Google AI"), "gemini");
        assert_eq!(normalize_provider_id("google-ai-studio"), "gemini");
        // Claude branding → Anthropic
        assert_eq!(normalize_provider_id("claude"), "anthropic");
        // Grok → xAI
        assert_eq!(normalize_provider_id("grok"), "xai");
        // Mistral aliases
        assert_eq!(normalize_provider_id("mistralai"), "mistral");
    }

    #[test]
    fn test_normalize_provider_id_unknown_returns_stripped() {
        // Unknown providers fall through as lowercased + punctuation-stripped
        // so downstream still gets a consistent string.
        assert_eq!(normalize_provider_id("Foo-Bar"), "foobar");
        assert_eq!(normalize_provider_id("CUSTOM"), "custom");
    }

    #[test]
    fn test_bare_model_id_strips_only_matching_provider_prefix() {
        // Synthetic "provider/" prefix is stripped for native APIs.
        assert_eq!(
            bare_model_id("gemini/gemini-2.5-pro", "gemini"),
            "gemini-2.5-pro"
        );
        // No prefix → unchanged.
        assert_eq!(
            bare_model_id("llama-3.3-70b-versatile", "groq"),
            "llama-3.3-70b-versatile"
        );
        // A slash that is part of the real model id (groq serves "qwen/qwen3-32b")
        // must survive — the prefix ("qwen") is NOT the provider ("groq").
        assert_eq!(bare_model_id("qwen/qwen3-32b", "groq"), "qwen/qwen3-32b");
        // OpenRouter keeps the full "org/model" form.
        assert_eq!(
            bare_model_id("anthropic/claude-sonnet-4-6", "openrouter"),
            "anthropic/claude-sonnet-4-6"
        );
    }

    #[tokio::test]
    async fn test_router_edge_override_for_greetings() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        // Configura clave de cloud para que el cloud sea elegible normalmente
        key_pool
            .add_global_key(ApiKeyEntry {
                key_id: "test-cloud".to_string(),
                provider: "anthropic".to_string(),
                api_key: "sk-ant-test".to_string(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
                active_models: None,
                is_free_tier: false,
            })
            .await?;

        let router = CognitiveRouter::new(catalog, key_pool);

        // Light chat prompt (hola) -> should override to LocalOnly, returning a local model
        let mut pcb = PCB::new("test".to_string(), 5, "hola".to_string());
        pcb.task_type = TaskType::Chat;
        pcb.model_pref = ModelPreference::CloudOnly; // normally CloudOnly, but gets overridden!

        let decision = router.decide(&pcb).await?;
        // Should bypass CloudOnly because of light override and return a local model
        assert_eq!(decision.provider, "ollama");
        Ok(())
    }

    #[tokio::test]
    async fn test_tiny_model_penalty_on_heavy_tasks() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let router = CognitiveRouter::new(catalog, key_pool);

        // We want to check that llama-3.2-1b-instruct is penalized on heavy tasks compared to plain chat
        let entry_1b = router
            .catalog_find("meta-llama/llama-3.2-1b-instruct")
            .await
            .unwrap();

        let ctx = ScoreCtx {
            prompt: "Implement this complex coding task in Rust",
            max_cost: 0.0,
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::Balanced,
            outcomes: ModelOutcomes::default(),
        };

        // 1. Scoring for Chat with short prompt (no penalty)
        let ctx_light = ScoreCtx {
            prompt: "hi",
            max_cost: 0.0,
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::Balanced,
            outcomes: ModelOutcomes::default(),
        };
        let score_light = router.compute_score(&entry_1b, TaskType::Chat, &ctx_light);

        // 2. Scoring for Code (heavy task -> 90% penalty)
        let score_heavy_task = router.compute_score(&entry_1b, TaskType::Code, &ctx);

        // 3. Scoring for Chat but complex prompt (>300 chars -> 90% penalty)
        let complex_prompt = "A".repeat(351);
        let ctx_complex = ScoreCtx {
            prompt: &complex_prompt,
            max_cost: 0.0,
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::Balanced,
            outcomes: ModelOutcomes::default(),
        };
        let score_complex_prompt = router.compute_score(&entry_1b, TaskType::Chat, &ctx_complex);

        // Heavy task score and complex prompt score should be dramatically lower (approx 10% or at least < 50%) compared to base quality
        assert!(score_heavy_task < score_light * 0.5);
        assert!(score_complex_prompt < score_light * 0.5);
        Ok(())
    }

    #[tokio::test]
    async fn test_routing_policy_scoring() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let router = CognitiveRouter::new(catalog, key_pool);

        // We'll compare gpt-4o (high cost, high quality) vs gemini-2.5-flash (lower cost, lower quality)
        let expensive = router.catalog_find("openai/gpt-4o").await.unwrap();
        let cheap = router.catalog_find("gemini-2.5-flash").await.unwrap();

        // Under CostOptimized policy, cheap model should score higher.
        // max_cost mirrors decide(): task-weighted over the candidate set.
        let ctx_cost = ScoreCtx {
            prompt: "Write a short poem",
            max_cost: task_weighted_cost(&expensive, TaskType::Code),
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::CostOptimized,
            outcomes: ModelOutcomes::default(),
        };

        let score_exp_cost = router.compute_score(&expensive, TaskType::Code, &ctx_cost);
        let score_cheap_cost = router.compute_score(&cheap, TaskType::Code, &ctx_cost);
        assert!(
            score_cheap_cost > score_exp_cost,
            "Cheap model should win under CostOptimized"
        );

        // Under QualityOptimized policy, expensive (higher quality) model should win
        let ctx_quality = ScoreCtx {
            prompt: "Write a short poem",
            max_cost: task_weighted_cost(&expensive, TaskType::Code),
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::QualityOptimized,
            outcomes: ModelOutcomes::default(),
        };

        let score_exp_qual = router.compute_score(&expensive, TaskType::Code, &ctx_quality);
        let score_cheap_qual = router.compute_score(&cheap, TaskType::Code, &ctx_quality);
        assert!(
            score_exp_qual > score_cheap_qual,
            "Expensive/high quality model should win under QualityOptimized"
        );

        Ok(())
    }

    /// CORE-319: the sticky cache must be keyed by the ORIGINAL request
    /// preference. The light-task override ("hola" + CloudOnly → LocalOnly)
    /// used to insert under the effective pref, so the lookup (which uses
    /// pcb.model_pref) could never hit and every trivial turn re-routed.
    #[tokio::test]
    async fn test_sticky_cache_keyed_by_original_pref() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        key_pool
            .add_global_key(ApiKeyEntry {
                key_id: "test-cloud".to_string(),
                provider: "anthropic".to_string(),
                api_key: "sk-ant-test".to_string(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
                active_models: None,
                is_free_tier: false,
            })
            .await?;
        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new("test".to_string(), 5, "hola".to_string());
        pcb.task_type = TaskType::Chat;
        pcb.model_pref = ModelPreference::CloudOnly;

        let first = router.decide(&pcb).await?;
        assert!(
            router
                .sticky_contains("default", TaskType::Chat, ModelPreference::CloudOnly)
                .await,
            "sticky entry must live under the request's original preference"
        );

        // Second identical turn must reuse the cached decision (same model).
        let second = router.decide(&pcb).await?;
        assert_eq!(first.model_id, second.model_id);
        assert_eq!(first.provider, second.provider);
        Ok(())
    }

    /// CORE-319: a sticky decision whose model tripped the per-model circuit
    /// (e.g. hard 401) must be discarded, and the re-route must skip that
    /// model even though the circuit was recorded under the bare API id
    /// while the catalog id is prefixed ("anthropic/…").
    #[tokio::test]
    async fn test_sticky_and_routing_skip_model_with_open_circuit() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        key_pool
            .add_global_key(ApiKeyEntry {
                key_id: "test-cloud".to_string(),
                provider: "anthropic".to_string(),
                api_key: "sk-ant-test".to_string(),
                api_url: None,
                label: None,
                is_active: true,
                rate_limited_until: None,
                active_models: None,
                is_free_tier: false,
            })
            .await?;
        let router = CognitiveRouter::new(catalog, key_pool);

        let mut pcb = PCB::new(
            "test".to_string(),
            5,
            "Could you please provide a detailed explanation of the cognitive routing architecture in Aegis Core?".to_string(),
        );
        pcb.task_type = TaskType::Chat;
        pcb.model_pref = ModelPreference::CloudOnly;

        let first = router.decide(&pcb).await?;

        // The chal layer records hard failures under the BARE api id —
        // exactly what RoutingDecision::model_id carries.
        router
            .tracker_ref()
            .record_model_unavailable(&first.model_id)
            .await;

        let second = router.decide(&pcb).await?;
        assert_ne!(
            second.model_id, first.model_id,
            "a model with an open per-model circuit must not be re-picked"
        );
        Ok(())
    }

    /// CORE-320: with ≥ 3 recorded samples a flaky model must score below an
    /// otherwise identical stable one; with < 3 samples there is no signal
    /// and the score must be unchanged.
    #[tokio::test]
    async fn test_reliability_factor_penalises_flaky_model() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let router = CognitiveRouter::new(catalog, key_pool);
        let entry = router.catalog_find("openai/gpt-4o").await.unwrap();

        let base_ctx = |outcomes: ModelOutcomes| ScoreCtx {
            prompt: "Implement a parser in Rust",
            max_cost: entry.cost_input_per_mtok + entry.cost_output_per_mtok,
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::Balanced,
            outcomes,
        };

        let stable =
            router.compute_score(&entry, TaskType::Code, &base_ctx(ModelOutcomes::default()));
        let flaky = router.compute_score(
            &entry,
            TaskType::Code,
            &base_ctx(ModelOutcomes {
                successes: 1,
                failures: 5,
            }),
        );
        let insufficient = router.compute_score(
            &entry,
            TaskType::Code,
            &base_ctx(ModelOutcomes {
                successes: 0,
                failures: 2,
            }),
        );

        assert!(
            flaky < stable,
            "flaky model ({}) must score below stable ({})",
            flaky,
            stable
        );
        assert!(
            (insufficient - stable).abs() < f64::EPSILON,
            "fewer than 3 samples must not change the score"
        );
        Ok(())
    }

    /// CORE-320: a premium model that FAILED the live tool-use probe
    /// (ToolUseSupport::Degraded) must drop to the bottom for subagent
    /// tasks, while its Chat score stays untouched.
    #[tokio::test]
    async fn test_degraded_tool_support_penalised_for_subagents() -> anyhow::Result<()> {
        let catalog = Arc::new(ModelCatalog::load_bundled_with_profile(
            ModelProfile::Hybrid,
        )?);
        let key_pool = Arc::new(KeyPool::new(Arc::new(NoopPersistor)));
        let router = CognitiveRouter::new(catalog, key_pool);

        let healthy = router.catalog_find("openai/gpt-4o").await.unwrap();
        let mut degraded = healthy.clone();
        degraded.tool_use_support = ToolUseSupport::Degraded;

        let ctx = ScoreCtx {
            prompt: "Implement a parser in Rust",
            max_cost: healthy.cost_input_per_mtok + healthy.cost_output_per_mtok,
            max_latency: 1000.0,
            observed_latency: None,
            recent_errors: 0,
            estimated_tokens: 10,
            routing_policy: RoutingPolicy::Balanced,
            outcomes: ModelOutcomes::default(),
        };

        let code_healthy = router.compute_score(&healthy, TaskType::Code, &ctx);
        let code_degraded = router.compute_score(&degraded, TaskType::Code, &ctx);
        assert!(
            code_degraded < code_healthy * 0.1,
            "degraded tool support must collapse the subagent score ({} vs {})",
            code_degraded,
            code_healthy
        );

        let chat_healthy = router.compute_score(&healthy, TaskType::Chat, &ctx);
        let chat_degraded = router.compute_score(&degraded, TaskType::Chat, &ctx);
        assert!(
            (chat_healthy - chat_degraded).abs() < f64::EPSILON,
            "Chat scoring must be unaffected by tool degradation"
        );
        Ok(())
    }
}
