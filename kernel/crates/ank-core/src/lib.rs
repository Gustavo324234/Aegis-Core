pub mod chal;
pub mod chronos;
pub mod citadel;
pub mod dag;
pub mod enclave;
pub mod oauth;
pub mod pcb;
pub mod plugins;
pub mod router;
pub mod scheduler;
pub mod scribe;
pub mod swarm; // Added pub mod swarm;
pub mod syscalls;
pub mod telemetry;
pub mod vcm;

// Re-exportar para fácil acceso
pub use chal::{CognitiveHAL, InferenceDriver, SystemError};
pub use chronos::ChronosDaemon;
pub use citadel::identity::Citadel;
pub use dag::{DagNode, DagNodeStatus, ExecutionGraph, GraphManager, NodeResult};
pub use enclave::{MasterEnclave, TenantDB};
pub use pcb::{ProcessState, TaskType, PCB};
pub use router::{CognitiveRouter, RoutingDecision, SirenEngine, SirenRouter};
pub use scheduler::persistence::{SQLCipherPersistor, StatePersistor};
pub use scheduler::{CognitiveScheduler, SchedulerEvent, SharedScheduler};
pub use scribe::diagnostic::DiagnosticLogger;
pub use swarm::SwarmManager;
pub use syscalls::{parse_syscall, Syscall}; // Added re-export for SwarmManager
