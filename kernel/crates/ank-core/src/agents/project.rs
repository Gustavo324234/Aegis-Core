use crate::agents::node::ProjectId;
use crate::agents::persistence::AgentPersistence;
use crate::agents::tree::AgentTree;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Estado del proyecto — permite archivar sin borrar datos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Archived,
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Active => write!(f, "active"),
            ProjectStatus::Archived => write!(f, "archived"),
        }
    }
}

/// Metadatos de un proyecto persistidos en SQLite (tabla `projects`).
/// El árbol y los contextos viven en el filesystem (ADR-CAA-013).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub project_id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProjectMetadata {
    pub fn new(project_id: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            project_id: project_id.into(),
            name: name.into(),
            description: None,
            status: ProjectStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn is_active(&self) -> bool {
        self.status == ProjectStatus::Active
    }
}

/// Registro de proyectos del tenant.
/// Los metadatos van en SQLite (tabla `projects`).
/// El árbol y los state summaries van en el filesystem via AgentPersistence.
pub struct ProjectRegistry {
    known_projects: HashMap<ProjectId, ProjectMetadata>,
    db_path: String,
    session_key: String,
    persistence: AgentPersistence,
    tenant_id: String,
}

impl ProjectRegistry {
    pub fn new(tenant_id: &str, session_key: &str) -> Self {
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        Self {
            known_projects: HashMap::new(),
            db_path: format!("{}/users/{}/memory.db", base_dir, tenant_id),
            session_key: session_key.to_string(),
            persistence: AgentPersistence::from_env(),
            tenant_id: tenant_id.to_string(),
        }
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Carga proyectos desde SQLite al iniciar sesión.
    /// Migración additive: crea la tabla `projects` si no existe.
    /// La tabla `agent_projects` de Epic 43 se mantiene para no romper datos existentes.
    pub async fn load_from_db(&mut self) -> anyhow::Result<()> {
        let db_path = self.db_path.clone();
        let session_key = self.session_key.clone();
        let tenant_id = self.tenant_id.clone();

        let rows = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<ProjectMetadata>> {
            let conn = rusqlite::Connection::open(&db_path)
                .with_context(|| format!("Failed to open db for tenant {}", tenant_id))?;

            conn.execute_batch(&format!("PRAGMA key = '{}';", session_key))
                .ok();

            // Tabla `projects` — schema canónico de Epic 45
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS projects (
                    project_id   TEXT PRIMARY KEY,
                    name         TEXT NOT NULL,
                    description  TEXT,
                    status       TEXT NOT NULL DEFAULT 'active',
                    created_at   TEXT NOT NULL,
                    updated_at   TEXT NOT NULL
                );",
            )
            .context("Failed to create projects table")?;

            let mut stmt = conn
                .prepare(
                    "SELECT project_id, name, description, status, created_at, updated_at
                     FROM projects ORDER BY updated_at DESC",
                )
                .context("Failed to prepare projects query")?;

            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })
                .context("Failed to query projects")?
                .filter_map(|r| r.ok())
                .filter_map(|(id, name, desc, status, created, updated)| {
                    let created_at = created.parse::<DateTime<Utc>>().ok()?;
                    let updated_at = updated.parse::<DateTime<Utc>>().ok()?;
                    let status = match status.as_str() {
                        "archived" => ProjectStatus::Archived,
                        _ => ProjectStatus::Active,
                    };
                    Some(ProjectMetadata {
                        project_id: id,
                        name,
                        description: desc,
                        status,
                        created_at,
                        updated_at,
                    })
                })
                .collect();

            Ok(rows)
        })
        .await
        .context("spawn_blocking failed in load_from_db")??;

        for meta in rows {
            info!(project_id = %meta.project_id, "[ProjectRegistry] Loaded project '{}'.", meta.name);
            self.known_projects.insert(meta.project_id.clone(), meta);
        }
        Ok(())
    }

    pub fn get(&self, project_id: &ProjectId) -> Option<&ProjectMetadata> {
        self.known_projects.get(project_id)
    }

    /// Crea o actualiza un proyecto en memoria y en SQLite.
    pub async fn upsert(&mut self, metadata: ProjectMetadata) -> anyhow::Result<()> {
        let db_path = self.db_path.clone();
        let session_key = self.session_key.clone();
        let meta = metadata.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = rusqlite::Connection::open(&db_path).context("Failed to open db")?;
            conn.execute_batch(&format!("PRAGMA key = '{}';", session_key)).ok();

            conn.execute(
                "INSERT INTO projects (project_id, name, description, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(project_id) DO UPDATE SET
                    name        = excluded.name,
                    description = excluded.description,
                    status      = excluded.status,
                    updated_at  = excluded.updated_at;",
                rusqlite::params![
                    meta.project_id,
                    meta.name,
                    meta.description,
                    meta.status.to_string(),
                    meta.created_at.to_rfc3339(),
                    meta.updated_at.to_rfc3339(),
                ],
            )
            .context("Failed to upsert project")?;
            Ok(())
        })
        .await
        .context("spawn_blocking failed in upsert")??;

        self.known_projects
            .insert(metadata.project_id.clone(), metadata);
        Ok(())
    }

    /// Archiva un proyecto (cambia status a Archived).
    pub async fn archive(&mut self, project_id: &ProjectId) -> anyhow::Result<()> {
        let meta = self
            .known_projects
            .get_mut(project_id)
            .ok_or_else(|| anyhow::anyhow!("Project {} not found", project_id))?;
        meta.status = ProjectStatus::Archived;
        meta.updated_at = Utc::now();
        let meta_clone = meta.clone();
        self.upsert(meta_clone).await?;
        Ok(())
    }

    pub fn list_active(&self) -> Vec<&ProjectMetadata> {
        let mut list: Vec<&ProjectMetadata> = self
            .known_projects
            .values()
            .filter(|m| m.is_active())
            .collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        list
    }

    pub fn search_by_name(&self, query: &str) -> Vec<&ProjectMetadata> {
        let q = query.to_lowercase();
        self.known_projects
            .values()
            .filter(|m| m.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn touch(&mut self, project_id: &ProjectId) {
        if let Some(meta) = self.known_projects.get_mut(project_id) {
            meta.updated_at = Utc::now();
        }
    }

    // --- Persistencia del árbol (delegada a AgentPersistence) ---

    /// Guarda el árbol de un proyecto en el filesystem.
    pub fn save_tree(&self, project_id: &ProjectId, tree: &AgentTree) -> anyhow::Result<()> {
        self.persistence
            .save_tree(&self.tenant_id, project_id, tree)
    }

    /// Carga el árbol de un proyecto desde el filesystem. Retorna `None` si no existe.
    pub fn load_tree(&self, project_id: &ProjectId) -> anyhow::Result<Option<AgentTree>> {
        self.persistence.load_tree(&self.tenant_id, project_id)
    }

    pub fn has_saved_tree(&self, project_id: &ProjectId) -> bool {
        self.persistence.has_saved_tree(&self.tenant_id, project_id)
    }

    pub fn persistence(&self) -> &AgentPersistence {
        &self.persistence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_metadata_new() {
        let meta = ProjectMetadata::new("aegis", "Aegis OS")
            .with_description("Sistema operativo cognitivo");
        assert_eq!(meta.project_id, "aegis");
        assert_eq!(meta.name, "Aegis OS");
        assert_eq!(meta.status, ProjectStatus::Active);
        assert!(meta.description.is_some());
        assert!(meta.is_active());
    }

    #[test]
    fn test_search_by_name() {
        let mut registry = ProjectRegistry::new("test_tenant", "test_key");
        registry.known_projects.insert(
            "aegis".to_string(),
            ProjectMetadata::new("aegis", "Aegis OS"),
        );
        registry.known_projects.insert(
            "shopping".to_string(),
            ProjectMetadata::new("shopping", "Lista de compras"),
        );

        let results = registry.search_by_name("aegis");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].project_id, "aegis");

        let all = registry.search_by_name("");
        assert_eq!(all.len(), 2);
    }
}
