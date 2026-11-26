# CORE-157 — feat: AgentMessage — Protocolo de Comunicación Inter-Agente

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155
**Estado:** ✅ Completo

---

## Contexto

Los agentes se comunican mediante mensajes tipados. Este ticket define el **protocolo de mensajería** entre nodos del árbol: qué puede enviar un supervisor a un subordinado y qué puede reportar un subordinado hacia arriba.

**Regla fundamental (ADR-AGENTS-002):** La comunicación lateral entre agentes de distinto supervisor está prohibida. Toda coordinación cross-domain sube al supervisor común y baja de nuevo.

---

## Cambios Requeridos

### Crear `ank-core/src/agents/message.rs`

```rust
use crate::agents::node::AgentId;

/// Contexto que el orquestador inyecta al despachar una tarea.
/// Contiene SOLO lo que el agente necesita para su tarea específica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// Archivos relevantes al scope del agente (rutas relativas)
    pub relevant_files: Vec<String>,
    /// Fragmentos de memoria L3 relevantes (desde VCM)
    pub memory_snippets: Vec<String>,
    /// Resultados de sub-agentes ya completados (para agregación)
    pub child_reports: Vec<AgentResult>,
    /// Tokens disponibles para este agente
    pub token_budget: usize,
}

/// Resultado que un agente produce al completar su tarea.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: AgentId,
    pub role_description: String,    // ej: "Kernel Engineer / Scheduler"
    pub summary: String,             // resumen para el supervisor inmediato
    pub artifacts: Vec<Artifact>,    // código, documentos, etc.
    pub metadata: serde_json::Value, // datos adicionales estructurados
}

/// Artefacto producido por un agente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub path: Option<String>,  // ruta en el repo, si aplica
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactKind {
    Code,
    Documentation,
    Plan,
    Report,
    Command,  // comando a ejecutar (para futura integración con Sandbox)
}

/// Estado del reporte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Success,
    PartialSuccess { warnings: Vec<String> },
    Failure { reason: String },
    NeedsInput { question: String },  // el agente necesita aclaración
}

/// Mensajes que fluyen entre nodos del árbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Supervisor → Subordinado: asigna una tarea
    Dispatch {
        task_description: String,
        context: AgentContext,
        reply_to: AgentId,
        deadline_ms: Option<u64>,
    },

    /// Subordinado → Supervisor: reporta resultado
    Report {
        from: AgentId,
        result: AgentResult,
        status: ReportStatus,
    },

    /// Sistema → Agente: señal de cancelación
    Cancel {
        reason: String,
    },
}

impl AgentMessage {
    pub fn is_dispatch(&self) -> bool { matches!(self, Self::Dispatch { .. }) }
    pub fn is_report(&self) -> bool { matches!(self, Self::Report { .. }) }
}
```

---

## Criterios de Aceptación

- [x] Todos los tipos compilan con `derive(Debug, Clone, Serialize, Deserialize)`
- [x] `AgentContext::default()` es posible (todos los Vecs vacíos, token_budget razonable)
- [x] `AgentMessage::is_dispatch()` y `is_report()` funcionan correctamente
- [x] No hay referencias circulares ni lifetime issues
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*