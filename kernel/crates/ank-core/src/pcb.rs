use crate::scheduler::ModelPreference;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskType {
    #[default]
    Chat,
    Coding,
    Planning,
    Analysis,
    Summarization,
    Extraction,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessState {
    New,
    Ready,
    Running,
    WaitingSyscall,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgramCounter {
    pub dag_id: String,
    pub current_node: String,
    pub total_nodes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPointers {
    pub l1_instruction: String,
    pub l2_context_refs: Vec<String>,
    pub swap_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registers {
    pub accumulator: String,
    pub temp_vars: HashMap<String, String>,
    pub sys_error_traceback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionMetrics {
    pub tokens_consumed: u64,
    pub cycles_executed: u32,
    pub max_cycles_allowed: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PCB {
    pub pid: String,
    pub parent_pid: Option<String>,
    pub process_name: String,
    pub created_at: DateTime<Utc>,
    pub state: ProcessState,
    pub priority: u32,
    pub program_counter: ProgramCounter,
    pub memory_pointers: MemoryPointers,
    pub registers: Registers,
    pub execution_metrics: ExecutionMetrics,
    pub model_pref: ModelPreference,
    #[serde(default)]
    pub task_type: TaskType,
    /// Archivos o contenido empaquetado para migración (Teleportación)
    #[serde(default)]
    pub inlined_context: HashMap<String, String>,
    // --- Multi-Tenant & Zero-Knowledge ---
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub public_id: Option<String>, // Added for secure logging (ANK-2410)
    #[serde(default)]
    pub session_key: Option<String>, // Sensitive: Avoid logging this!
    #[serde(default)]
    pub teleport_token: Option<String>, // OTP for secure node-to-node migration
}

impl std::fmt::Debug for PCB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PCB")
            .field("pid", &self.pid)
            .field("tenant_id", &"***REDACTED***")
            .field("public_id", &self.public_id)
            .field(
                "session_key",
                &self.session_key.as_ref().map(|_| "***REDACTED***"),
            )
            .field(
                "teleport_token",
                &self.teleport_token.as_ref().map(|_| "***REDACTED***"),
            )
            .field("state", &self.state)
            .field("priority", &self.priority)
            .field("process_name", &self.process_name)
            .finish()
    }
}

impl PCB {
    pub fn new(name: String, priority: u32, l1_prompt: String) -> Self {
        Self {
            pid: format!("proc_{}", &Uuid::new_v4().to_string()[..8]),
            parent_pid: None,
            process_name: name,
            created_at: Utc::now(),
            state: ProcessState::New,
            priority,
            program_counter: ProgramCounter {
                dag_id: "pending".to_string(),
                current_node: "start".to_string(),
                total_nodes: 0,
            },
            memory_pointers: MemoryPointers {
                l1_instruction: l1_prompt,
                l2_context_refs: Vec::new(),
                swap_refs: Vec::new(),
            },
            registers: Registers {
                accumulator: String::new(),
                temp_vars: HashMap::new(),
                sys_error_traceback: None,
            },
            execution_metrics: ExecutionMetrics {
                tokens_consumed: 0,
                cycles_executed: 0,
                max_cycles_allowed: 15,
            },
            model_pref: ModelPreference::HybridSmart,
            task_type: TaskType::default(),
            inlined_context: HashMap::new(),
            tenant_id: None,
            public_id: None,
            session_key: None,
            teleport_token: None,
        }
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

// Implementación de ordenamiento para BinaryHeap (Priority Queue)
// Rust's BinaryHeap es un Max-Heap. Prioridad 10 > Prioridad 0.
impl Ord for PCB {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.created_at.cmp(&other.created_at).reverse()) // Si empate, el más antiguo primero
    }
}

impl PartialOrd for PCB {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn test_pcb_creation_and_state_change() {
        let name = "TestProcess".to_string();
        let mut pcb = PCB::new(name.clone(), 5, "Prompt".to_string());

        assert_eq!(pcb.process_name, name);
        assert_eq!(pcb.state, ProcessState::New);
        assert_eq!(pcb.priority, 5);
        assert!(pcb.pid.starts_with("proc_"));

        // Cambio de estado
        pcb.state = ProcessState::WaitingSyscall;
        assert_eq!(pcb.state, ProcessState::WaitingSyscall);

        // Verificar inmutabilidad de otros campos (manualmente)
        assert_eq!(pcb.priority, 5);
        assert_eq!(pcb.process_name, name);
    }

    #[test]
    fn test_pcb_serialization() -> anyhow::Result<()> {
        let pcb = PCB::new("SerializeTest".to_string(), 10, "prompt".to_string());
        let json = pcb.to_json().context("Failed to serialize")?;

        let deserialized: PCB = PCB::from_json(&json).context("Failed to deserialize")?;
        assert_eq!(pcb.pid, deserialized.pid);
        assert_eq!(pcb.priority, deserialized.priority);
        assert_eq!(
            pcb.memory_pointers.l1_instruction,
            deserialized.memory_pointers.l1_instruction
        );
        Ok(())
    }

    #[test]
    fn test_pcb_ordering() {
        let pcb_low = PCB::new("Low".to_string(), 1, "low".to_string());
        let pcb_high = PCB::new("High".to_string(), 10, "high".to_string());

        // En nuestro BinaryHeap de Rust, el mayor va primero.
        assert!(pcb_high > pcb_low);
    }

    #[test]
    fn test_pcb_deserialization_compatibility() -> anyhow::Result<()> {
        // This JSON is missing task_type and newer fields
        let json = r#"{
            "pid": "proc_old",
            "process_name": "OldProc",
            "created_at": "2026-03-27T00:00:00Z",
            "state": "New",
            "priority": 5,
            "program_counter": {
                "dag_id": "dag-1",
                "current_node": "start",
                "total_nodes": 10
            },
            "memory_pointers": {
                "l1_instruction": "prompt",
                "l2_context_refs": [],
                "swap_refs": []
            },
            "registers": {
                "accumulator": "",
                "temp_vars": {}
            },
            "execution_metrics": {
                "tokens_consumed": 0,
                "cycles_executed": 0,
                "max_cycles_allowed": 15
            },
            "model_pref": "HybridSmart",
            "inlined_context": {}
        }"#;

        let pcb: PCB = serde_json::from_str(json).context("Failed to deserialize old PCB")?;
        assert_eq!(pcb.pid, "proc_old");
        assert_eq!(pcb.task_type, TaskType::Chat); // Should default to Chat
        assert_eq!(pcb.public_id, None); // Should default to None
        Ok(())
    }
}
