use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const MAX_TELEMETRY_WINDOW: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedInference {
    pub tokens_per_second: f64,
    pub tokens_emitted: u32,
    pub model_id: String,
    pub duration_ms: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TelemetryWindow {
    inferences: VecDeque<CompletedInference>,
    max_size: usize,
    total_tokens_session: u64,
}

impl TelemetryWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            inferences: VecDeque::with_capacity(max_size),
            max_size,
            total_tokens_session: 0,
        }
    }

    pub fn add(&mut self, inference: CompletedInference) {
        if self.inferences.len() >= self.max_size {
            if let Some(removed) = self.inferences.pop_front() {
                self.total_tokens_session = self
                    .total_tokens_session
                    .saturating_sub(removed.tokens_emitted as u64);
            }
        }
        self.total_tokens_session += inference.tokens_emitted as u64;
        self.inferences.push_back(inference);
    }

    pub fn tokens_per_second_avg(&self) -> f64 {
        if self.inferences.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.inferences.iter().map(|i| i.tokens_per_second).sum();
        sum / self.inferences.len() as f64
    }

    pub fn total_tokens_session(&self) -> u64 {
        self.total_tokens_session
    }

    pub fn estimated_cost_usd(&self) -> Option<f64> {
        let mut sum = 0.0_f64;
        let mut has_any = false;
        for inf in &self.inferences {
            if let Some(cost) = inf.cost_usd {
                sum += cost;
                has_any = true;
            }
        }
        if has_any {
            Some(sum)
        } else {
            None
        }
    }

    pub fn completed_count(&self) -> usize {
        self.inferences.len()
    }
}

impl Default for TelemetryWindow {
    fn default() -> Self {
        Self::new(MAX_TELEMETRY_WINDOW)
    }
}

#[derive(Clone)]
pub struct TelemetryState {
    window: Arc<Mutex<TelemetryWindow>>,
}

impl TelemetryState {
    pub fn new() -> Self {
        Self {
            window: Arc::new(Mutex::new(TelemetryWindow::default())),
        }
    }

    pub async fn add_inference(&self, inference: CompletedInference) {
        let mut window = self.window.lock().await;
        window.add(inference);
    }

    pub async fn metrics(&self) -> TelemetryMetrics {
        let window = self.window.lock().await;
        TelemetryMetrics {
            tokens_per_second: window.tokens_per_second_avg(),
            total_tokens_session: window.total_tokens_session(),
            estimated_cost_usd: window.estimated_cost_usd(),
            completed_inferences: window.completed_count(),
        }
    }
}

impl Default for TelemetryState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    pub tokens_per_second: f64,
    pub total_tokens_session: u64,
    pub estimated_cost_usd: Option<f64>,
    pub completed_inferences: usize,
}
