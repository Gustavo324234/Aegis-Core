use crate::dag::{ExecutionGraph, GraphManager};
use crate::scheduler::compiler::GraphCompiler;
use tracing::{info, instrument, warn};

/// --- GRAPH INTEGRATOR ---
/// Puente de integración entre el Orquestador S-DAG y el Scheduler de ANK.
/// Garantiza que ningún grafo con dependencias rotas entre en el Pipeline de ejecución.
pub struct GraphIntegrator;

impl GraphIntegrator {
    /// Valida y registra un grafo en el manager.
    /// Si la validación falla (ciclos o punteros colgantes), genera un grafo
    /// monolítico de fallback para no interrumpir el servicio (SRE Resilience).
    #[instrument(skip(manager, graph), name = "ANK_Graph_Integration")]
    pub fn validate_and_register(manager: &mut GraphManager, graph: ExecutionGraph) {
        info!(graph_id = %graph.graph_id, "Intercepting graph for topological validation...");

        match GraphCompiler::validate(&graph) {
            Ok(_) => {
                info!(graph_id = %graph.graph_id, "S-DAG Validation PASSED. Inserting into active set.");
                manager.active_graphs.insert(graph.graph_id.clone(), graph);
            }
            Err(e) => {
                warn!(
                    graph_id = %graph.graph_id,
                    error = %e,
                    "S-DAG Validation FAILED (Mathematical Breach). Triggering Fallback."
                );

                // Aplicar Fallback: Un solo nodo con el prompt original
                let fallback_graph = GraphCompiler::create_fallback(&graph.original_prompt);
                let fallback_id = fallback_graph.graph_id.clone();

                manager
                    .active_graphs
                    .insert(fallback_id.clone(), fallback_graph);
                info!(fallback_id = %fallback_id, "Monolithic fallback graph registered successfully.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{DagNode, DagNodeStatus, ExecutionGraph};
    use crate::scheduler::ModelPreference;
    use anyhow::Context;
    use std::collections::HashMap;

    #[test]
    fn test_integration_fallback_activation() -> anyhow::Result<()> {
        let mut manager = GraphManager::new();

        // Grafo con ciclo: A -> B -> A
        let mut nodes = HashMap::new();
        nodes.insert(
            "A".into(),
            DagNode {
                node_id: "A".into(),
                description: "Task A".into(),
                dependencies: vec!["B".into()],
                required_model: ModelPreference::LocalOnly,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
            },
        );
        nodes.insert(
            "B".into(),
            DagNode {
                node_id: "B".into(),
                description: "Task B".into(),
                dependencies: vec!["A".into()],
                required_model: ModelPreference::LocalOnly,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
            },
        );

        let cyclic_graph = ExecutionGraph {
            graph_id: "cyclic_1".into(),
            original_prompt: "Create a cycle".into(),
            nodes,
        };

        // Al registrar, debe detectar el error y aplicar fallback
        GraphIntegrator::validate_and_register(&mut manager, cyclic_graph);

        assert_eq!(manager.active_graphs.len(), 1);
        let registered_graph = manager
            .active_graphs
            .values()
            .next()
            .context("Active graphs should contain the fallback")?;

        // Debe ser el grafo de fallback (monolítico)
        assert!(registered_graph.graph_id.starts_with("graph_fallback_"));
        assert_eq!(registered_graph.nodes.len(), 1);
        let node = registered_graph
            .nodes
            .values()
            .next()
            .context("Fallback graph should contain one node")?;
        assert_eq!(node.description, "Create a cycle");
        Ok(())
    }
}
