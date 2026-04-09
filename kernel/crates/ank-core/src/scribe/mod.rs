pub mod diagnostic;
use git2::{Repository, Signature};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::info;

/// --- SCRIBE ERROR SYSTEM ---
#[derive(Error, Debug)]
pub enum ScribeError {
    #[error("Git Repository Error: {0}")]
    GitError(String),
    #[error("Missing Commit Metadata: {0}")]
    MissingMetadata(String),
    #[error("File Write Error: {0} - {1}")]
    FileWriteError(String, String),
    #[error("Invalid Operation: {0}")]
    InvalidOperation(String),
    #[error("Audit Failure: Content verification failed")]
    AuditFailure,
    #[error("Security Violation: {0}")]
    SecurityViolation(String),
}

/// --- COMMIT METADATA ---
/// Estándar obligatorio para toda escritura realizada por el Kernel.
/// Permite la reconstrucción histórica del estado cognitivo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitMetadata {
    pub task_id: String,
    pub version_increment: VersionType,
    pub summary: String,
    pub impact: ImpactLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// --- SCRIBE MANAGER ---
/// Responsable del versionado y auditoría del sistema de archivos local.
/// Garantiza que no se realicen cambios sin dejar rastro (The Scribe).
pub struct ScribeManager {
    root_path: String,
    /// Lock crítico para sincronizar operaciones de Git.
    /// libgit2 no permite accesos concurrentes al archivo index.lock.
    commit_lock: Arc<Mutex<()>>,
}

impl ScribeManager {
    pub fn new(root: &str) -> Self {
        Self {
            root_path: root.to_string(),
            commit_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Calcula la ruta física del workspace de un tenant.
    fn compute_tenant_path(&self, tenant_id: &str) -> String {
        format!("{}/{}/workspace", self.root_path, tenant_id)
    }

    /// Inicializa un repositorio Git local en el workspace del tenant si no existe.
    pub async fn init_repo(&self, tenant_id: &str) -> Result<(), ScribeError> {
        let _lock = self.commit_lock.lock().await;
        let tenant_path = self.compute_tenant_path(tenant_id);
        let path = Path::new(&tenant_path);

        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| ScribeError::FileWriteError(tenant_path.clone(), e.to_string()))?;
        }

        if !self.is_initialized(tenant_id) {
            info!("Initializing new Git repository at {}", tenant_path);
            Repository::init(path).map_err(|e| ScribeError::GitError(e.to_string()))?;
        } else {
            info!("Git repository already exists at {}", tenant_path);
        }

        Ok(())
    }

    /// Escribe contenido en un archivo del tenant y realiza un commit atómico.
    pub async fn write_and_commit(
        &self,
        tenant_id: &str,
        file_path: &str,
        content: &[u8],
        metadata: CommitMetadata,
    ) -> Result<(), ScribeError> {
        if metadata.summary.is_empty() {
            return Err(ScribeError::MissingMetadata(
                "Summary cannot be empty".into(),
            ));
        }

        // SECURITY: Enforce jailing at the Scribe level regardless of caller.
        // This is the last line of defense before any filesystem write.
        if !crate::vcm::is_safe_path(tenant_id, file_path) {
            return Err(ScribeError::SecurityViolation(format!(
                "Path '{}' escapes tenant workspace for '{}'",
                file_path, tenant_id
            )));
        }

        let tenant_path = self.compute_tenant_path(tenant_id);

        // Adquirimos el lock antes de cualquier operación de Git o IO
        // para garantizar que la 'transacción' sea exclusiva.
        let _lock = self.commit_lock.lock().await;

        let repo = Repository::open(&tenant_path)
            .map_err(|e| ScribeError::GitError(format!("Failed to open repo: {}", e)))?;

        // 2. Escritura física asíncrona - Aseguramos que el directorio exista
        let full_path = Path::new(&tenant_path).join(file_path);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| ScribeError::FileWriteError(file_path.to_string(), e.to_string()))?;

        // 3. Fase de Git (Index & Commit)
        self.commit_changes(&repo, file_path, &metadata)?;

        info!(
            task_id = %metadata.task_id,
            file = %file_path,
            "Scribe: File written and committed successfully."
        );
        Ok(())
    }

    /// Realiza un hard reset del workspace del tenant.
    pub async fn hard_reset(&self, tenant_id: &str, commit_hash: &str) -> Result<(), ScribeError> {
        let tenant_path = self.compute_tenant_path(tenant_id);
        let _lock = self.commit_lock.lock().await;
        let repo =
            Repository::open(&tenant_path).map_err(|e| ScribeError::GitError(e.to_string()))?;

        let obj = repo
            .revparse_single(commit_hash)
            .map_err(|e| ScribeError::InvalidOperation(format!("Commit not found: {}", e)))?;

        repo.reset(&obj, git2::ResetType::Hard, None)
            .map_err(|e| ScribeError::GitError(format!("Reset failed: {}", e)))?;

        info!(hash = %commit_hash, "Scribe: Hard reset performed.");
        Ok(())
    }

    /// Lógica interna para realizar el commit siguiendo SRE standards.
    fn commit_changes(
        &self,
        repo: &Repository,
        rel_path: &str,
        metadata: &CommitMetadata,
    ) -> Result<(), ScribeError> {
        let mut index = repo
            .index()
            .map_err(|e| ScribeError::GitError(e.to_string()))?;

        // Add specific file to index
        index
            .add_path(Path::new(rel_path))
            .map_err(|e| ScribeError::GitError(format!("Add to index failed: {}", e)))?;
        index
            .write()
            .map_err(|e| ScribeError::GitError(e.to_string()))?;

        let tree_id = index
            .write_tree()
            .map_err(|e| ScribeError::GitError(e.to_string()))?;
        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| ScribeError::GitError(e.to_string()))?;

        // Autor virtual ANK
        let signature = Signature::now("ANK Scribe", "ank@aegis.ia")
            .map_err(|e| ScribeError::GitError(e.to_string()))?;

        // Buscar padre (parent) si existe
        let mut parents = Vec::new();
        if let Ok(head) = repo.head() {
            if let Ok(parent) = head.peel_to_commit() {
                parents.push(parent);
            }
        }

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &metadata.summary,
            &tree,
            &parent_refs,
        )
        .map_err(|e| ScribeError::GitError(format!("Commit failed: {}", e)))?;

        Ok(())
    }

    /// Verifica si el directorio del tenant está bajo control de The Scribe.
    fn is_initialized(&self, tenant_id: &str) -> bool {
        let tenant_path = self.compute_tenant_path(tenant_id);
        Path::new(&tenant_path).join(".git").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_scribe_init_and_commit() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let root = dir.path().to_str().context("Tempdir path is not UTF-8")?;
        let scribe = ScribeManager::new(root);
        let tenant_id = "user_123";

        // 1. Init
        scribe.init_repo(tenant_id).await?;
        assert!(scribe.is_initialized(tenant_id));

        // 2. Commit
        let metadata = CommitMetadata {
            task_id: "test-1".into(),
            version_increment: VersionType::Patch,
            summary: "Initial test commit".into(),
            impact: ImpactLevel::Low,
        };

        let tenant_path = scribe.compute_tenant_path(tenant_id);
        std::fs::create_dir_all(Path::new(&tenant_path)).unwrap_or_default();

        scribe
            .write_and_commit(tenant_id, "test.txt", b"Hello Scribe", metadata.clone())
            .await
            .context("Failed to write and commit")?;

        // 3. Verify
        let tenant_path = scribe.compute_tenant_path(tenant_id);
        let repo =
            Repository::open(&tenant_path).context("Failed to open repo for verification")?;
        let head = repo.head().context("Failed to get HEAD")?;
        let commit = head.peel_to_commit().context("Failed to peel to commit")?;

        assert_eq!(commit.message(), Some("Initial test commit"));
        assert_eq!(commit.author().name(), Some("ANK Scribe"));

        // Check file exists
        let content = std::fs::read_to_string(Path::new(&tenant_path).join("test.txt"))
            .context("Failed to read committed file")?;
        assert_eq!(content, "Hello Scribe");
        Ok(())
    }

    #[tokio::test]
    async fn test_scribe_error_on_empty_metadata() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let root = dir.path().to_str().context("Tempdir path is not UTF-8")?;
        let scribe = ScribeManager::new(root);
        let tenant_id = "user_456";
        scribe.init_repo(tenant_id).await?;

        let metadata = CommitMetadata {
            task_id: "test-2".into(),
            version_increment: VersionType::Patch,
            summary: "".into(), // Inválido
            impact: ImpactLevel::Low,
        };

        let result = scribe
            .write_and_commit(tenant_id, "error.txt", b"fails", metadata)
            .await;
        assert!(matches!(result, Err(ScribeError::MissingMetadata(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_scribe_blocks_path_traversal() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let root = dir.path().to_str().context("Tempdir path is not UTF-8")?;
        let scribe = ScribeManager::new(root);
        let tenant_id = "user_sec";
        scribe.init_repo(tenant_id).await?;

        let metadata = CommitMetadata {
            task_id: "sec-test".into(),
            version_increment: VersionType::Patch,
            summary: "Path traversal attempt".into(),
            impact: ImpactLevel::Critical,
        };

        let result = scribe
            .write_and_commit(tenant_id, "../../etc/passwd", b"malicious", metadata)
            .await;

        let Err(e) = result else {
            anyhow::bail!("Expected write_and_commit to return Err for path traversal, but got Ok");
        };
        assert!(
            matches!(e, ScribeError::SecurityViolation(_)),
            "Expected SecurityViolation, got: {:?}",
            e
        );
        Ok(())
    }
}
