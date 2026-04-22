use crate::rate_limiter::AuthRateLimiter;
use ank_core::{
    chal::CognitiveHAL, citadel::identity::Citadel, router::syncer::CatalogSyncer,
    telemetry::TelemetryState, SchedulerEvent, StatePersistor,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub scheduler_tx: mpsc::Sender<SchedulerEvent>,
    pub event_broker: Arc<RwLock<HashMap<String, broadcast::Sender<ank_proto::v1::TaskEvent>>>>,
    pub citadel: Arc<Mutex<Citadel>>,
    pub hal: Arc<RwLock<CognitiveHAL>>,
    pub router: Arc<RwLock<ank_core::router::CognitiveRouter>>,
    pub siren_router: Arc<ank_core::router::SirenRouter>,
    pub catalog_syncer: Option<Arc<CatalogSyncer>>,
    pub persistence: Arc<dyn StatePersistor>,
    pub config: crate::config::HttpConfig,
    pub auth_rate_limiter: AuthRateLimiter,
    pub telemetry: TelemetryState,
    pub tunnel_url: Arc<RwLock<Option<String>>>,
}
