use crate::pcb::TaskType;
use crate::scheduler::ModelPreference;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub type AgentId = Uuid;
pub type ProjectId = String;

/// Rol jerárquico del agente dentro del árbol de orquestación.
/// Las variantes con datos capturan el scope declarado al momento del spawn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Nivel 0 — exactamente 1 por tenant. Conversa con el usuario.
    /// No persiste el árbol, solo el historial resumido de la sesión.
    ChatAgent,

    /// Nivel 1 — 1 por proyecto activo. Coordinador raíz del proyecto.
    ProjectSupervisor { name: String, description: String },

    /// Nivel 2..N — supervisores de dominio, profundidad ilimitada.
    Supervisor { name: String, scope: String },

    /// Nivel hoja — ejecutor atómico. Efímero, no persiste.
    Specialist { scope: String },
}

impl AgentRole {
    /// TaskType por defecto según el rol. El padre puede hacer override al spawear.
    pub fn default_task_type(&self) -> TaskType {
        match self {
            AgentRole::ChatAgent => TaskType::Chat,
            AgentRole::ProjectSupervisor { .. } => TaskType::Planning,
            AgentRole::Supervisor { .. } => TaskType::Analysis,
            AgentRole::Specialist { .. } => TaskType::Code,
        }
    }

    /// ModelPreference por defecto según el rol (ADR-CAA-012).
    pub fn default_model_preference(&self) -> ModelPreference {
        match self {
            // Chat Agent necesita baja latencia — siempre cloud
            AgentRole::ChatAgent => ModelPreference::CloudOnly,
            AgentRole::ProjectSupervisor { .. } => ModelPreference::HybridSmart,
            AgentRole::Supervisor { .. } => ModelPreference::HybridSmart,
            AgentRole::Specialist { .. } => ModelPreference::HybridSmart,
        }
    }

    pub fn is_specialist(&self) -> bool {
        matches!(self, AgentRole::Specialist { .. })
    }

    pub fn is_supervisor(&self) -> bool {
        matches!(
            self,
            AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. }
        )
    }

    pub fn display_name(&self) -> &str {
        match self {
            AgentRole::ChatAgent => "Chat Agent",
            AgentRole::ProjectSupervisor { name, .. } => name.as_str(),
            AgentRole::Supervisor { name, .. } => name.as_str(),
            AgentRole::Specialist { scope } => scope.as_str(),
        }
    }
}

/// Estado del ciclo de vida del agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Running,
    /// Esperando que sus subordinados terminen.
    WaitingReport,
    Complete,
    Failed {
        reason: String,
    },
}

/// Nodo del árbol de agentes cognitivos — extendido con campos de persistencia (CORE-190).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub state: AgentState,
    pub parent_id: Option<AgentId>,
    pub children: Vec<AgentId>,

    pub project_id: ProjectId,

    /// System prompt base del agente. Cargado desde kernel/config/agents/*.md por InstructionLoader.
    pub system_prompt: String,

    /// Tipo cognitivo de la tarea — usado por el CMR (ADR-CAA-012).
    pub task_type: TaskType,

    /// Preferencia de modelo — usada por el CMR para selección (ADR-CAA-012).
    pub model_preference: ModelPreference,

    /// Presupuesto de tokens para el contexto de este agente (ADR-CAA-009).
    pub context_budget: usize,

    /// Último reporte consolidado recibido de hijos o generado por este nodo.
    /// Usado por el Chat Agent para responder preguntas sin lanzar una Query.
    pub last_report: Option<String>,

    /// PCB del proceso activo en el scheduler vinculado a este agente.
    pub pcb_id: Option<String>,

    pub created_at: DateTime<Utc>,

    // --- Persistencia (ADR-CAA-005v2) ---
    /// Path al archivo .md de state summary en agent_contexts/{agent_id}.md
    /// None para Specialists (efímeros) y ChatAgent.
    pub persisted_context_path: Option<PathBuf>,

    /// true si este nodo fue cargado desde disk al restaurar un proyecto.
    pub is_restored: bool,

    /// true si el proveedor del agente no soporta tool use (CORE-237).
    /// En modo degradado el Orchestrator no inyecta herramientas y solo asigna tareas atómicas.
    pub is_degraded: bool,
}

impl AgentNode {
    pub fn new(
        role: AgentRole,
        project_id: ProjectId,
        parent_id: Option<AgentId>,
        system_prompt: impl Into<String>,
        task_type: TaskType,
    ) -> Self {
        let model_preference = role.default_model_preference();
        let _is_specialist = role.is_specialist();

        Self {
            agent_id: Uuid::new_v4(),
            role,
            state: AgentState::Idle,
            parent_id,
            children: Vec::new(),
            project_id,
            system_prompt: system_prompt.into(),
            task_type,
            model_preference,
            context_budget: 8192,
            last_report: None,
            pcb_id: None,
            created_at: Utc::now(),
            // Specialists son efímeros — no persisten
            persisted_context_path: None, // se asigna por AgentPersistence
            is_restored: false,
            is_degraded: false,
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    pub fn add_child(&mut self, child_id: AgentId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    pub fn set_state(&mut self, state: AgentState) {
        self.state = state;
    }

    pub fn set_last_report(&mut self, report: String) {
        self.last_report = Some(report);
    }

    pub fn set_pcb_id(&mut self, pcb_id: String) {
        self.pcb_id = Some(pcb_id);
    }

    pub fn set_persisted_context_path(&mut self, path: PathBuf) {
        self.persisted_context_path = Some(path);
    }

    pub fn should_persist(&self) -> bool {
        !self.role.is_specialist() && !matches!(self.role, AgentRole::ChatAgent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_supervisor() -> AgentNode {
        AgentNode::new(
            AgentRole::Supervisor {
                name: "Kernel".to_string(),
                scope: "módulos del kernel".to_string(),
            },
            "aegis".to_string(),
            None,
            "prompt",
            TaskType::Analysis,
        )
    }

    #[test]
    fn test_default_task_type() {
        assert_eq!(AgentRole::ChatAgent.default_task_type(), TaskType::Chat);
        assert_eq!(
            AgentRole::ProjectSupervisor {
                name: "p".into(),
                description: "d".into()
            }
            .default_task_type(),
            TaskType::Planning
        );
        assert_eq!(
            AgentRole::Supervisor {
                name: "s".into(),
                scope: "sc".into()
            }
            .default_task_type(),
            TaskType::Analysis
        );
        assert_eq!(
            AgentRole::Specialist { scope: "sp".into() }.default_task_type(),
            TaskType::Code
        );
    }

    #[test]
    fn test_default_model_preference() {
        assert_eq!(
            AgentRole::ChatAgent.default_model_preference(),
            ModelPreference::CloudOnly
        );
        assert_eq!(
            AgentRole::Specialist { scope: "x".into() }.default_model_preference(),
            ModelPreference::HybridSmart
        );
    }

    #[test]
    fn test_should_persist() {
        let supervisor = make_supervisor();
        assert!(supervisor.should_persist());

        let specialist = AgentNode::new(
            AgentRole::Specialist {
                scope: "leer archivo".to_string(),
            },
            "aegis".to_string(),
            None,
            "prompt",
            TaskType::Code,
        );
        assert!(!specialist.should_persist());

        let chat = AgentNode::new(
            AgentRole::ChatAgent,
            "aegis".to_string(),
            None,
            "prompt",
            TaskType::Chat,
        );
        assert!(!chat.should_persist());
    }

    #[test]
    fn test_add_child() {
        let mut node = make_supervisor();
        let child_id = Uuid::new_v4();
        node.add_child(child_id);
        node.add_child(child_id); // idempotente
        assert_eq!(node.children.len(), 1);
    }
}
