use crate::agents::node::ProjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Metadatos persistentes de un proyecto en el enclave del tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub project_id: ProjectId,
    pub display_name: String,
    pub description: String,
    /// Última vez que tuvo un ProjectSupervisor activo.
    pub last_active: DateTime<Utc>,
    /// System prompt base para el ProjectSupervisor de este proyecto.
    pub supervisor_prompt: String,
    /// Dominios conocidos de este proyecto (se agregan dinámicamente).
    pub known_domains: Vec<String>,
}

/// Registro de proyectos del tenant — persistidos en SQLCipher, cargados en memoria.
pub struct ProjectRegistry {
    known_projects: HashMap<ProjectId, ProjectMetadata>,
    /// Ruta a la base de datos del tenant (./users/{tenant_id}/memory.db).
    db_path: String,
    /// Session key para descifrar el enclave SQLCipher del tenant.
    session_key: String,
}

impl ProjectRegistry {
    pub fn new(tenant_id: &str, session_key: &str) -> Self {
        Self {
            known_projects: HashMap::new(),
            db_path: format!("./users/{}/memory.db", tenant_id),
            session_key: session_key.to_string(),
        }
    }

    /// Carga proyectos desde el enclave al iniciar sesión.
    /// Crea la tabla `agent_projects` si no existe (migración additive).
    pub async fn load_from_enclave(&mut self, tenant_id: &str) -> anyhow::Result<()> {
        use anyhow::Context;

        let db_path = self.db_path.clone();
        let session_key = self.session_key.clone();
        let tenant_id = tenant_id.to_string();

        let rows = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<ProjectMetadata>> {
            let conn = rusqlite::Connection::open(&db_path)
                .with_context(|| format!("Failed to open enclave for tenant {}", tenant_id))?;

            // Aplicar la key SQLCipher
            conn.execute_batch(&format!("PRAGMA key = '{}';", session_key))
                .ok(); // No falla si SQLCipher no está disponible (dev mode)

            // Migración additive: crear tabla si no existe
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS agent_projects (
                    project_id       TEXT PRIMARY KEY,
                    display_name     TEXT NOT NULL,
                    description      TEXT NOT NULL DEFAULT '',
                    last_active      TEXT NOT NULL,
                    supervisor_prompt TEXT NOT NULL DEFAULT '',
                    known_domains    TEXT NOT NULL DEFAULT '[]'
                );",
            )
            .with_context(|| "Failed to create agent_projects table")?;

            let mut stmt = conn
                .prepare(
                    "SELECT project_id, display_name, description, last_active,
                            supervisor_prompt, known_domains
                     FROM agent_projects ORDER BY last_active DESC",
                )
                .with_context(|| "Failed to prepare agent_projects query")?;

            let rows: Vec<ProjectMetadata> = stmt
                .query_map([], |row| {
                    let last_active_str: String = row.get(3)?;
                    let domains_json: String = row.get(5)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        last_active_str,
                        row.get::<_, String>(4)?,
                        domains_json,
                    ))
                })
                .with_context(|| "Failed to query agent_projects")?
                .filter_map(|r| r.ok())
                .filter_map(|(pid, name, desc, last_active_str, prompt, domains_json)| {
                    let last_active = last_active_str.parse::<DateTime<Utc>>().ok()?;
                    let known_domains: Vec<String> =
                        serde_json::from_str(&domains_json).unwrap_or_default();
                    Some(ProjectMetadata {
                        project_id: pid,
                        display_name: name,
                        description: desc,
                        last_active,
                        supervisor_prompt: prompt,
                        known_domains,
                    })
                })
                .collect();

            Ok(rows)
        })
        .await
        .with_context(|| "spawn_blocking failed in load_from_enclave")??;

        for meta in rows {
            info!(project_id = %meta.project_id, "Loaded project from enclave.");
            self.known_projects.insert(meta.project_id.clone(), meta);
        }

        Ok(())
    }

    pub fn get(&self, project_id: &ProjectId) -> Option<&ProjectMetadata> {
        self.known_projects.get(project_id)
    }

    /// Registra un proyecto nuevo o actualiza uno existente en memoria y en SQLCipher.
    pub async fn upsert(
        &mut self,
        metadata: ProjectMetadata,
        _tenant_id: &str,
    ) -> anyhow::Result<()> {
        use anyhow::Context;

        let db_path = self.db_path.clone();
        let session_key = self.session_key.clone();
        let meta = metadata.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = rusqlite::Connection::open(&db_path)
                .with_context(|| "Failed to open enclave for upsert")?;

            conn.execute_batch(&format!("PRAGMA key = '{}';", session_key))
                .ok();

            let domains_json = serde_json::to_string(&meta.known_domains)
                .unwrap_or_else(|_| "[]".to_string());

            conn.execute(
                "INSERT INTO agent_projects
                    (project_id, display_name, description, last_active, supervisor_prompt, known_domains)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(project_id) DO UPDATE SET
                    display_name     = excluded.display_name,
                    description      = excluded.description,
                    last_active      = excluded.last_active,
                    supervisor_prompt = excluded.supervisor_prompt,
                    known_domains    = excluded.known_domains;",
                rusqlite::params![
                    meta.project_id,
                    meta.display_name,
                    meta.description,
                    meta.last_active.to_rfc3339(),
                    meta.supervisor_prompt,
                    domains_json,
                ],
            )
            .with_context(|| "Failed to upsert project metadata")?;

            Ok(())
        })
        .await
        .with_context(|| "spawn_blocking failed in upsert")??;

        self.known_projects
            .insert(metadata.project_id.clone(), metadata);

        Ok(())
    }

    /// Busca proyectos cuyo nombre contenga el query (case-insensitive).
    /// Usado para resolución nombre → ProjectId desde el input del usuario.
    pub fn search_by_name(&self, query: &str) -> Vec<&ProjectMetadata> {
        let q = query.to_lowercase();
        self.known_projects
            .values()
            .filter(|m| m.display_name.to_lowercase().contains(&q))
            .collect()
    }

    /// Actualiza last_active y opcionalmente añade un nuevo dominio conocido.
    pub async fn touch(
        &mut self,
        project_id: &ProjectId,
        new_domain: Option<String>,
        tenant_id: &str,
    ) -> anyhow::Result<()> {
        let meta = self
            .known_projects
            .get_mut(project_id)
            .ok_or_else(|| anyhow::anyhow!("Project {} not found in registry", project_id))?;

        meta.last_active = Utc::now();

        if let Some(domain) = new_domain {
            if !meta.known_domains.contains(&domain) {
                meta.known_domains.push(domain);
            }
        }

        let meta_clone = meta.clone();
        self.upsert(meta_clone, tenant_id).await?;

        Ok(())
    }

    /// Lista todos los proyectos conocidos, ordenados por last_active descendente.
    pub fn list_recent(&self) -> Vec<&ProjectMetadata> {
        let mut list: Vec<&ProjectMetadata> = self.known_projects.values().collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.last_active));
        list
    }
}
