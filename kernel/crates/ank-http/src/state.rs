use crate::rate_limiter::AuthRateLimiter;
use ank_core::{
    agents::orchestrator::AgentOrchestrator, chal::CognitiveHAL, citadel::identity::Citadel,
    pr_manager::WorkspaceWsEvent, router::syncer::CatalogSyncer, telemetry::TelemetryState,
    SchedulerEvent, StatePersistor,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub scheduler_tx: mpsc::Sender<SchedulerEvent>,
    pub event_broker: Arc<RwLock<HashMap<String, broadcast::Sender<ank_proto::v1::TaskEvent>>>>,
    pub citadel: Arc<Mutex<Citadel>>,
    pub hal: Arc<CognitiveHAL>,
    pub scribe: Arc<ank_core::scribe::ScribeManager>,
    pub router: Arc<RwLock<ank_core::router::CognitiveRouter>>,
    pub siren_router: Arc<ank_core::router::SirenRouter>,
    pub catalog_syncer: Option<Arc<CatalogSyncer>>,
    pub persistence: Arc<dyn StatePersistor>,
    pub config: crate::config::HttpConfig,
    pub auth_rate_limiter: AuthRateLimiter,
    pub telemetry: TelemetryState,
    pub tunnel_url: Arc<RwLock<Option<String>>>,
    /// CORE-158 (Epic 43): Orquestador del árbol de agentes jerárquico.
    pub agent_orchestrator: Arc<AgentOrchestrator>,
    /// CORE-175 (Epic 44): Broadcast channel para eventos del Developer Workspace.
    pub workspace_events: Arc<broadcast::Sender<WorkspaceWsEvent>>,
}
