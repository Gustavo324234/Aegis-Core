pub mod catalog;
pub mod discovery;
pub mod key_pool;
pub mod rate_tracker;
pub mod siren;
pub mod syncer;

pub use siren::{SirenEngine, SirenRouter};

use crate::chal::SystemError;
use crate::pcb::{TaskType, PCB};
use crate::scheduler::ModelPreference;
pub use catalog::{ModelCatalog, ModelEntry, ToolUseSupport};
pub use key_pool::KeyPool;
pub use rate_tracker::ModelUsageTracker;
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

pub struct CognitiveRouter {
    catalog: Arc<ModelCatalog>,
    key_pool: Arc<KeyPool>,
    tracker: Arc<ModelUsageTracker>,
    /// CORE-FIX (B4): cache of the last routing decision per conversation
    /// intent. Keeps consecutive turns on the same model so the persona/style
    /// stays consistent. Invalidated on failure (see `invalidate_sticky`).
    sticky: Arc<RwLock<HashMap<StickyKey, (Instant, RoutingDecision)>>>,
}

impl CognitiveRouter {
    pub fn new(catalog: Arc<ModelCatalog>, key_pool: Arc<KeyPool>) -> Self {
        Self {
            catalog,
            key_pool,
            tracker: Arc::new(ModelUsageTracker::new()),
            sticky: Arc::new(RwLock::new(HashMap::new())),
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
                    // Re-validate the model is still healthy before reusing.
                    if !self
                        .tracker
                        .is_provider_circuit_open(&decision.provider)
                        .await
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
                skipped_by_breaker.push(entry.model_id.clone());
                providers_in_cooldown.insert(entry.provider.clone());
                continue;
            }
            // CORE-FIX (D): per-model circuit. The provider may be perfectly
            // healthy overall (e.g. ollama_cloud responds for gpt-oss:120b)
            // but a specific model on it keeps returning HTTP 200 with zero
            // content (cogito-2.1:671b from the smoke test). Skip just that
            // model so the router promotes a sibling instead of falling
            // through to a different provider.
            if self.tracker.is_model_circuit_open(&entry.model_id).await {
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
            .map(|e| e.cost_input_per_mtok + e.cost_output_per_mtok)
            .fold(0.0_f64, f64::max);
        let max_latency = {
            let mut ml = 0.0_f64;
            for e in &available {
                let obs = self.tracker.observed_latency_ms(&e.model_id).await;
                let lat = obs.unwrap_or(e.avg_latency_ms.unwrap_or(2000)) as f64;
                if lat > ml {
                    ml = lat;
                }
            }
            ml
        };

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
                self.tracker
                    .capacity_factor(&entry.model_id, entry.free_tier_rpm, entry.free_tier_rpd)
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
                observed_latency: self.tracker.observed_latency_ms(&entry.model_id).await,
                recent_errors: self.tracker.recent_errors(&entry.model_id).await,
            };
            let base = self.compute_score(&entry, task_type, &ctx);
            // Soft penalty: multiply by sqrt(capacity) so a model at 50% headroom
            // scores ~70% of its base, still competitive but deprioritised.
            scored.push((base * capacity.sqrt(), entry));
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
        let mut fallback_chain: Vec<FallbackDecision> = Vec::new();
        for (_, entry) in scored.iter().skip(1).take(2) {
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
        let fallback_ids: Vec<String> = scored
            .iter()
            .skip(1)
            .take(2)
            .map(|(_, e)| e.model_id.clone())
            .collect();
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
            let sticky_key = StickyKey {
                tenant_id: tenant_id.to_string(),
                task_type,
                model_pref: effective_pref,
            };
            self.sticky
                .write()
                .await
                .insert(sticky_key, (Instant::now(), decision.clone()));
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

    pub fn tracker_ref(&self) -> &Arc<ModelUsageTracker> {
        &self.tracker
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
        let total_cost = entry.cost_input_per_mtok + entry.cost_output_per_mtok;
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
        let estimated_tokens = (prompt.len() / 4).max(1);
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

        // ── 7. Task-aware weights ────────────────────────────────────
        // CORE-FIX: previously the formula was fixed (quality 40 / cost
        // 25 / speed 20 / fit 15). For Chat that gave cost too much
        // pull and rewarded free-but-massive models. Re-weight per
        // task so:
        //   - Chat        → fast + cheap matter more than peak quality
        //   - Code/Plan   → quality dominates, cost matters less
        //   - Analysis    → quality dominates
        //   - others      → original balanced weights
        let (w_quality, w_cost, w_speed, w_fit) = match task_type {
            TaskType::Chat => (0.30, 0.20, 0.35, 0.15),
            TaskType::Code | TaskType::Planning => (0.55, 0.15, 0.15, 0.15),
            TaskType::Analysis => (0.55, 0.15, 0.15, 0.15),
            _ => (0.40, 0.25, 0.20, 0.15),
        };

        let raw =
            quality * w_quality + cost_inv * w_cost + speed_inv * w_speed + context_fit * w_fit;
        (raw * (1.0 - error_penalty) * (1.0 - oversize_penalty)).max(0.0)
    }

    /// Busca una entrada en el catálogo por model_id (CORE-237).
    pub async fn catalog_find(&self, model_id: &str) -> Option<ModelEntry> {
        self.catalog.find(model_id).await
    }

    /// Actualiza el estado de tool_use_support de un modelo en el catálogo (CORE-237).
    pub async fn update_tool_use_support(
        &self,
        model_id: &str,
        support: crate::router::catalog::ToolUseSupport,
    ) {
        let mut entries = self.catalog.all_entries().await;
        for entry in &mut entries {
            if entry.model_id == model_id {
                entry.tool_use_support = support;
                break;
            }
        }
        self.catalog.replace_all(entries).await;
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
        // All other providers: strip the leading "provider/" prefix if present
        _ => model_id
            .split_once('/')
            .map(|(_, bare)| bare.to_string())
            .unwrap_or_else(|| model_id.to_string()),
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
}
