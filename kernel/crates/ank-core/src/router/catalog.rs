use crate::pcb::TaskType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskScores {
    #[serde(default)]
    pub chat: u8,
    #[serde(default)]
    pub coding: u8,
    #[serde(default)]
    pub planning: u8,
    #[serde(default)]
    pub analysis: u8,
    #[serde(default)]
    pub summarization: u8,
    #[serde(default)]
    pub extraction: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub model_id: String,
    pub provider: String,
    pub display_name: String,
    pub context_window: u32,
    pub cost_input_per_mtok: f64,
    pub cost_output_per_mtok: f64,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_json_mode: bool,
    #[serde(default)]
    pub task_scores: TaskScores,
    #[serde(default)]
    pub is_local: bool,
}

impl ModelEntry {
    pub fn score_for(&self, task: TaskType) -> u8 {
        match task {
            TaskType::Chat => self.task_scores.chat,
            TaskType::Coding => self.task_scores.coding,
            TaskType::Planning => self.task_scores.planning,
            TaskType::Analysis => self.task_scores.analysis,
            TaskType::Summarization => self.task_scores.summarization,
            TaskType::Extraction => self.task_scores.extraction,
            TaskType::Local => 5, // Local type always scores max for local models
        }
    }
}

pub struct ModelCatalog {
    entries: Arc<RwLock<Vec<ModelEntry>>>,
    last_synced: Arc<RwLock<Option<DateTime<Utc>>>>,
}

static BUNDLED_YAML: &str = include_str!("models.yaml");

impl ModelCatalog {
    pub fn load_bundled() -> anyhow::Result<Self> {
        let entries: Vec<ModelEntry> = serde_yaml::from_str(BUNDLED_YAML)
            .map_err(|e| anyhow::anyhow!("Failed to parse bundled models.yaml: {}", e))?;
        anyhow::ensure!(
            !entries.is_empty(),
            "models.yaml is empty — cannot start router"
        );
        Ok(Self {
            entries: Arc::new(RwLock::new(entries)),
            last_synced: Arc::new(RwLock::new(None)),
        })
    }

    pub async fn get_candidates(&self, task: TaskType) -> Vec<ModelEntry> {
        let entries = self.entries.read().await;
        let mut candidates: Vec<ModelEntry> = entries
            .iter()
            .filter(|e| e.score_for(task) >= 3)
            .cloned()
            .collect();
        candidates.sort_by_key(|b| std::cmp::Reverse(b.score_for(task)));
        candidates
    }

    pub async fn find(&self, model_id: &str) -> Option<ModelEntry> {
        let entries = self.entries.read().await;
        entries.iter().find(|e| e.model_id == model_id).cloned()
    }

    pub async fn replace_all(&self, new_entries: Vec<ModelEntry>) {
        let mut entries = self.entries.write().await;
        *entries = new_entries;
        let mut last = self.last_synced.write().await;
        *last = Some(chrono::Utc::now());
    }

    pub async fn last_synced(&self) -> Option<DateTime<Utc>> {
        *self.last_synced.read().await
    }

    pub async fn all_entries(&self) -> Vec<ModelEntry> {
        self.entries.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_bundled_not_empty() -> anyhow::Result<()> {
        let catalog = ModelCatalog::load_bundled()?;
        let all = catalog.all_entries().await;
        assert!(!all.is_empty(), "Bundled catalog must not be empty");
        assert!(
            all.len() >= 15,
            "Bundled catalog must have at least 15 models"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_get_candidates_coding() -> anyhow::Result<()> {
        let catalog = ModelCatalog::load_bundled()?;
        let candidates = catalog.get_candidates(TaskType::Coding).await;
        assert!(!candidates.is_empty());
        // All candidates have score >= 3
        for c in &candidates {
            assert!(c.task_scores.coding >= 3);
        }
        // Sorted by score desc
        for i in 1..candidates.len() {
            assert!(candidates[i - 1].task_scores.coding >= candidates[i].task_scores.coding);
        }
        Ok(())
    }
}
