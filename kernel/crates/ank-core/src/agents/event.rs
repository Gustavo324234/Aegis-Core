use crate::agents::node::{AgentId, AgentRole, ProjectId};
use serde::{Deserialize, Serialize};

/// Eventos emitidos por el AgentOrchestrator hacia la UI via WebSocket.
/// Stream separado del chat para no afectar la latencia de respuesta (ADR-CAA-008).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Un nuevo agente fue spaweado.
    Spawned {
        agent_id: AgentId,
        role: AgentRole,
        parent_id: Option<AgentId>,
        project_id: ProjectId,
        model: String,
        task_type: String,
    },

    /// El estado de un agente cambió.
    StateChanged {
        agent_id: AgentId,
        state: String,
    },

    /// Actividad observable de un agente (descripción en lenguaje natural).
    Activity {
        agent_id: AgentId,
        description: String,
    },

    /// Un agente generó un reporte (hacia arriba o síntesis final).
    Reported {
        agent_id: AgentId,
        summary: String,
    },

    /// Un agente fue eliminado del árbol.
    Pruned {
        agent_id: AgentId,
    },

    /// Un proyecto fue restaurado desde el filesystem (CORE-207).
    Restored {
        project_id: ProjectId,
        node_count: usize,
    },

    /// Snapshot completo del árbol actual (enviado al conectarse o bajo demanda).
    TreeSnapshot {
        nodes: Vec<crate::agents::orchestrator::AgentNodeSummary>,
    },
}
