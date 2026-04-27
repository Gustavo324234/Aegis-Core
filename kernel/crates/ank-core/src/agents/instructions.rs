use crate::agents::node::AgentRole;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Nombres de archivo para cada rol (sin extensión).
const CHAT_AGENT_FILE: &str = "chat_agent";
const PROJECT_SUPERVISOR_FILE: &str = "project_supervisor";
const SUPERVISOR_FILE: &str = "supervisor";
const SPECIALIST_FILE: &str = "specialist";

/// Carga las instrucciones de rol desde `kernel/config/agents/*.md` en runtime (CORE-197).
/// Los archivos son editables sin recompilar (ADR-CAA-004).
/// Si un archivo no está disponible, usa un fallback embebido.
pub struct InstructionLoader {
    /// Directorio base donde viven los archivos .md
    config_dir: PathBuf,
    /// Cache en memoria: filename_stem → contenido
    cache: HashMap<String, String>,
}

impl InstructionLoader {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
            cache: HashMap::new(),
        }
    }

    /// Crea un loader con la ruta estándar relativa al workspace de Aegis.
    pub fn default_from_workspace(workspace_root: &Path) -> Self {
        Self::new(workspace_root.join("kernel").join("config").join("agents"))
    }

    /// Precarga todos los archivos de instrucciones al inicializar.
    pub fn preload(&mut self) -> anyhow::Result<()> {
        let files = [
            CHAT_AGENT_FILE,
            PROJECT_SUPERVISOR_FILE,
            SUPERVISOR_FILE,
            SPECIALIST_FILE,
        ];
        for name in &files {
            let path = self.config_dir.join(format!("{}.md", name));
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    info!(
                        "[InstructionLoader] Loaded {}.md ({} chars)",
                        name,
                        content.len()
                    );
                    self.cache.insert(name.to_string(), content);
                }
                Err(e) => {
                    warn!(
                        "[InstructionLoader] Could not load {}.md: {}. Using fallback.",
                        name, e
                    );
                    self.cache
                        .insert(name.to_string(), Self::fallback(name).to_string());
                }
            }
        }
        Ok(())
    }

    /// Retorna las instrucciones para un rol dado.
    /// Si el archivo no está en cache, intenta leerlo del disco.
    /// Si falla, retorna el fallback embebido.
    pub fn instructions_for(&mut self, role: &AgentRole) -> String {
        let key = Self::key_for_role(role);
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }
        // Intentar leer del disco (por si fue editado en runtime)
        let path = self.config_dir.join(format!("{}.md", key));
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.cache.insert(key.clone(), content.clone());
                content
            }
            Err(_) => {
                let fb = Self::fallback(&key).to_string();
                self.cache.insert(key, fb.clone());
                fb
            }
        }
    }

    /// Construye el system prompt para un nodo: instrucciones de rol + contexto de proyecto.
    pub fn build_system_prompt(
        &mut self,
        role: &AgentRole,
        project_id: &str,
        extra_context: Option<&str>,
    ) -> String {
        let instructions = self.instructions_for(role);
        let mut prompt = format!("# Proyecto: {}\n\n{}\n", project_id, instructions);
        if let Some(ctx) = extra_context {
            prompt.push_str("\n---\n\n## Contexto previo\n\n");
            prompt.push_str(ctx);
            prompt.push('\n');
        }
        prompt
    }

    fn key_for_role(role: &AgentRole) -> String {
        match role {
            AgentRole::ChatAgent => CHAT_AGENT_FILE.to_string(),
            AgentRole::ProjectSupervisor { .. } => PROJECT_SUPERVISOR_FILE.to_string(),
            AgentRole::Supervisor { .. } => SUPERVISOR_FILE.to_string(),
            AgentRole::Specialist { .. } => SPECIALIST_FILE.to_string(),
        }
    }

    /// Instrucciones de fallback embebidas — garantizan funcionalidad básica
    /// si los archivos .md no están disponibles.
    fn fallback(key: &str) -> &'static str {
        match key {
            CHAT_AGENT_FILE => {
                "Sos el Chat Agent de Aegis OS. Conversás con el usuario, \
                 delegás trabajo a los supervisores y respondés con claridad."
            }
            PROJECT_SUPERVISOR_FILE => {
                "Sos un Project Supervisor de Aegis OS. Coordinás el trabajo \
                 de un proyecto, delegás a supervisores de dominio y consolidás resultados."
            }
            SUPERVISOR_FILE => {
                "Sos un Supervisor de dominio en Aegis OS. Coordinás tu área, \
                 delegás a specialists y reportás al supervisor padre."
            }
            SPECIALIST_FILE => {
                "Sos un Specialist Agent de Aegis OS. Ejecutás una tarea atómica \
                 específica y reportás el resultado. No creás sub-agentes."
            }
            _ => "Agente de Aegis OS. Ejecutá tu tarea y reportá el resultado.",
        }
    }
}

/// Template para el state summary generado por supervisores al cerrar sesión (CORE-207).
pub fn state_summary_template(fecha: &str) -> String {
    format!(
        "## Estado al {fecha}\n\n\
         ### Completado\n\
         \n\
         ### En progreso\n\
         \n\
         ### Decisiones tomadas\n\
         \n\
         ### Pendiente\n\
         \n\
         ### Sub-supervisores y specialists activos\n\
         \n\
         ### Contexto importante\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_fallback_for_all_roles() {
        let roles = [
            AgentRole::ChatAgent,
            AgentRole::ProjectSupervisor {
                name: "p".into(),
                description: "d".into(),
            },
            AgentRole::Supervisor {
                name: "s".into(),
                scope: "sc".into(),
            },
            AgentRole::Specialist { scope: "sp".into() },
        ];
        let mut loader = InstructionLoader::new("/nonexistent/path");
        for role in &roles {
            let instructions = loader.instructions_for(role);
            assert!(!instructions.is_empty());
        }
    }

    #[test]
    fn test_load_from_disk() {
        let dir = tempdir().unwrap();
        let content = "# Test Supervisor\nEjecutá tu tarea.";
        let path = dir.path().join("supervisor.md");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();

        let mut loader = InstructionLoader::new(dir.path());
        let role = AgentRole::Supervisor {
            name: "Test".into(),
            scope: "test scope".into(),
        };
        let instructions = loader.instructions_for(&role);
        assert_eq!(instructions.trim(), content.trim());
    }

    #[test]
    fn test_build_system_prompt_includes_project() {
        let mut loader = InstructionLoader::new("/nonexistent");
        let role = AgentRole::Specialist {
            scope: "leer mod.rs".into(),
        };
        let prompt = loader.build_system_prompt(&role, "aegis-os", None);
        assert!(prompt.contains("aegis-os"));
    }

    #[test]
    fn test_state_summary_template_contains_sections() {
        let template = state_summary_template("2026-04-26");
        assert!(template.contains("Completado"));
        assert!(template.contains("En progreso"));
        assert!(template.contains("Decisiones tomadas"));
        assert!(template.contains("Pendiente"));
        assert!(template.contains("Contexto importante"));
        assert!(template.contains("2026-04-26"));
    }
}
