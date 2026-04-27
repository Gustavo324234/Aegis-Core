use crate::agents::node::{AgentId, AgentNode, AgentRole, AgentState, ProjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Árbol de agentes activos en memoria para una sesión.
/// Thread-safe via `Arc<RwLock<AgentTree>>` en el `AgentOrchestrator`.
/// Serializable a JSON para persistencia entre sesiones (ADR-CAA-005v2).
#[derive(Debug, Default)]
pub struct AgentTree {
    nodes: HashMap<AgentId, AgentNode>,
    /// project_id → agent_id del ProjectSupervisor raíz.
    project_roots: HashMap<ProjectId, AgentId>,
}

/// Snapshot serializable del árbol. Solo estructura + metadatos, sin canales.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTreeSnapshot {
    pub nodes: Vec<AgentNode>,
    pub project_roots: HashMap<ProjectId, AgentId>,
}

impl AgentTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserta un nodo. Si tiene `parent_id`, lo registra como hijo del padre.
    pub fn insert(&mut self, node: AgentNode) -> anyhow::Result<AgentId> {
        let agent_id = node.agent_id;

        if let Some(parent_id) = node.parent_id {
            let parent = self.nodes.get_mut(&parent_id).ok_or_else(|| {
                anyhow::anyhow!("Parent agent {} not found in tree", parent_id)
            })?;
            parent.add_child(agent_id);
        }

        if matches!(node.role, AgentRole::ProjectSupervisor { .. }) {
            self.project_roots.insert(node.project_id.clone(), agent_id);
        }

        self.nodes.insert(agent_id, node);
        Ok(agent_id)
    }

    pub fn get(&self, id: &AgentId) -> Option<&AgentNode> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: &AgentId) -> Option<&mut AgentNode> {
        self.nodes.get_mut(id)
    }

    /// Retorna los hijos directos de un nodo.
    pub fn children(&self, id: &AgentId) -> Vec<&AgentNode> {
        let Some(node) = self.nodes.get(id) else {
            return Vec::new();
        };
        node.children
            .iter()
            .filter_map(|child_id| self.nodes.get(child_id))
            .collect()
    }

    /// Retorna todos los descendientes (recursivo) de un nodo, sin incluirlo.
    pub fn descendants(&self, id: &AgentId) -> Vec<&AgentNode> {
        let mut result = Vec::new();
        self.collect_descendants(id, &mut result);
        result
    }

    fn collect_descendants<'a>(&'a self, id: &AgentId, out: &mut Vec<&'a AgentNode>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        for child_id in &node.children {
            if let Some(child) = self.nodes.get(child_id) {
                out.push(child);
                self.collect_descendants(child_id, out);
            }
        }
    }

    /// Retorna el ProjectSupervisor de un proyecto dado.
    pub fn project_root(&self, project_id: &ProjectId) -> Option<&AgentNode> {
        let root_id = self.project_roots.get(project_id)?;
        self.nodes.get(root_id)
    }

    /// Retorna todos los ProjectSupervisors activos.
    pub fn all_roots(&self) -> Vec<&AgentNode> {
        self.project_roots
            .values()
            .filter_map(|id| self.nodes.get(id))
            .collect()
    }

    /// Retorna todos los supervisores (no specialists) — candidatos a generar state summary.
    pub fn all_supervisors(&self) -> Vec<&AgentNode> {
        self.nodes.values().filter(|n| n.should_persist()).collect()
    }

    /// Elimina un nodo y todos sus descendientes.
    /// Actualiza la lista de hijos del padre si existe.
    pub fn prune(&mut self, id: &AgentId) -> anyhow::Result<usize> {
        if !self.nodes.contains_key(id) {
            anyhow::bail!("Agent {} not found in tree", id);
        }

        let descendants: Vec<AgentId> = self.descendants(id).iter().map(|n| n.agent_id).collect();
        let parent_id = self.nodes.get(id).and_then(|n| n.parent_id);
        let project_id = self.nodes.get(id).map(|n| n.project_id.clone());
        let is_root = self
            .nodes
            .get(id)
            .map(|n| matches!(n.role, AgentRole::ProjectSupervisor { .. }))
            .unwrap_or(false);

        let mut count = 0;
        for desc_id in &descendants {
            if let Some(n) = self.nodes.remove(desc_id) {
                if matches!(n.role, AgentRole::ProjectSupervisor { .. }) {
                    self.project_roots.remove(&n.project_id);
                }
                count += 1;
            }
        }

        self.nodes.remove(id);
        count += 1;

        if is_root {
            if let Some(pid) = project_id {
                self.project_roots.remove(&pid);
            }
        }

        if let Some(ppid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&ppid) {
                parent.children.retain(|c| c != id);
            }
        }

        Ok(count)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Retorna todos los nodos del árbol (para snapshots de UI).
    pub fn all_nodes(&self) -> Vec<&AgentNode> {
        self.nodes.values().collect()
    }

    /// Acceso mutable directo al mapa de nodos — usado al restaurar desde snapshot.
    /// No actualiza `project_roots`; usar `register_root` para eso.
    pub fn nodes_mut_raw(&mut self) -> &mut HashMap<AgentId, AgentNode> {
        &mut self.nodes
    }

    /// Registra manualmente un ProjectSupervisor como raíz de un proyecto.
    /// Usado al restaurar el árbol desde filesystem.
    pub fn register_root(&mut self, project_id: ProjectId, agent_id: AgentId) {
        self.project_roots.insert(project_id, agent_id);
    }

    // --- Persistencia (CORE-191) ---

    /// Serializa el árbol a un snapshot JSON.
    /// El snapshot contiene la estructura completa (jerarquía + metadatos).
    /// No incluye canales ni estado de runtime.
    pub fn serialize(&self) -> anyhow::Result<AgentTreeSnapshot> {
        let nodes: Vec<AgentNode> = self.nodes.values().cloned().collect();
        Ok(AgentTreeSnapshot {
            nodes,
            project_roots: self.project_roots.clone(),
        })
    }

    /// Reconstituye el árbol desde un snapshot. Los nodos se marcan como `is_restored = true`.
    /// Los canales de runtime se crean nuevos — el árbol solo restaura la estructura.
    pub fn restore(snapshot: AgentTreeSnapshot) -> anyhow::Result<Self> {
        let mut tree = AgentTree::new();

        // Primer paso: insertar todos los nodos sin vincular hijos
        // (los nodos ya traen sus Vec<children> del snapshot)
        for mut node in snapshot.nodes {
            node.is_restored = true;
            // Resetear estado a Idle al restaurar — el trabajo en curso se retoma
            // desde el state summary, no del estado runtime anterior
            if node.state != AgentState::Complete {
                node.state = AgentState::Idle;
            }
            let agent_id = node.agent_id;
            // No llamamos a insert() normal porque los hijos ya están en el Vec
            // y el padre ya tiene el child registrado — solo insertamos el nodo
            if matches!(node.role, AgentRole::ProjectSupervisor { .. }) {
                tree.project_roots.insert(node.project_id.clone(), agent_id);
            }
            tree.nodes.insert(agent_id, node);
        }

        // Validar consistencia: todos los parent_id referenciados deben existir
        let ids: Vec<AgentId> = tree.nodes.keys().cloned().collect();
        for id in &ids {
            if let Some(node) = tree.nodes.get(id) {
                if let Some(parent_id) = node.parent_id {
                    if !tree.nodes.contains_key(&parent_id) {
                        anyhow::bail!(
                            "Corrupt snapshot: node {} references missing parent {}",
                            id,
                            parent_id
                        );
                    }
                }
            }
        }

        Ok(tree)
    }

    /// Serializa el árbol a JSON string.
    pub fn to_json(&self) -> anyhow::Result<String> {
        let snapshot = self.serialize()?;
        Ok(serde_json::to_string_pretty(&snapshot)?)
    }

    /// Reconstituye el árbol desde un JSON string.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let snapshot: AgentTreeSnapshot = serde_json::from_str(json)?;
        Self::restore(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::TaskType;

    fn make_project_supervisor(project: &str) -> AgentNode {
        AgentNode::new(
            AgentRole::ProjectSupervisor {
                name: project.to_string(),
                description: "test project".to_string(),
            },
            project.to_string(),
            None,
            "prompt",
            TaskType::Planning,
        )
    }

    fn make_supervisor(project: &str, parent: Option<AgentId>) -> AgentNode {
        AgentNode::new(
            AgentRole::Supervisor {
                name: "Kernel".to_string(),
                scope: "kernel modules".to_string(),
            },
            project.to_string(),
            parent,
            "prompt",
            TaskType::Analysis,
        )
    }

    fn make_specialist(project: &str, parent: Option<AgentId>) -> AgentNode {
        AgentNode::new(
            AgentRole::Specialist {
                scope: "leer mod.rs".to_string(),
            },
            project.to_string(),
            parent,
            "prompt",
            TaskType::Code,
        )
    }

    #[test]
    fn test_insert_root() {
        let mut tree = AgentTree::new();
        let node = make_project_supervisor("aegis");
        let id = tree.insert(node).unwrap();
        assert!(tree.get(&id).is_some());
        assert!(tree.project_root(&"aegis".to_string()).is_some());
    }

    #[test]
    fn test_insert_with_missing_parent_returns_err() {
        let mut tree = AgentTree::new();
        let fake_parent = uuid::Uuid::new_v4();
        let node = make_specialist("aegis", Some(fake_parent));
        assert!(tree.insert(node).is_err());
    }

    #[test]
    fn test_descendants() {
        let mut tree = AgentTree::new();
        let root = make_project_supervisor("aegis");
        let root_id = tree.insert(root).unwrap();

        let domain = make_supervisor("aegis", Some(root_id));
        let domain_id = tree.insert(domain).unwrap();

        let spec = make_specialist("aegis", Some(domain_id));
        let spec_id = tree.insert(spec).unwrap();

        let descs = tree.descendants(&root_id);
        assert_eq!(descs.len(), 2);
        let ids: Vec<AgentId> = descs.iter().map(|n| n.agent_id).collect();
        assert!(ids.contains(&domain_id));
        assert!(ids.contains(&spec_id));
    }

    #[test]
    fn test_prune() {
        let mut tree = AgentTree::new();
        let root = make_project_supervisor("aegis");
        let root_id = tree.insert(root).unwrap();

        let domain = make_supervisor("aegis", Some(root_id));
        let domain_id = tree.insert(domain).unwrap();

        let spec = make_specialist("aegis", Some(domain_id));
        tree.insert(spec).unwrap();

        let removed = tree.prune(&domain_id).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(tree.len(), 1);
        assert!(tree.get(&root_id).unwrap().children.is_empty());
    }

    #[test]
    fn test_serialize_restore_roundtrip() {
        let mut tree = AgentTree::new();
        let root = make_project_supervisor("aegis");
        let root_id = tree.insert(root).unwrap();
        let domain = make_supervisor("aegis", Some(root_id));
        let domain_id = tree.insert(domain).unwrap();
        let spec = make_specialist("aegis", Some(domain_id));
        let spec_id = tree.insert(spec).unwrap();

        let json = tree.to_json().unwrap();
        let restored = AgentTree::from_json(&json).unwrap();

        assert_eq!(restored.len(), 3);
        assert!(restored.get(&root_id).is_some());
        assert!(restored.get(&domain_id).is_some());
        assert!(restored.get(&spec_id).is_some());
        assert!(restored.project_root(&"aegis".to_string()).is_some());

        // Todos los nodos restaurados deben tener is_restored = true
        assert!(restored.get(&root_id).unwrap().is_restored);
        assert!(restored.get(&domain_id).unwrap().is_restored);
    }

    #[test]
    fn test_all_supervisors() {
        let mut tree = AgentTree::new();
        let root = make_project_supervisor("aegis");
        let root_id = tree.insert(root).unwrap();
        let domain = make_supervisor("aegis", Some(root_id));
        let domain_id = tree.insert(domain).unwrap();
        let spec = make_specialist("aegis", Some(domain_id));
        tree.insert(spec).unwrap();

        let supervisors = tree.all_supervisors();
        // ProjectSupervisor + Supervisor = 2; Specialist no persiste
        assert_eq!(supervisors.len(), 2);
    }
}
