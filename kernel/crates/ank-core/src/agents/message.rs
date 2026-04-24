use crate::agents::node::AgentId;
use serde::{Deserialize, Serialize};

/// Contexto que el orquestador inyecta al despachar una tarea.
/// Contiene solo lo que el agente necesita para su tarea específica (ADR-AGENTS-006).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentContext {
    /// Archivos relevantes al scope del agente (rutas relativas al repo).
    pub relevant_files: Vec<String>,
    /// Fragmentos de memoria L3 relevantes recuperados del VCM.
    pub memory_snippets: Vec<String>,
    /// Resultados de sub-agentes ya completados (para agregación por supervisores).
    pub child_reports: Vec<AgentResult>,
    /// Tokens disponibles para este agente (impuesto por VCM).
    pub token_budget: usize,
}

/// Resultado que un agente produce al completar su tarea.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: AgentId,
    /// Descripción legible del rol, ej: "Kernel Engineer / Scheduler".
    pub role_description: String,
    /// Resumen ejecutivo para el supervisor inmediato.
    pub summary: String,
    /// Artefactos producidos (código, documentos, planes, etc.).
    pub artifacts: Vec<Artifact>,
    /// Datos adicionales estructurados (métricas, timestamps, etc.).
    pub metadata: serde_json::Value,
}

/// Artefacto producido por un agente al completar su tarea.
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
    /// Comando a ejecutar (integración futura con Sandbox).
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
/// Comunicación lateral (entre agentes de distinto supervisor) está prohibida
/// por ADR-AGENTS-002.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Supervisor → Subordinado: asigna una tarea con contexto filtrado.
    Dispatch {
        task_description: String,
        context: AgentContext,
        reply_to: AgentId,
        deadline_ms: Option<u64>,
    },

    /// Subordinado → Supervisor: reporta resultado de tarea completada.
    Report {
        from: AgentId,
        result: AgentResult,
        status: ReportStatus,
    },

    /// Sistema → Agente: señal de cancelación (ej: timeout, terminate).
    Cancel { reason: String },
}

impl AgentMessage {
    pub fn is_dispatch(&self) -> bool {
        matches!(self, Self::Dispatch { .. })
    }

    pub fn is_report(&self) -> bool {
        matches!(self, Self::Report { .. })
    }
}
