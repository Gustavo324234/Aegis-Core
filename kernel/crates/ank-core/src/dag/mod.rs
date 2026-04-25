use crate::pcb::{TaskType, PCB};
use crate::scheduler::ModelPreference;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// --- ESTADOS DEL NODO EN EL DAG ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DagNodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// --- NODO DEL GRAFO ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    pub node_id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub required_model: ModelPreference,
    pub task_hint: Option<TaskType>,
    pub expected_output: Option<String>,
    pub status: DagNodeStatus,
    /// CORE-161 (Epic 43): Si este nodo debe ser ejecutado por un agente específico
    /// del árbol jerárquico, aquí está su AgentId (= uuid::Uuid).
    /// None para nodos ejecutados directamente por el Scheduler.
    #[serde(default)]
    pub agent_id: Option<uuid::Uuid>,
}

/// --- GRAFO DE EJECUCIÓN (DAG) ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    pub graph_id: String,
    pub original_prompt: String,
    pub nodes: HashMap<String, DagNode>,
}

/// --- RESULTADO DE UN NODO (FEEDBACK) ---
#[derive(Debug, Clone)]
pub struct NodeResult {
    pub node_id: String,
    pub output: String,
    pub status: DagNodeStatus,
}

/// --- GRAPH MANAGER (EL ORQUESTADOR) ---
pub struct GraphManager {
    pub active_graphs: HashMap<String, ExecutionGraph>,
}

impl Default for GraphManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphManager {
    pub fn new() -> Self {
        Self {
            active_graphs: HashMap::new(),
        }
    }

    /// Escanea todos los grafos activos y emite PCBs para todos los nodos
    /// cuyas dependencias estén resueltas (Ejecución Paralela).
    pub fn tick(&mut self) -> Vec<PCB> {
        let mut ready_pcbs = Vec::new();

        for (graph_id, graph) in self.active_graphs.iter_mut() {
            // 1. Identificar todos los nodos que pueden arrancar simultáneamente
            let mut nodes_to_start = Vec::new();

            for node in graph.nodes.values() {
                if node.status == DagNodeStatus::Pending {
                    let can_start = node.dependencies.iter().all(|dep_id| {
                        graph
                            .nodes
                            .get(dep_id)
                            .map(|dep| dep.status == DagNodeStatus::Completed)
                            .unwrap_or(false)
                    });

                    if can_start {
                        nodes_to_start.push(node.node_id.clone());
                    }
                }
            }

            // 2. Generar PCBs con Context Forwarding (Inyección de dependencias)
            for node_id in nodes_to_start {
                // Recolectar contexto de dependencias (Inmutable)
                let (description, deps_context) = {
                    let mut context = HashMap::new();

                    if let Some(node) = graph.nodes.get(&node_id) {
                        for dep_id in &node.dependencies {
                            if let Some(dep_node) = graph.nodes.get(dep_id) {
                                if let Some(output) = &dep_node.expected_output {
                                    // REGLA: dependency_[id] para el trabajador remoto
                                    context
                                        .insert(format!("dependency_{}", dep_id), output.clone());
                                }
                            }
                        }
                        (node.description.clone(), context)
                    } else {
                        // Resiliencia/Zero-Panic: Si el nodo desapareció del DAG concurrentemente,
                        continue;
                    }
                };

                // Actualizar estado a Running y crear PCB (Mutable)
                if let Some(node) = graph.nodes.get_mut(&node_id) {
                    node.status = DagNodeStatus::Running;

                    let mut pcb = PCB::new(node.node_id.clone(), 5, description);

                    pcb.model_pref = node.required_model;
                    if let Some(hint) = node.task_hint {
                        pcb.task_type = hint;
                    }
                    pcb.parent_pid = Some(graph_id.clone());
                    pcb.inlined_context = deps_context;

                    ready_pcbs.push(pcb);
                }
            }
        }

        ready_pcbs
    }

    /// Recibe el resultado de un nodo y actualiza el grafo para desbloquear hijos.
    pub fn handle_result(&mut self, result: NodeResult) -> Result<()> {
        for graph in self.active_graphs.values_mut() {
            if let Some(node) = graph.nodes.get_mut(&result.node_id) {
                node.status = result.status;
                node.expected_output = Some(result.output);
                return Ok(());
            }
        }
        anyhow::bail!("Node {} not found in any active graph", result.node_id)
    }

    /// Implementación del Planner S-DAG
    pub fn generate_dag_from_prompt(prompt: &str) -> Result<ExecutionGraph> {
        // En producción (hasta integrar un parser cognitivo LLM real para S-DAG),
        // generamos un DAG atómico de un solo nodo (Zero-Panic by default).
        let mut nodes = HashMap::new();
        nodes.insert(
            "default_node".to_string(),
            DagNode {
                node_id: "default_node".to_string(),
                description: prompt.to_string(),
                dependencies: vec![],
                required_model: ModelPreference::HybridSmart,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
                agent_id: None,
            },
        );
        Ok(ExecutionGraph {
            graph_id: format!("graph_{}", Uuid::new_v4().to_string().split_at(8).0),
            original_prompt: prompt.to_string(),
            nodes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn test_diamond_graph_parallel_execution() -> anyhow::Result<()> {
        let mut manager = GraphManager::new();

        // Estructura Diamante: A -> [B, C] -> D
        let mut nodes = HashMap::new();

        nodes.insert(
            "A".into(),
            DagNode {
                node_id: "A".into(),
                description: "Task A".into(),
                dependencies: vec![],
                required_model: ModelPreference::LocalOnly,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
                agent_id: None,
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
                agent_id: None,
            },
        );

        nodes.insert(
            "C".into(),
            DagNode {
                node_id: "C".into(),
                description: "Task C".into(),
                dependencies: vec!["A".into()],
                required_model: ModelPreference::LocalOnly,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
                agent_id: None,
            },
        );

        nodes.insert(
            "D".into(),
            DagNode {
                node_id: "D".into(),
                description: "Task D".into(),
                dependencies: vec!["B".into(), "C".into()],
                required_model: ModelPreference::LocalOnly,
                task_hint: None,
                expected_output: None,
                status: DagNodeStatus::Pending,
                agent_id: None,
            },
        );

        manager.active_graphs.insert(
            "graph_1".into(),
            ExecutionGraph {
                graph_id: "graph_1".into(),
                original_prompt: "Diamond Test".into(),
                nodes,
            },
        );

        // 1. Tick inicial: Solo A debe salir
        println!("[DEBUG] Tick 1...");
        let pcbs = manager.tick();
        assert_eq!(pcbs.len(), 1);
        assert_eq!(pcbs[0].process_name, "A");

        // 2. Finalizar A
        println!("[DEBUG] Handle A...");
        manager.handle_result(NodeResult {
            node_id: "A".into(),
            output: "Output from A".into(),
            status: DagNodeStatus::Completed,
        })?;

        // 3. Tick: B y C deben salir en PARALELO
        println!("[DEBUG] Tick 2 (B and C)...");
        let pcbs = manager.tick();
        assert_eq!(pcbs.len(), 2);
        let ids: Vec<String> = pcbs.iter().map(|p| p.process_name.clone()).collect();
        assert!(ids.contains(&"B".to_string()));
        assert!(ids.contains(&"C".to_string()));

        // 4. Finalizar B (D sigue bloqueado porque falta C)
        println!("[DEBUG] Handle B...");
        manager.handle_result(NodeResult {
            node_id: "B".into(),
            output: "Code B".into(),
            status: DagNodeStatus::Completed,
        })?;

        println!("[DEBUG] Tick 3 (Should be empty)...");
        assert!(manager.tick().is_empty());

        // 5. Finalizar C
        println!("[DEBUG] Handle C...");
        manager.handle_result(NodeResult {
            node_id: "C".into(),
            output: "Code C".into(),
            status: DagNodeStatus::Completed,
        })?;

        // 6. Tick: D debe salir con el contexto de B y C inyectado
        println!("[DEBUG] Tick 4 (D)...");
        let pcbs = manager.tick();
        println!("[DEBUG] Verifying PCBS len...");
        assert_eq!(pcbs.len(), 1);
        println!("[DEBUG] PCB len verified.");
        let pcb_d = &pcbs[0];
        assert_eq!(pcb_d.process_name, "D");

        // Verificar Context Forwarding (Join/Gather)
        assert_eq!(
            pcb_d
                .inlined_context
                .get("dependency_B")
                .context("dependency_B should be present")?,
            "Code B"
        );
        assert_eq!(
            pcb_d
                .inlined_context
                .get("dependency_C")
                .context("dependency_C should be present")?,
            "Code C"
        );

        println!("Diamond Graph Flow: SUCCESS. Task D gathered parallel context correctly.");
        Ok(())
    }
}
