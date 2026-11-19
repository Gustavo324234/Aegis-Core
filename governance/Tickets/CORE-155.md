# CORE-155 — feat: AgentNode, AgentRole y AgentState — Tipos Base del Sistema Multiagente

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna (ticket fundacional)

---

## Contexto

La Epic 43 introduce una jerarquía n-ary de agentes cognitivos. Este ticket crea los **tipos de datos base** que representan un nodo en esa jerarquía. Es el cimiento sobre el que se construyen todos los tickets siguientes.

El módulo a crear es `ank-core/src/agents/`.

---

## Cambios Requeridos

### 1. Crear `ank-core/src/agents/mod.rs`

```rust
pub mod node;
pub mod message;
pub mod tree;
pub mod orchestrator;
pub mod project;

pub use node::{AgentId, AgentNode, AgentRole, AgentState};
pub use message::{AgentMessage, AgentContext, AgentResult, ReportStatus};
pub use tree::AgentTree;
pub use orchestrator::AgentOrchestrator;
pub use project::ProjectRegistry;
```

### 2. Crear `ank-core/src/agents/node.rs`

Definir los siguientes tipos con `derive(Debug, Clone, Serialize, Deserialize)`:

```rust
// Identificador único de agente
pub type AgentId = uuid::Uuid;

// Identificador de proyecto
pub type ProjectId = String;

// Nivel jerárquico del agente
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    /// Supervisor de un proyecto completo. Hijo directo del usuario.
    ProjectSupervisor,
    /// Supervisor de un dominio (ej: "Kernel", "Shell", "Frontend").
    DomainSupervisor,
    /// Agente especializado que ejecuta tareas atómicas.
    Specialist,
}

// Estado del ciclo de vida del agente
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Running,
    WaitingReport,   // Esperando que sus subordinados terminen
    Complete,
    Failed { reason: String },
}

// Nodo del árbol de agentes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub project_id: ProjectId,
    pub domain: String,                   // ej: "kernel", "shell", "frontend"
    pub parent_id: Option<AgentId>,
    pub children: Vec<AgentId>,
    pub system_prompt: String,
    pub task_type: crate::pcb::TaskType,  // para CMR routing
    pub state: AgentState,
    pub context_budget: usize,            // tokens máximos
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl AgentNode {
    pub fn new(
        role: AgentRole,
        project_id: ProjectId,
        domain: impl Into<String>,
        parent_id: Option<AgentId>,
        system_prompt: impl Into<String>,
        task_type: crate::pcb::TaskType,
    ) -> Self { ... }

    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    pub fn add_child(&mut self, child_id: AgentId) { ... }

    pub fn set_state(&mut self, state: AgentState) { ... }
}
```

### 3. Agregar `agents` al `lib.rs` de `ank-core`

En `ank-core/src/lib.rs`, agregar:
```rust
pub mod agents;
```

---

## Criterios de Aceptación

- [ ] El módulo `ank-core::agents` existe y compila sin warnings
- [ ] `AgentRole`, `AgentState`, `AgentNode` son públicos y exportados desde `agents::mod`
- [ ] `AgentNode::new()` inicializa con estado `Idle`, `children` vacío, y `created_at` = now
- [ ] `AgentNode::is_root()` retorna `true` si `parent_id` es `None`
- [ ] Todos los tipos implementan `Debug`, `Clone`, `Serialize`, `Deserialize`
- [ ] `cargo build` pasa sin warnings (`-D warnings`)
- [ ] No usar `unwrap()` ni `expect()` en el código nuevo

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: AgentNode/AgentRole/AgentState existen en kernel/crates/ank-core/src/agents/node.rs (líneas 14/74/89).
