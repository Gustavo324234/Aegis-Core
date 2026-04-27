use crate::agents::node::AgentId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Correlaciona un Query con su QueryReply a través de los niveles del árbol.
pub type QueryId = Uuid;

/// Contexto inyectado al despachar una tarea.
/// Solo contiene lo necesario para el scope del agente (ADR-CAA-009).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentContext {
    /// Archivos relevantes al scope del agente (rutas relativas al repo).
    pub relevant_files: Vec<String>,
    /// Fragmentos de memoria L3 relevantes recuperados del VCM.
    pub memory_snippets: Vec<String>,
    /// Reportes de sub-agentes ya completados (para agregación por supervisores).
    pub child_reports: Vec<AgentResult>,
    /// Tokens disponibles para este agente (controlado por ContextBudget).
    pub token_budget: usize,
}

/// Resultado producido por un agente al completar su tarea.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: AgentId,
    /// Descripción legible del rol, ej: "Supervisor/Kernel".
    pub role_description: String,
    /// Resumen ejecutivo para el supervisor inmediato.
    pub summary: String,
    /// Artefactos producidos (código, documentos, planes, etc.).
    pub artifacts: Vec<Artifact>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    /// Ruta en el repo, si el artefacto corresponde a un archivo.
    pub path: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactKind {
    Code,
    Documentation,
    Plan,
    Report,
    Command,
}

/// Estado del reporte enviado por un subordinado a su supervisor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Success,
    PartialSuccess {
        warnings: Vec<String>,
    },
    Failure {
        reason: String,
    },
    /// El agente necesita aclaración antes de continuar.
    NeedsInput {
        question: String,
    },
}

/// Mensajes que fluyen entre nodos del árbol de agentes.
///
/// Reglas de comunicación (Epic 45):
/// - Dispatch: solo hacia abajo (padre → hijo)
/// - Report: solo hacia arriba (hijo → padre)
/// - Query: hacia abajo, sin generar trabajo nuevo
/// - QueryReply: hacia arriba, condensada por cada nivel
/// - Lateral: solo entre nodos con el mismo parent_id
/// - Nunca salta niveles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Padre → Hijo: asigna trabajo. Puede resultar en spawn de nuevos agentes.
    Dispatch {
        task_description: String,
        context: AgentContext,
        reply_to: AgentId,
        deadline_ms: Option<u64>,
    },

    /// Hijo → Padre: resultado de tarea completada.
    Report {
        from: AgentId,
        result: AgentResult,
        status: ReportStatus,
    },

    /// Padre → Hijo: consulta de información sin crear trabajo nuevo (ADR-CAA-003).
    /// Un nodo que recibe un Query NO puede hacer Dispatch como consecuencia.
    Query {
        question: String,
        /// Hint para que el receptor sepa hacia qué sub-árbol bajar la query.
        context_hint: Option<String>,
        reply_to: AgentId,
        query_id: QueryId,
    },

    /// Hijo → Padre: respuesta a un Query.
    /// Cada nivel condensa la respuesta antes de reenviarla hacia arriba (ADR-CAA-011).
    QueryReply {
        /// Respuesta condensada para el nivel receptor.
        answer: String,
        query_id: QueryId,
        from: AgentId,
    },

    /// Sistema → Agente: señal de cancelación.
    Cancel { reason: String },
}

impl AgentMessage {
    pub fn is_dispatch(&self) -> bool {
        matches!(self, Self::Dispatch { .. })
    }

    pub fn is_report(&self) -> bool {
        matches!(self, Self::Report { .. })
    }

    pub fn is_query(&self) -> bool {
        matches!(self, Self::Query { .. })
    }

    pub fn is_query_reply(&self) -> bool {
        matches!(self, Self::QueryReply { .. })
    }

    pub fn query_id(&self) -> Option<QueryId> {
        match self {
            Self::Query { query_id, .. } => Some(*query_id),
            Self::QueryReply { query_id, .. } => Some(*query_id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_id_correlation() {
        let qid = Uuid::new_v4();
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();

        let query = AgentMessage::Query {
            question: "¿qué hace authenticate_tenant?".to_string(),
            context_hint: Some("enclave".to_string()),
            reply_to: sender,
            query_id: qid,
        };

        let reply = AgentMessage::QueryReply {
            answer: "Autentica al tenant contra el enclave SQLCipher.".to_string(),
            query_id: qid,
            from: receiver,
        };

        assert_eq!(query.query_id(), reply.query_id());
        assert!(query.is_query());
        assert!(reply.is_query_reply());
        assert!(!query.is_dispatch());
    }

    #[test]
    fn test_report_status_variants() {
        let s = ReportStatus::Success;
        assert_eq!(s, ReportStatus::Success);

        let f = ReportStatus::Failure {
            reason: "timeout".to_string(),
        };
        assert_ne!(s, f);
    }
}
