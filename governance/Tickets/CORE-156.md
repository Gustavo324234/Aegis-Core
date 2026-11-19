# CORE-156 — feat: AgentTree — Estructura en Memoria y Operaciones

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155
**Estado:** ✅ Completo

---

## Contexto

Con los tipos base definidos en CORE-155, este ticket implementa el **contenedor del árbol de agentes**: la estructura que permite insertar, consultar, recorrer y podar nodos en la jerarquía activa.

El `AgentTree` vive en memoria durante una sesión de usuario. No se persiste en SQLCipher (ver ADR-AGENTS-001 en EPIC_43).

---

## Cambios Requeridos

### Crear `ank-core/src/agents/tree.rs`

```rust
use std::collections::HashMap;
use crate::agents::node::{AgentId, AgentNode, AgentRole, ProjectId};

/// Árbol de agentes activos en memoria.
/// Thread-safe via Arc<RwLock<AgentTree>> en el AgentOrchestrator.
#[derive(Debug, Default)]
pub struct AgentTree {
    nodes: HashMap<AgentId, AgentNode>,
    /// Mapea project_id → agent_id del ProjectSupervisor raíz
    project_roots: HashMap<ProjectId, AgentId>,
}

impl AgentTree {
    pub fn new() -> Self { ... }

    /// Inserta un nodo. Si tiene parent_id, lo registra como hijo del padre.
    /// Retorna error si el padre no existe.
    pub fn insert(&mut self, node: AgentNode) -> anyhow::Result<AgentId> { ... }

    /// Retorna referencia inmutable a un nodo.
    pub fn get(&self, id: &AgentId) -> Option<&AgentNode> { ... }

    /// Retorna referencia mutable a un nodo.
    pub fn get_mut(&mut self, id: &AgentId) -> Option<&mut AgentNode> { ... }

    /// Retorna los hijos directos de un nodo.
    pub fn children(&self, id: &AgentId) -> Vec<&AgentNode> { ... }

    /// Retorna todos los descendientes (recursivo) de un nodo.
    pub fn descendants(&self, id: &AgentId) -> Vec<&AgentNode> { ... }

    /// Retorna el ProjectSupervisor de un proyecto dado.
    pub fn project_root(&self, project_id: &ProjectId) -> Option<&AgentNode> { ... }

    /// Retorna todos los ProjectSupervisors activos.
    pub fn all_roots(&self) -> Vec<&AgentNode> { ... }

    /// Elimina un nodo y todos sus descendientes.
    /// Actualiza la lista de hijos del padre si existe.
    pub fn prune(&mut self, id: &AgentId) -> anyhow::Result<usize> { ... }
    // retorna la cantidad de nodos eliminados

    /// Cantidad total de nodos en el árbol.
    pub fn len(&self) -> usize { ... }

    pub fn is_empty(&self) -> bool { ... }
}
```

**Invariantes a mantener:**
- Un nodo con `role == ProjectSupervisor` debe estar registrado en `project_roots`.
- El `parent_id` de todo nodo no-raíz debe existir en `nodes`.
- `children` de cada nodo debe ser consistente con los `parent_id` del árbol.

---

## Criterios de Aceptación

- [x] `AgentTree::insert()` registra correctamente en `project_roots` si el rol es `ProjectSupervisor`
- [x] `AgentTree::insert()` actualiza `children` del padre si `parent_id` está presente
- [x] `AgentTree::insert()` retorna `Err` si el `parent_id` declarado no existe en el árbol
- [x] `AgentTree::prune()` elimina el nodo target y todos sus descendientes recursivamente
- [x] `AgentTree::prune()` actualiza la lista `children` del padre del nodo eliminado
- [x] `AgentTree::descendants()` retorna todos los nodos del subárbol (no solo hijos directos)
- [x] `AgentTree::project_root()` retorna `None` para proyectos no existentes (sin panic)
- [x] Tests unitarios para `insert`, `prune`, `descendants` y `project_root`
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*