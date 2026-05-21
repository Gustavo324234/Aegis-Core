use chrono::{Local, NaiveDate};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Tracks per-model request counts within a sliding 60-second window (RPM)
/// and a daily window (RPD). Used by CognitiveRouter to enforce free-tier
/// rate limits proactively, before Google returns a 429.
pub struct ModelUsageTracker {
    minute_window: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
    daily_counts: Arc<RwLock<HashMap<String, (u32, NaiveDate)>>>,
    latency_samples: Arc<RwLock<HashMap<String, VecDeque<u32>>>>,
    error_window: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
    /// CORE-FIX: per-model success/failure counts since process start.
    /// Reset never — accumulated over the lifetime of the router.
    outcomes: Arc<RwLock<HashMap<String, ModelOutcomes>>>,
    /// CORE-FIX: circuit breaker — recent failures per provider (not per model).
    /// If a provider as a whole is failing, no point trying its individual models.
    provider_failures: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
    /// CORE-FIX (D): per-model "returned 200 OK but zero content tokens" tracker.
    /// Some providers (notably ollama_cloud serving 671B models) silently return
    /// nothing instead of erroring; we treat that as an implicit failure and
    /// trip a per-model circuit so the router stops picking that model.
    empty_responses: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
    /// CORE-FIX (F): per-model HARD-unavailable tracker. Unlike empty_responses
    /// (transient, needs 2 hits to trip), this trips the circuit on the FIRST
    /// deterministic failure: HTTP 401 (key/model not authorized) or a Gemini
    /// 429 with `limit: 0` (the model isn't on the key's tier at all). These
    /// never succeed on retry, so re-picking the same model every request just
    /// burns time and keys. One hit → skip the model for 5 minutes.
    model_unavailable: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ModelOutcomes {
    pub successes: u64,
    pub failures: u64,
}

impl ModelOutcomes {
    pub fn success_rate(&self) -> Option<f64> {
        let total = self.successes + self.failures;
        if total == 0 {
            None
        } else {
            Some(self.successes as f64 / total as f64)
        }
    }
}

impl Default for ModelUsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelUsageTracker {
    pub fn new() -> Self {
        Self {
            minute_window: Arc::new(RwLock::new(HashMap::new())),
            daily_counts: Arc::new(RwLock::new(HashMap::new())),
            latency_samples: Arc::new(RwLock::new(HashMap::new())),
            error_window: Arc::new(RwLock::new(HashMap::new())),
            outcomes: Arc::new(RwLock::new(HashMap::new())),
            provider_failures: Arc::new(RwLock::new(HashMap::new())),
            empty_responses: Arc::new(RwLock::new(HashMap::new())),
            model_unavailable: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Call this when a routing decision is made for a model with a free-tier key.
    pub async fn record_request(&self, model_id: &str) {
        let now = Instant::now();
        let today = Local::now().date_naive();

        {
            let mut map = self.minute_window.write().await;
            let queue = map.entry(model_id.to_string()).or_default();
            let cutoff = now - Duration::from_secs(60);
            while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
                queue.pop_front();
            }
            queue.push_back(now);
        }

        {
            let mut map = self.daily_counts.write().await;
            let entry = map.entry(model_id.to_string()).or_insert((0, today));
            if entry.1 != today {
                *entry = (1, today);
            } else {
                entry.0 += 1;
            }
        }
    }

    async fn requests_last_minute(&self, model_id: &str) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(60);
        let mut map = self.minute_window.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.len() as u32
    }

    async fn requests_today(&self, model_id: &str) -> u32 {
        let today = Local::now().date_naive();
        let map = self.daily_counts.read().await;
        match map.get(model_id) {
            Some((count, date)) if *date == today => *count,
            _ => 0,
        }
    }

    /// Returns a capacity factor in [0.0, 1.0]:
    /// - 1.0 → no usage or no configured limits
    /// - 0.0 → fully exhausted (requests >= limit)
    /// - intermediate → approaching limit; router penalises the score proportionally
    ///
    /// Only meaningful for free-tier keys — callers that hold a paid key should ignore this.
    pub async fn capacity_factor(
        &self,
        model_id: &str,
        free_tier_rpm: Option<u32>,
        free_tier_rpd: Option<u32>,
    ) -> f64 {
        let mut factor = 1.0_f64;

        if let Some(rpm) = free_tier_rpm {
            if rpm > 0 {
                let used = self.requests_last_minute(model_id).await as f64;
                factor = factor.min((1.0 - used / rpm as f64).max(0.0));
            }
        }

        if let Some(rpd) = free_tier_rpd {
            if rpd > 0 {
                let used = self.requests_today(model_id).await as f64;
                factor = factor.min((1.0 - used / rpd as f64).max(0.0));
            }
        }

        factor
    }

    /// Registers the observed round-trip latency (ms) after a completed request.
    pub async fn record_latency(&self, model_id: &str, latency_ms: u32) {
        let mut map = self.latency_samples.write().await;
        let samples = map.entry(model_id.to_string()).or_default();
        if samples.len() >= 20 {
            samples.pop_front();
        }
        samples.push_back(latency_ms);
    }

    /// Records a provider error to temporarily penalise the model in scoring.
    pub async fn record_error(&self, model_id: &str) {
        let now = Instant::now();
        let mut map = self.error_window.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        let cutoff = now - Duration::from_secs(300);
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.push_back(now);
    }

    /// Returns the average observed latency, or None if no samples exist yet.
    pub async fn observed_latency_ms(&self, model_id: &str) -> Option<u32> {
        let map = self.latency_samples.read().await;
        let samples = map.get(model_id)?;
        if samples.is_empty() {
            return None;
        }
        let avg = samples.iter().map(|&v| v as u64).sum::<u64>() / samples.len() as u64;
        Some(avg as u32)
    }

    /// Returns the number of errors recorded in the last 5 minutes.
    pub async fn recent_errors(&self, model_id: &str) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(300);
        let mut map = self.error_window.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.len() as u32
    }

    // ─── CORE-FIX: D2 — per-model success/failure outcome tracking ───────

    /// Increment the success counter for a model.
    pub async fn record_success(&self, model_id: &str) {
        let mut map = self.outcomes.write().await;
        map.entry(model_id.to_string()).or_default().successes += 1;
    }

    /// Increment the failure counter for a model and tally a provider-level
    /// failure for the circuit breaker.
    pub async fn record_failure(&self, model_id: &str, provider: &str) {
        // A failure is also a scoring signal: feed the per-model error window so
        // CognitiveRouter::compute_score deprioritises a model that just failed
        // for the next 5 minutes. Without this the `error_penalty` term was dead
        // — nothing fed the window — so a rate-limited model (e.g. groq's 12k-TPM
        // free tier, which a single agent turn blows past) kept winning the top
        // score and 429-ing again on every fresh turn instead of yielding to a
        // sibling with headroom.
        self.record_error(model_id).await;
        {
            let mut map = self.outcomes.write().await;
            map.entry(model_id.to_string()).or_default().failures += 1;
        }
        // Also count this against the provider — feeds the circuit breaker.
        let now = Instant::now();
        let mut pf = self.provider_failures.write().await;
        let queue = pf.entry(provider.to_string()).or_default();
        // Keep only last 30s of failures.
        let cutoff = now - Duration::from_secs(30);
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.push_back(now);
    }

    /// Snapshot of outcomes for a model. None if no requests recorded yet.
    pub async fn outcomes_for(&self, model_id: &str) -> Option<ModelOutcomes> {
        self.outcomes.read().await.get(model_id).copied()
    }

    /// Full snapshot of all model outcomes — for /stats or telemetry endpoints.
    pub async fn all_outcomes(&self) -> HashMap<String, ModelOutcomes> {
        self.outcomes.read().await.clone()
    }

    // ─── CORE-FIX: B3 — circuit breaker per provider ─────────────────────

    /// Number of failures recorded for `provider` in the last 30 seconds.
    pub async fn provider_failures_recent(&self, provider: &str) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(30);
        let mut map = self.provider_failures.write().await;
        let queue = map.entry(provider.to_string()).or_default();
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.len() as u32
    }

    /// Circuit is "open" (i.e. skip this provider) if it has accumulated
    /// 3 or more failures in the last 30 seconds. Closes automatically
    /// once the window slides past.
    pub async fn is_provider_circuit_open(&self, provider: &str) -> bool {
        self.provider_failures_recent(provider).await >= 3
    }

    /// CORE-FIX (inspired by OpenClaw's per-profile cooldown tracking):
    /// returns the number of seconds until this provider's circuit closes
    /// (i.e. its oldest failure slides out of the 30s window), or None if
    /// the circuit is currently closed (provider is OK to try).
    ///
    /// The UI uses this to render "retry in Ns" countdowns instead of just
    /// showing the user a generic "all models exhausted" error.
    pub async fn provider_cooldown_remaining_secs(&self, provider: &str) -> Option<u64> {
        let pf = self.provider_failures.read().await;
        let queue = pf.get(provider)?;
        let oldest = *queue.front()?;
        // Cooldown closes when the oldest failure ages past the 30s window.
        let now = Instant::now();
        let window = Duration::from_secs(30);
        let age = now.duration_since(oldest);
        if age >= window {
            None
        } else {
            Some((window - age).as_secs().max(1))
        }
    }

    // ─── CORE-FIX (D): per-model "silent failure" circuit ────────────────
    //
    // Some providers (ollama_cloud is the worst offender at the moment)
    // happily reply HTTP 200 with an empty content stream when their
    // model is overloaded, mis-configured, or doesn't actually exist.
    // The chal layer treats that as an implicit failure and walks the
    // fallback chain — but if we let the router keep picking that same
    // model on the next request, every chat starts with a stuttered
    // fallback. These methods give us a per-model circuit breaker
    // independent of the provider-level one.

    /// Record that a model returned 200 OK with zero content tokens.
    /// Kept in a 5-minute sliding window.
    pub async fn record_empty_response(&self, model_id: &str) {
        let now = Instant::now();
        let mut map = self.empty_responses.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        let cutoff = now - Duration::from_secs(300);
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.push_back(now);
    }

    /// Number of empty-response events for this model in the last 5 minutes.
    pub async fn empty_responses_recent(&self, model_id: &str) -> u32 {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(300);
        let mut map = self.empty_responses.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.len() as u32
    }

    /// CORE-FIX (F): record a deterministic, non-retryable failure for a model
    /// (HTTP 401, or a Gemini 429 with `limit: 0`). Trips the model circuit on
    /// the FIRST occurrence — kept in the same 5-minute sliding window.
    pub async fn record_model_unavailable(&self, model_id: &str) {
        let now = Instant::now();
        let mut map = self.model_unavailable.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        let cutoff = now - Duration::from_secs(300);
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        queue.push_back(now);
    }

    /// Whether this model had a hard-unavailable event in the last 5 minutes.
    pub async fn model_unavailable_recent(&self, model_id: &str) -> bool {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(300);
        let mut map = self.model_unavailable.write().await;
        let queue = map.entry(model_id.to_string()).or_default();
        while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
            queue.pop_front();
        }
        !queue.is_empty()
    }

    /// Circuit is "open" (i.e. skip this model in routing) when EITHER:
    /// - it returned an empty stream twice within the last 5 minutes
    ///   (transient/soft failure), OR
    /// - it had ONE hard-unavailable event (401 / tier-0 quota) in the last
    ///   5 minutes (deterministic failure — no point retrying).
    ///
    /// The circuit auto-closes as those events age out of the window.
    pub async fn is_model_circuit_open(&self, model_id: &str) -> bool {
        if self.model_unavailable_recent(model_id).await {
            return true;
        }
        self.empty_responses_recent(model_id).await >= 2
    }

    /// Seconds remaining until this model's circuit closes (oldest empty
    /// response slides out of the 5min window). `None` when the circuit
    /// is currently closed.
    pub async fn model_cooldown_remaining_secs(&self, model_id: &str) -> Option<u64> {
        let map = self.empty_responses.read().await;
        let queue = map.get(model_id)?;
        if queue.len() < 2 {
            return None;
        }
        let oldest = *queue.front()?;
        let now = Instant::now();
        let window = Duration::from_secs(300);
        let age = now.duration_since(oldest);
        if age >= window {
            None
        } else {
            Some((window - age).as_secs().max(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn record_failure_feeds_error_window_for_scoring() {
        // The error_penalty in compute_score reads recent_errors(); failures must
        // populate that window or a rate-limited model never gets deprioritised.
        let t = ModelUsageTracker::new();
        assert_eq!(t.recent_errors("groq/llama").await, 0);
        t.record_failure("groq/llama", "groq").await;
        t.record_failure("groq/llama", "groq").await;
        assert_eq!(
            t.recent_errors("groq/llama").await,
            2,
            "failures must feed recent_errors so compute_score penalises a flaky model"
        );
        // A different, healthy model is unaffected.
        assert_eq!(t.recent_errors("gpt-oss:120b").await, 0);
    }

    #[tokio::test]
    async fn record_failure_still_trips_provider_circuit() {
        // Feeding the error window must not break the existing 3-in-30s circuit.
        let t = ModelUsageTracker::new();
        for _ in 0..3 {
            t.record_failure("m", "ollama_cloud").await;
        }
        assert!(t.is_provider_circuit_open("ollama_cloud").await);
    }
}
