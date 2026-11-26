# CORE-158 — feat: AgentOrchestrator — Ciclo de Vida y Coordinación

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155, CORE-156, CORE-157, CORE-160

---

## Contexto

El `AgentOrchestrator` es el componente central de la Epic 43. Coordina el ciclo de vida completo de los agentes: los crea, les despacha tareas, recibe sus reportes, agrega resultados y los destruye cuando ya no son necesarios.

Vive dentro del `CognitiveScheduler` existente como un campo adicional. **No reemplaza** el Scheduler ni el DAG engine; los usa como infraestructura.

---

## Cambios Requeridos

### Crear `ank-core/src/agents/orchestrator.rs`

```rust
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use crate::agents::{AgentTree, AgentId, AgentNode, AgentMessage, AgentContext, AgentResult};
use crate::agents::node::{AgentRole, AgentState, ProjectId};
use crate::router::CognitiveRouter;
use crate::vcm::VirtualContextManager;

pub struct AgentOrchestrator {
    tree: Arc<RwLock<AgentTree>>,
    router: Arc<CognitiveRouter>,   // CMR existente
    vcm: Arc<VirtualContextManager>, // VCM existente
    /// Canal para enviar mensajes a agentes individuales
    /// key: AgentId, value: sender del canal del agente
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
}

impl AgentOrchestrator {
    pub fn new(router: Arc<CognitiveRouter>, vcm: Arc<VirtualContextManager>) -> Self { ... }

    /// Crea un ProjectSupervisor para el proyecto dado (si no existe).
    /// Retorna el AgentId del supervisor (nuevo o existente).
    pub async fn ensure_project_supervisor(
        &self,
        project_id: &ProjectId,
        project_description: &str,
    ) -> anyhow::Result<AgentId> { ... }

    /// Spawnea un nuevo agente como hijo del padre indicado.
    /// El padre debe existir en el árbol.
    pub async fn spawn_agent(
        &self,
        role: AgentRole,
        project_id: ProjectId,
        domain: String,
        parent_id: AgentId,
        system_prompt: String,
        task_type: crate::pcb::TaskType,
    ) -> anyhow::Result<AgentId> { ... }

    /// Despacha una tarea al agente indicado.
    /// El AgentOrchestrator construye el AgentContext filtrando via VCM.
    pub async fn dispatch(
        &self,
        agent_id: AgentId,
        task_description: String,
        scope_hints: Vec<String>,  // pistas para que VCM filtre el contexto
    ) -> anyhow::Result<()> { ... }

    /// Loop interno de un agente: recibe mensajes, llama al CMR para inferencia,
    /// produce un AgentResult y lo reporta al padre.
    /// Se lanza en un tokio::spawn separado por agente.
    async fn run_agent_loop(
        agent_id: AgentId,
        tree: Arc<RwLock<AgentTree>>,
        router: Arc<CognitiveRouter>,
        mut rx: mpsc::Receiver<AgentMessage>,
        parent_tx: Option<mpsc::Sender<AgentMessage>>,
    ) { ... }

    /// Procesa el reporte de un agente subordinado.
    /// Si todos los hijos del padre completaron, agrega y reporta al abuelo.
    async fn handle_report(
        &self,
        from: AgentId,
        result: AgentResult,
    ) -> anyhow::Result<()> { ... }

    /// Elimina un agente y su subárbol del árbol activo.
    pub async fn terminate(&self, agent_id: &AgentId) -> anyhow::Result<()> { ... }

    /// Retorna un snapshot del árbol actual (para la API HTTP).
    pub async fn tree_snapshot(&self) -> AgentTree { ... }
}
```

### Integración con CognitiveScheduler

En `ank-core/src/scheduler/mod.rs` (o donde viva el scheduler), agregar:

```rust
pub struct CognitiveScheduler {
    // ... campos existentes ...
    pub agent_orchestrator: Arc<AgentOrchestrator>,
}
```

**Importante:** `AgentOrchestrator` debe inicializarse con las mismas instancias de `CognitiveRouter` y `VirtualContextManager` que ya usa el Scheduler. No crear instancias nuevas.

### Lógica de `run_agent_loop`

El loop de cada agente sigue este algoritmo:
1. Espera `AgentMessage::Dispatch` en su canal
2. Extrae `task_description` y `context`
3. Construye el prompt: `system_prompt + context.memory_snippets + context.relevant_files + task_description`
4. Llama al CMR con `task_type` del agente para obtener el mejor modelo
5. Ejecuta inferencia via el HAL existente
6. Parsea la respuesta del LLM para extraer `AgentResult` (summary + artifacts)
7. Envía `AgentMessage::Report` al canal del padre
8. Actualiza su `AgentState` a `Complete` en el árbol

---

## Criterios de Aceptación

- [ ] `AgentOrchestrator::ensure_project_supervisor()` es idempotente — dos llamadas con el mismo `project_id` retornan el mismo `AgentId`
- [ ] `AgentOrchestrator::spawn_agent()` falla con `Err` si `parent_id` no existe en el árbol
- [ ] Cada agente corre en su propio `tokio::spawn` con su propio canal `mpsc`
- [ ] Cuando todos los hijos de un nodo reportan `Success`, el padre recibe automáticamente `AgentMessage::Report` de tipo agregado
- [ ] `AgentOrchestrator::terminate()` cancela el canal del agente y llama a `AgentTree::prune()`
- [ ] No hay `unwrap()` ni `expect()` en el código nuevo
- [ ] Los logs distinguen claramente el nivel jerárquico: `[PROJECT][Aegis]`, `[DOMAIN][Kernel]`, `[SPECIALIST][scheduler]`
- [ ] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: AgentOrchestrator implementado en agents/orchestrator.rs (ciclo de vida completo, usado por ank-server y ws/chat).
