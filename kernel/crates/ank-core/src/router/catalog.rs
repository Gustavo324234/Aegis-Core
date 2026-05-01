use crate::pcb::TaskType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Soporte de tool use del modelo/proveedor (CORE-237).
/// Usado por el CognitiveHAL para activar/desactivar inyección de herramientas en Ollama.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolUseSupport {
    /// Estado inicial — aún no se ha probado si el modelo soporta tool use.
    #[default]
    Unknown,
    /// El modelo respondió correctamente a una llamada con tools.
    Supported,
    /// El modelo no soporta tool use (error 400 o respuesta de texto sin tool_calls).
    Degraded,
}

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
    /// Estado de soporte de tool use detectado en runtime (CORE-237).
    #[serde(default)]
    pub tool_use_support: ToolUseSupport,
    #[serde(default)]
    pub task_scores: TaskScores,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default)]
    pub avg_latency_ms: Option<u32>,
    /// Free-tier requests-per-minute limit (None = no known limit / paid-only model).
    #[serde(default)]
    pub free_tier_rpm: Option<u32>,
    /// Free-tier requests-per-day limit (None = no known limit / paid-only model).
    #[serde(default)]
    pub free_tier_rpd: Option<u32>,
}

impl ModelEntry {
    pub fn score_for(&self, task: TaskType) -> u8 {
        match task {
            TaskType::Chat => self.task_scores.chat,
            TaskType::Code => self.task_scores.coding,
            TaskType::Planning => self.task_scores.planning,
            TaskType::Analysis => self.task_scores.analysis,
            TaskType::Summarization => self.task_scores.summarization,
            TaskType::Extraction => self.task_scores.extraction,
            TaskType::Creative => self.task_scores.analysis, // Creative usa el score de analysis como fallback
            TaskType::Local => 5, // Local type always scores max for local models
        }
    }
}

/// Perfil de inferencia — controla qué modelos están disponibles en el catálogo.
/// Se configura via variable de entorno `AEGIS_MODEL_PROFILE`.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelProfile {
    /// Solo modelos cloud (is_local=false). Default para servidores sin GPU/Ollama.
    Cloud,
    /// Solo modelos locales (is_local=true). Para instancias air-gapped con Ollama.
    Local,
    /// Todos los modelos disponibles. El Router elige según scoring.
    Hybrid,
}

impl ModelProfile {
    /// Lee el perfil desde la variable de entorno `AEGIS_MODEL_PROFILE`.
    /// Valores válidos: "cloud", "local", "hybrid". Default: "cloud".
    pub fn from_env() -> Self {
        match std::env::var("AEGIS_MODEL_PROFILE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "local" => Self::Local,
            "hybrid" => Self::Hybrid,
            _ => Self::Cloud, // default seguro: cloud
        }
    }

    /// Filtra una lista de entries según el perfil.
    pub fn filter(&self, entries: Vec<ModelEntry>) -> Vec<ModelEntry> {
        match self {
            Self::Cloud => entries.into_iter().filter(|e| !e.is_local).collect(),
            Self::Local => entries.into_iter().filter(|e| e.is_local).collect(),
            Self::Hybrid => entries,
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
        Self::load_bundled_with_profile(ModelProfile::from_env())
    }

    pub fn load_bundled_with_profile(profile: ModelProfile) -> anyhow::Result<Self> {
        let all_entries: Vec<ModelEntry> = serde_yaml::from_str(BUNDLED_YAML)
            .map_err(|e| anyhow::anyhow!("Failed to parse bundled models.yaml: {}", e))?;

        anyhow::ensure!(
            !all_entries.is_empty(),
            "models.yaml is empty — cannot start router"
        );

        let entries = profile.filter(all_entries);

        anyhow::ensure!(
            !entries.is_empty(),
            "No models available after applying profile {:?}. Check AEGIS_MODEL_PROFILE and models.yaml.",
            profile
        );

        tracing::info!(
            profile = ?profile,
            count = entries.len(),
            "ModelCatalog loaded with profile filter"
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
        let catalog = ModelCatalog::load_bundled_with_profile(ModelProfile::Hybrid)?;
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
        let catalog = ModelCatalog::load_bundled_with_profile(ModelProfile::Hybrid)?;
        let candidates = catalog.get_candidates(TaskType::Code).await;
        assert!(!candidates.is_empty());
        for c in &candidates {
            assert!(c.task_scores.coding >= 3);
        }
        for i in 1..candidates.len() {
            assert!(candidates[i - 1].task_scores.coding >= candidates[i].task_scores.coding);
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_cloud_profile_excludes_local() -> anyhow::Result<()> {
        let catalog = ModelCatalog::load_bundled_with_profile(ModelProfile::Cloud)?;
        let all = catalog.all_entries().await;
        for entry in &all {
            assert!(
                !entry.is_local,
                "Cloud profile should not include local model: {}",
                entry.model_id
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_local_profile_excludes_cloud() -> anyhow::Result<()> {
        let catalog = ModelCatalog::load_bundled_with_profile(ModelProfile::Local)?;
        let all = catalog.all_entries().await;
        for entry in &all {
            assert!(
                entry.is_local,
                "Local profile should not include cloud model: {}",
                entry.model_id
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_hybrid_profile_includes_all() -> anyhow::Result<()> {
        let catalog_hybrid = ModelCatalog::load_bundled_with_profile(ModelProfile::Hybrid)?;
        let catalog_cloud = ModelCatalog::load_bundled_with_profile(ModelProfile::Cloud)?;
        let catalog_local = ModelCatalog::load_bundled_with_profile(ModelProfile::Local)?;

        let hybrid_count = catalog_hybrid.all_entries().await.len();
        let cloud_count = catalog_cloud.all_entries().await.len();
        let local_count = catalog_local.all_entries().await.len();

        assert_eq!(
            hybrid_count,
            cloud_count + local_count,
            "Hybrid should equal cloud + local"
        );
        Ok(())
    }
}
