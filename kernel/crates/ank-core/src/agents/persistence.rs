use crate::agents::node::{AgentId, ProjectId};
use crate::agents::tree::AgentTree;
use anyhow::Context;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Maneja la persistencia del árbol de agentes y los state summaries en el filesystem.
///
/// Estructura de directorios (ADR-CAA-013):
/// ```text
/// {data_dir}/users/{tenant_id}/projects/{project_id}/
/// ├── project.json          <- metadata del proyecto
/// ├── agent_tree.json       <- estructura del arbol serializada
/// └── agent_contexts/
///     ├── {agent_id}.md     <- state summary por supervisor
///     └── ...
/// ```
pub struct AgentPersistence {
    /// Raíz del directorio de datos de Aegis (AEGIS_DATA_DIR)
    data_dir: PathBuf,
}

impl AgentPersistence {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn from_env() -> Self {
        let dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        Self::new(dir)
    }

    // --- Rutas ---

    pub fn project_dir(&self, tenant_id: &str, project_id: &ProjectId) -> PathBuf {
        self.data_dir
            .join("users")
            .join(tenant_id)
            .join("projects")
            .join(project_id)
    }

    pub fn tree_path(&self, tenant_id: &str, project_id: &ProjectId) -> PathBuf {
        self.project_dir(tenant_id, project_id)
            .join("agent_tree.json")
    }

    pub fn contexts_dir(&self, tenant_id: &str, project_id: &ProjectId) -> PathBuf {
        self.project_dir(tenant_id, project_id).join("agent_contexts")
    }

    pub fn context_path(&self, tenant_id: &str, project_id: &ProjectId, agent_id: &AgentId) -> PathBuf {
        self.contexts_dir(tenant_id, project_id)
            .join(format!("{}.md", agent_id))
    }

    // --- Árbol ---

    /// Serializa el árbol a `agent_tree.json`. Crea los directorios si no existen.
    pub fn save_tree(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
        tree: &AgentTree,
    ) -> anyhow::Result<()> {
        let path = self.tree_path(tenant_id, project_id);
        self.ensure_dir(path.parent().expect("tree path has parent"))?;

        let json = tree.to_json().context("Failed to serialize agent tree")?;
        std::fs::write(&path, json.as_bytes())
            .with_context(|| format!("Failed to write agent_tree.json at {:?}", path))?;

        info!(
            tenant = %tenant_id,
            project = %project_id,
            path = %path.display(),
            "[AgentPersistence] Tree saved ({} nodes).",
            tree.len()
        );
        Ok(())
    }

    /// Carga el árbol desde `agent_tree.json`. Retorna `None` si no existe.
    pub fn load_tree(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<Option<AgentTree>> {
        let path = self.tree_path(tenant_id, project_id);
        if !path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read agent_tree.json at {:?}", path))?;

        let tree = AgentTree::from_json(&json)
            .with_context(|| format!("Failed to parse agent_tree.json at {:?}", path))?;

        info!(
            tenant = %tenant_id,
            project = %project_id,
            "[AgentPersistence] Tree loaded ({} nodes, all marked is_restored=true).",
            tree.len()
        );
        Ok(Some(tree))
    }

    /// Verifica si existe un árbol guardado para un proyecto.
    pub fn has_saved_tree(&self, tenant_id: &str, project_id: &ProjectId) -> bool {
        self.tree_path(tenant_id, project_id).exists()
    }

    // --- State Summaries ---

    /// Guarda el state summary de un supervisor en `agent_contexts/{agent_id}.md`.
    /// También asigna el `persisted_context_path` en el nodo para referencia futura.
    pub fn save_state_summary(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
        agent_id: &AgentId,
        summary: &str,
    ) -> anyhow::Result<PathBuf> {
        let path = self.context_path(tenant_id, project_id, agent_id);
        self.ensure_dir(path.parent().expect("context path has parent"))?;

        std::fs::write(&path, summary.as_bytes())
            .with_context(|| format!("Failed to write state summary at {:?}", path))?;

        info!(
            tenant = %tenant_id,
            project = %project_id,
            agent = %agent_id,
            "[AgentPersistence] State summary saved."
        );
        Ok(path)
    }

    /// Carga el state summary de un supervisor. Retorna `None` si no existe.
    pub fn load_state_summary(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
        agent_id: &AgentId,
    ) -> anyhow::Result<Option<String>> {
        let path = self.context_path(tenant_id, project_id, agent_id);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read state summary at {:?}", path))?;
        Ok(Some(content))
    }

    /// Carga los state summaries de todos los supervisores en el árbol restaurado.
    /// Retorna un map de AgentId → contenido del .md.
    pub fn load_all_summaries(
        &self,
        tenant_id: &str,
        tree: &AgentTree,
    ) -> anyhow::Result<std::collections::HashMap<AgentId, String>> {
        let mut summaries = std::collections::HashMap::new();
        for supervisor in tree.all_supervisors() {
            let project_id = &supervisor.project_id;
            match self.load_state_summary(tenant_id, project_id, &supervisor.agent_id)? {
                Some(content) => {
                    summaries.insert(supervisor.agent_id, content);
                }
                None => {
                    warn!(
                        agent = %supervisor.agent_id,
                        "[AgentPersistence] No state summary found for supervisor — starting fresh."
                    );
                }
            }
        }
        Ok(summaries)
    }

    /// Elimina todos los archivos de persistencia de un proyecto (archivado).
    pub fn delete_project(
        &self,
        tenant_id: &str,
        project_id: &ProjectId,
    ) -> anyhow::Result<()> {
        let dir = self.project_dir(tenant_id, project_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .with_context(|| format!("Failed to delete project dir {:?}", dir))?;
            info!(
                tenant = %tenant_id,
                project = %project_id,
                "[AgentPersistence] Project data deleted."
            );
        }
        Ok(())
    }

    fn ensure_dir(&self, path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)
                .with_context(|| format!("Failed to create directory {:?}", path))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::node::{AgentNode, AgentRole};
    use crate::pcb::TaskType;
    use tempfile::tempdir;

    fn make_tree() -> AgentTree {
        let mut tree = AgentTree::new();
        let root = AgentNode::new(
            AgentRole::ProjectSupervisor {
                name: "Aegis OS".to_string(),
                description: "test".to_string(),
            },
            "aegis".to_string(),
            None,
            "prompt",
            TaskType::Planning,
        );
        let root_id = tree.insert(root).unwrap();
        let domain = AgentNode::new(
            AgentRole::Supervisor {
                name: "Kernel".to_string(),
                scope: "kernel".to_string(),
            },
            "aegis".to_string(),
            Some(root_id),
            "prompt",
            TaskType::Analysis,
        );
        tree.insert(domain).unwrap();
        tree
    }

    #[test]
    fn test_save_and_load_tree() {
        let dir = tempdir().unwrap();
        let persistence = AgentPersistence::new(dir.path());
        let tree = make_tree();

        persistence.save_tree("tenant1", &"aegis".to_string(), &tree).unwrap();
        assert!(persistence.has_saved_tree("tenant1", &"aegis".to_string()));

        let loaded = persistence.load_tree("tenant1", &"aegis".to_string()).unwrap();
        assert!(loaded.is_some());
        let loaded_tree = loaded.unwrap();
        assert_eq!(loaded_tree.len(), tree.len());
    }

    #[test]
    fn test_save_and_load_summary() {
        let dir = tempdir().unwrap();
        let persistence = AgentPersistence::new(dir.path());
        let agent_id = uuid::Uuid::new_v4();
        let summary = "## Estado al 2026-04-26\n\n### Completado\n- Tarea X";

        persistence
            .save_state_summary("tenant1", &"aegis".to_string(), &agent_id, summary)
            .unwrap();

        let loaded = persistence
            .load_state_summary("tenant1", &"aegis".to_string(), &agent_id)
            .unwrap();
        assert_eq!(loaded.unwrap(), summary);
    }

    #[test]
    fn test_load_tree_returns_none_if_not_exists() {
        let dir = tempdir().unwrap();
        let persistence = AgentPersistence::new(dir.path());
        let result = persistence.load_tree("tenant1", &"nonexistent".to_string()).unwrap();
        assert!(result.is_none());
    }
}
