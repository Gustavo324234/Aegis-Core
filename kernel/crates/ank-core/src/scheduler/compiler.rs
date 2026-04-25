use crate::dag::{DagNode, DagNodeStatus, ExecutionGraph};
use crate::scheduler::ModelPreference;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tracing::warn;
use uuid::Uuid;

/// --- ERRORES DE COMPILACIÓN TOPOLÓGICA ---
#[derive(Error, Debug, PartialEq)]
pub enum GraphError {
    #[error("Cyclic dependency detected in graph: {0}")]
    CyclicDependency(String),
    #[error("Missing dependency: Node '{0}' references non-existent node '{1}'")]
    MissingDependency(String, String),
}

/// --- GRAPH COMPILER ---
/// Implementa validaciones estáticas y deterministas sobre grafos S-DAG
/// generados por modelos de lenguaje propenso a alucinaciones.
pub struct GraphCompiler;

impl GraphCompiler {
    /// Valida que el grafo sea un DAG (Directed Acyclic Graph) válido
    /// y que no tenga dependencias a nodos inexistentes.
    pub fn validate(graph: &ExecutionGraph) -> Result<(), GraphError> {
        // 1. Validar "Dangling Dependencies" (Referencias fantasma)
        for node in graph.nodes.values() {
            for dep_id in &node.dependencies {
                if !graph.nodes.contains_key(dep_id) {
                    return Err(GraphError::MissingDependency(
                        node.node_id.clone(),
                        dep_id.clone(),
                    ));
                }
            }
        }

        // 2. detectar Ciclos usando DFS
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        for node_id in graph.nodes.keys() {
            if !visited.contains(node_id) {
                Self::check_cycles(node_id, graph, &mut visited, &mut visiting)?;
            }
        }

        Ok(())
    }

    fn check_cycles(
        node_id: &String,
        graph: &ExecutionGraph,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
    ) -> Result<(), GraphError> {
        visiting.insert(node_id.clone());

        if let Some(node) = graph.nodes.get(node_id) {
            for dep_id in &node.dependencies {
                if visiting.contains(dep_id) {
                    return Err(GraphError::CyclicDependency(format!(
                        "{} -> {}",
                        node_id, dep_id
                    )));
                }
                if !visited.contains(dep_id) {
                    Self::check_cycles(dep_id, graph, visited, visiting)?;
                }
            }
        }

        visiting.remove(node_id);
        visited.insert(node_id.clone());
        Ok(())
    }

    /// --- SRE FALLBACK: ZERO-PANIC ---
    /// Genera un grafo monolítico de emergencia si la compilación del S-DAG falla.
    pub fn create_fallback(original_prompt: &str) -> ExecutionGraph {
        warn!(prompt = %original_prompt, "Graph compilation failed, falling back to monolithic node");

        let mut nodes = HashMap::new();
        let fallback_node_id = format!("monolithic_{}", Uuid::new_v4().to_string().split_at(8).0);

        nodes.insert(
            fallback_node_id.clone(),
            DagNode {
                node_id: fallback_node_id.clone(),
                description: original_prompt.to_string(),
                dependencies: vec![],
                required_model: ModelPreference::HybridSmart,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
                agent_id: None,
            },
        );

        ExecutionGraph {
            graph_id: format!(
                "graph_fallback_{}",
                Uuid::new_v4().to_string().split_at(8).0
            ),
            original_prompt: original_prompt.to_string(),
            nodes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::DagNode;
    use anyhow::Context;
    use std::collections::HashMap;

    fn create_test_node(id: &str, deps: Vec<&str>) -> DagNode {
        DagNode {
            node_id: id.to_string(),
            description: format!("Task {}", id),
            dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
            required_model: ModelPreference::LocalOnly,
            task_hint: None,
            expected_output: None,
            status: DagNodeStatus::Pending,
            agent_id: None,
        }
    }

    #[test]
    fn test_valid_dag_compilation() {
        let mut nodes = HashMap::new();
        nodes.insert("A".to_string(), create_test_node("A", vec![]));
        nodes.insert("B".to_string(), create_test_node("B", vec!["A"]));
        nodes.insert("C".to_string(), create_test_node("C", vec!["A"]));
        nodes.insert("D".to_string(), create_test_node("D", vec!["B", "C"]));

        let graph = ExecutionGraph {
            graph_id: "test".to_string(),
            original_prompt: "test".to_string(),
            nodes,
        };

        assert!(GraphCompiler::validate(&graph).is_ok());
    }

    #[test]
    fn test_reject_cyclic_graph() -> anyhow::Result<()> {
        let mut nodes = HashMap::new();
        nodes.insert("A".to_string(), create_test_node("A", vec!["B"]));
        nodes.insert("B".to_string(), create_test_node("B", vec!["A"]));

        let graph = ExecutionGraph {
            graph_id: "test".to_string(),
            original_prompt: "test".to_string(),
            nodes,
        };

        let result = GraphCompiler::validate(&graph);
        assert!(result.is_err());
        let Err(err) = result else {
            anyhow::bail!("Validation should fail for cyclic graph");
        };
        assert!(matches!(err, GraphError::CyclicDependency(_)));
        Ok(())
    }

    #[test]
    fn test_reject_missing_dependency() -> anyhow::Result<()> {
        let mut nodes = HashMap::new();
        nodes.insert("A".to_string(), create_test_node("A", vec!["NON_EXISTENT"]));

        let graph = ExecutionGraph {
            graph_id: "test".to_string(),
            original_prompt: "test".to_string(),
            nodes,
        };

        let result = GraphCompiler::validate(&graph);
        assert!(result.is_err());
        let Err(err) = result else {
            anyhow::bail!("Validation should fail for missing dependency");
        };
        assert!(matches!(err, GraphError::MissingDependency(_, _)));
        Ok(())
    }

    #[test]
    fn test_fallback_generation() -> anyhow::Result<()> {
        let prompt = "Solve the Riemann hypothesis";
        let fallback = GraphCompiler::create_fallback(prompt);

        assert_eq!(fallback.nodes.len(), 1);
        let node = fallback
            .nodes
            .values()
            .next()
            .context("Fallback graph should have one node")?;
        assert_eq!(node.description, prompt);
        assert!(node.dependencies.is_empty());
        assert!(fallback.graph_id.starts_with("graph_fallback_"));
        Ok(())
    }
}
