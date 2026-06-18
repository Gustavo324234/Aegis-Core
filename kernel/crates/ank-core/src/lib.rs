// release-please trigger: PR #277 squash landed with a non-Conventional
// title ("Fix/router chat supervisors") so release-please skipped it.
// This anchor lets a new Conventional commit re-associate the PR's
// changes with the ank-core package path.
pub mod agents;
pub mod chal;
pub mod chronos;
pub mod citadel;
pub mod dag;
pub mod enclave;
pub mod executor;
pub mod git;
pub mod oauth;
pub mod pcb;
pub mod plugins;
pub mod pr_manager;
pub mod router;
pub mod scheduler;
pub mod scribe;
pub mod speaker_id;
pub mod swarm; // Added pub mod swarm;
pub mod syscalls;
pub mod telemetry;
pub mod tunnel;
pub mod trainer;
pub mod vcm;
pub mod workspace;

// Re-exportar para fácil acceso
pub use chal::{CognitiveHAL, InferenceDriver, SystemError};
pub use trainer::{TrainingManager, TrainingConfig, TrainingStatus, TrainingProgress};
pub use chronos::ChronosDaemon;
pub use citadel::identity::Citadel;
pub use dag::{DagNode, DagNodeStatus, ExecutionGraph, GraphManager, NodeResult};
pub use enclave::{MasterEnclave, TenantDB};
pub use pcb::{ProcessRole, ProcessState, TaskType, PCB};
pub use router::{CognitiveRouter, RoutingDecision, SirenEngine, SirenRouter};
pub use scheduler::persistence::{SQLCipherPersistor, StatePersistor};
pub use scheduler::{CognitiveScheduler, SchedulerEvent, SchedulerStats, SharedScheduler};
pub use scribe::diagnostic::DiagnosticLogger;
pub use swarm::SwarmManager;
pub use syscalls::{parse_syscall, Syscall}; // Added re-export for SwarmManager
pub use tunnel::TunnelClient;
