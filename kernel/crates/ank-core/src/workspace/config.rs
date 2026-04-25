use crate::enclave::TenantDB;
use serde::{Deserialize, Serialize};

/// Helpers de configuración del workspace. Todos los métodos son sincrónicos
/// y operan sobre una TenantDB ya abierta.
pub struct WorkspaceConfig;

impl WorkspaceConfig {
    pub fn get(db: &TenantDB, key: &str) -> anyhow::Result<Option<String>> {
        db.workspace_config_get(key)
    }

    pub fn set(db: &TenantDB, key: &str, value: &str) -> anyhow::Result<()> {
        db.workspace_config_set(key, value)
    }

    /// Carga toda la configuración del workspace como struct tipado.
    /// Los campos faltantes retornan sus valores por defecto.
    pub fn load_all(db: &TenantDB) -> anyhow::Result<WorkspaceSettings> {
        let github_token = db.workspace_config_get("github_token")?;
        let project_root = db.workspace_config_get("project_root")?;
        let github_repo = db.workspace_config_get("github_repo")?;

        let terminal_allowlist = db
            .workspace_config_get("terminal_allowlist")?
            .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
            .unwrap_or_else(default_allowlist);

        let pr_merge_mode = db
            .workspace_config_get("pr_merge_mode")?
            .map(|v| match v.as_str() {
                "automatic" => MergeMode::Automatic,
                _ => MergeMode::Manual,
            })
            .unwrap_or_default();

        let pr_auto_fix_ci = db
            .workspace_config_get("pr_auto_fix_ci")?
            .map(|v| v == "true")
            .unwrap_or(true);

        Ok(WorkspaceSettings {
            github_token,
            project_root,
            github_repo,
            terminal_allowlist,
            pr_merge_mode,
            pr_auto_fix_ci,
        })
    }
}

fn default_allowlist() -> Vec<String> {
    vec![
        "cargo".to_string(),
        "npm".to_string(),
        "git".to_string(),
        "python".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    /// None si no está configurado. NUNCA se expone en APIs — ver WorkspaceSettingsDto.
    #[serde(skip_serializing)]
    pub github_token: Option<String>,
    pub project_root: Option<String>,
    pub github_repo: Option<String>,
    pub terminal_allowlist: Vec<String>,
    pub pr_merge_mode: MergeMode,
    pub pr_auto_fix_ci: bool,
}

/// DTO para serializar al frontend: el token nunca aparece — solo "configured" o null.
#[derive(Debug, Serialize)]
pub struct WorkspaceSettingsDto {
    pub github_token_status: Option<&'static str>,
    pub project_root: Option<String>,
    pub github_repo: Option<String>,
    pub terminal_allowlist: Vec<String>,
    pub pr_merge_mode: String,
    pub pr_auto_fix_ci: bool,
}

impl From<WorkspaceSettings> for WorkspaceSettingsDto {
    fn from(s: WorkspaceSettings) -> Self {
        Self {
            github_token_status: if s.github_token.is_some() {
                Some("configured")
            } else {
                None
            },
            project_root: s.project_root,
            github_repo: s.github_repo,
            terminal_allowlist: s.terminal_allowlist,
            pr_merge_mode: match s.pr_merge_mode {
                MergeMode::Automatic => "automatic".to_string(),
                MergeMode::Manual => "manual".to_string(),
            },
            pr_auto_fix_ci: s.pr_auto_fix_ci,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MergeMode {
    Automatic,
    #[default]
    Manual,
}
