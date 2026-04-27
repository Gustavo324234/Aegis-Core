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
}
