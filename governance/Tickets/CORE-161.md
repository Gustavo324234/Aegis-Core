# CORE-161 — feat: DagNode Extension — Campo agent_id en DagNode

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155

---

## Contexto

El `DagNode` en `ank-core/src/dag/` representa una tarea dentro de un Directed Acyclic Graph (S-DAG). Para que un `ProjectSupervisor` pueda compilar un S-DAG donde cada nodo corresponde a un `SpecialistAgent` (ADR-AGENTS-004), el `DagNode` necesita saber a qué agente está asignado.

Este ticket es análogo a CORE-160 pero para el DAG engine. **Mínimamente invasivo.**

---

## Cambios Requeridos

### Localizar el struct DagNode

Buscar en `ank-core/src/dag/` el struct que representa un nodo individual del DAG. Puede estar en `dag/node.rs`, `dag/mod.rs`, o similar.

### Agregar campo al DagNode

```rust
use crate::agents::node::AgentId;

pub struct DagNode {
    // ... campos existentes, NO modificar ...
    
    /// Si este nodo del DAG debe ser ejecutado por un agente específico
    /// del árbol jerárquico, aquí está su AgentId.
    /// None para nodos ejecutados directamente por el Scheduler.
    pub agent_id: Option<AgentId>,
}
```

El campo debe ser `None` por defecto para mantener compatibilidad total con los DAGs existentes.

---

## Criterios de Aceptación

- [ ] `DagNode` tiene campo `pub agent_id: Option<AgentId>`
- [ ] Todo el código existente que crea DagNodes sigue compilando sin cambios
- [ ] El campo por defecto es `None`
- [ ] `cargo build` del workspace completo pasa sin warnings

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: DagNode.agent_id existe (scheduler/graph.rs:67, compiler.rs:96).
