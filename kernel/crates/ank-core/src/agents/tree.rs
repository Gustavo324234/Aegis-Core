use crate::agents::node::{AgentId, AgentNode, AgentRole, ProjectId};
use std::collections::HashMap;

/// Árbol de agentes activos en memoria para una sesión de usuario.
/// Thread-safe via `Arc<RwLock<AgentTree>>` en el `AgentOrchestrator`.
/// No se persiste en SQLCipher (ADR-AGENTS-001).
#[derive(Debug, Default)]
pub struct AgentTree {
    nodes: HashMap<AgentId, AgentNode>,
    /// Mapea project_id → agent_id del ProjectSupervisor raíz.
    project_roots: HashMap<ProjectId, AgentId>,
}

impl AgentTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserta un nodo. Si tiene `parent_id`, lo registra como hijo del padre.
    /// Retorna error si el padre declarado no existe en el árbol.
    pub fn insert(&mut self, node: AgentNode) -> anyhow::Result<AgentId> {
        let agent_id = node.agent_id;

        if let Some(parent_id) = node.parent_id {
            let parent = self
                .nodes
                .get_mut(&parent_id)
                .ok_or_else(|| anyhow::anyhow!("Parent agent {} not found in tree", parent_id))?;
            parent.add_child(agent_id);
        }

        if node.role == AgentRole::ProjectSupervisor {
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

    /// Elimina un nodo y todos sus descendientes.
    /// Actualiza la lista de hijos del padre si existe.
    /// Retorna la cantidad de nodos eliminados.
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
            .map(|n| n.role == AgentRole::ProjectSupervisor)
            .unwrap_or(false);

        let mut count = 0;
        for desc_id in &descendants {
            if let Some(n) = self.nodes.remove(desc_id) {
                if n.role == AgentRole::ProjectSupervisor {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::node::{AgentRole, AgentState};
    use crate::pcb::TaskType;

    fn make_node(
        role: AgentRole,
        project: &str,
        domain: &str,
        parent: Option<AgentId>,
    ) -> AgentNode {
        AgentNode::new(
            role,
            project.to_string(),
            domain,
            parent,
            "prompt",
            TaskType::Planning,
        )
    }

    #[test]
    fn test_insert_root() {
        let mut tree = AgentTree::new();
        let node = make_node(AgentRole::ProjectSupervisor, "aegis", "Aegis OS", None);
        let id = tree.insert(node).unwrap();
        assert!(tree.get(&id).is_some());
        assert!(tree.project_root(&"aegis".to_string()).is_some());
    }

    #[test]
    fn test_insert_with_missing_parent_returns_err() {
        let mut tree = AgentTree::new();
        let fake_parent = uuid::Uuid::new_v4();
        let node = make_node(
            AgentRole::Specialist,
            "aegis",
            "scheduler",
            Some(fake_parent),
        );
        assert!(tree.insert(node).is_err());
    }

    #[test]
    fn test_descendants() {
        let mut tree = AgentTree::new();
        let root = make_node(AgentRole::ProjectSupervisor, "aegis", "Aegis OS", None);
        let root_id = tree.insert(root).unwrap();

        let domain = make_node(
            AgentRole::DomainSupervisor,
            "aegis",
            "Kernel",
            Some(root_id),
        );
        let domain_id = tree.insert(domain).unwrap();

        let spec = make_node(AgentRole::Specialist, "aegis", "scheduler", Some(domain_id));
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
        let root = make_node(AgentRole::ProjectSupervisor, "aegis", "Aegis OS", None);
        let root_id = tree.insert(root).unwrap();

        let domain = make_node(
            AgentRole::DomainSupervisor,
            "aegis",
            "Kernel",
            Some(root_id),
        );
        let domain_id = tree.insert(domain).unwrap();

        let spec = make_node(AgentRole::Specialist, "aegis", "scheduler", Some(domain_id));
        tree.insert(spec).unwrap();

        let removed = tree.prune(&domain_id).unwrap();
        assert_eq!(removed, 2); // domain + specialist
        assert_eq!(tree.len(), 1); // only root remains
                                   // Root's children list must be updated
        assert!(tree.get(&root_id).unwrap().children.is_empty());
    }

    #[test]
    fn test_project_root_missing() {
        let tree = AgentTree::new();
        assert!(tree.project_root(&"nonexistent".to_string()).is_none());
    }
}
