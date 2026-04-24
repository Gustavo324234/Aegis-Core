use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identificador único de agente — alias de Uuid para claridad semántica.
pub type AgentId = Uuid;

/// Identificador de proyecto — nombre canónico del proyecto.
pub type ProjectId = String;

/// Nivel jerárquico del agente dentro del árbol de orquestación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Supervisor de un proyecto completo. Interlocutor directo del usuario.
    ProjectSupervisor,
    /// Supervisor de un dominio (ej: "Kernel", "Shell", "Frontend").
    DomainSupervisor,
    /// Agente especializado que ejecuta tareas atómicas.
    Specialist,
}

/// Estado del ciclo de vida del agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Running,
    /// Esperando que sus subordinados terminen.
    WaitingReport,
    Complete,
    Failed { reason: String },
}

/// Nodo fundamental del árbol de agentes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub project_id: ProjectId,
    /// Nombre del dominio funcional, ej: "kernel", "shell", "frontend".
    pub domain: String,
    pub parent_id: Option<AgentId>,
    pub children: Vec<AgentId>,
    pub system_prompt: String,
    /// Tipo cognitivo de la tarea — usado por el CMR para selección de modelo.
    pub task_type: crate::pcb::TaskType,
    pub state: AgentState,
    /// Tokens máximos disponibles para el contexto de este agente.
    pub context_budget: usize,
    pub created_at: DateTime<Utc>,
}

impl AgentNode {
    pub fn new(
        role: AgentRole,
        project_id: ProjectId,
        domain: impl Into<String>,
        parent_id: Option<AgentId>,
        system_prompt: impl Into<String>,
        task_type: crate::pcb::TaskType,
    ) -> Self {
        Self {
            agent_id: Uuid::new_v4(),
            role,
            project_id,
            domain: domain.into(),
            parent_id,
            children: Vec::new(),
            system_prompt: system_prompt.into(),
            task_type,
            state: AgentState::Idle,
            context_budget: 8192,
            created_at: Utc::now(),
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
}
