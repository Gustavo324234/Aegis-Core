use std::pin::Pin;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use tonic::{Request, Response, Status};

use ank_core::enclave::MasterEnclave;
use ank_core::{SchedulerEvent, PCB as CorePCB};
use ank_http::AppState;
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{
    AdminSetupRequest, AdminSetupResponse, Empty, Pcb as ProtoPcb, ProcessList,
    ProcessState as ProtoProcessState, SystemStatus, TaskEvent, TaskRequest, TaskResponse,
    TaskSubscription, TenantCreateRequest, TenantCreateResponse,
};

const MIN_GRPC_PASSPHRASE_CHARS: usize = 6;
static GRPC_PROCESS_START: OnceLock<Instant> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct CitadelAuth {
    pub tenant_id: String,
    pub session_key: String,
    pub public_id: String,
}

pub struct AnkRpcServer {
    state: AppState,
}

fn format_uptime(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem = secs % 60;
    format!("{}h {}m {}s", hours, mins, rem)
}

async fn authenticate_enclave_auth(
    enclave: &MasterEnclave,
    auth: &CitadelAuth,
) -> Result<bool, Status> {
    if let Ok(true) = enclave
        .authenticate_master(&auth.tenant_id, &auth.session_key)
        .await
    {
        return Ok(true);
    }

    match enclave
        .authenticate_tenant(&auth.tenant_id, &auth.session_key)
        .await
    {
        Ok(true) => Ok(false),
        Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => {
            Err(Status::unauthenticated("PASSWORD_MUST_CHANGE"))
        }
        _ => Err(Status::unauthenticated(
            "Citadel AUTH_FAILURE: Access Denied.",
        )),
    }
}

async fn bootstrap_grpc_master(
    enclave: &MasterEnclave,
    username: &str,
    raw_passphrase: &str,
    setup_token: &str,
) -> Result<(), Status> {
    if enclave
        .admin_exists()
        .await
        .map_err(|e| Status::internal(e.to_string()))?
    {
        return Err(Status::permission_denied("Already initialized"));
    }

    let trimmed_user = username.trim();
    if trimmed_user.is_empty() {
        return Err(Status::invalid_argument("Username cannot be empty"));
    }
    if raw_passphrase.trim().is_empty()
        || raw_passphrase.chars().count() < MIN_GRPC_PASSPHRASE_CHARS
    {
        return Err(Status::invalid_argument(format!(
            "Passphrase must be at least {} characters long",
            MIN_GRPC_PASSPHRASE_CHARS
        )));
    }

    // CORE-370: Setup token is strictly mandatory (never skip validation on empty string).
    let trimmed_token = setup_token.trim();
    if trimmed_token.is_empty() {
        return Err(Status::unauthenticated("Setup token is required"));
    }

    let is_valid = enclave
        .validate_setup_token_only(trimmed_token)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    if !is_valid {
        return Err(Status::unauthenticated("Invalid or expired setup token"));
    }

    // CORE-370: Hash raw passphrase with SHA-256 to match Citadel Enclave contract.
    let pass_hash = ank_http::citadel::hash_passphrase(raw_passphrase);
    enclave
        .initialize_master(trimmed_user, &pass_hash)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    enclave
        .consume_setup_token(trimmed_token)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(())
}

async fn reset_grpc_tenant_password(
    enclave: &MasterEnclave,
    tenant_id: &str,
    raw_new_passphrase: &str,
) -> Result<(), Status> {
    let trimmed_tid = tenant_id.trim();
    if trimmed_tid.is_empty() {
        return Err(Status::invalid_argument("tenant_id cannot be empty"));
    }
    if raw_new_passphrase.trim().is_empty()
        || raw_new_passphrase.chars().count() < MIN_GRPC_PASSPHRASE_CHARS
    {
        return Err(Status::invalid_argument(format!(
            "New passphrase must be at least {} characters long",
            MIN_GRPC_PASSPHRASE_CHARS
        )));
    }

    let new_hash = ank_http::citadel::hash_passphrase(raw_new_passphrase);
    enclave
        .reset_tenant_password(trimmed_tid, &new_hash)
        .await
        .map_err(|e| Status::internal(e.to_string()))
}

#[allow(clippy::result_large_err)]
fn authorize_watch_pid(
    processes: &[CorePCB],
    pid: &str,
    caller_tenant: &str,
    is_master: bool,
) -> Result<(), Status> {
    let trimmed_pid = pid.trim();
    if trimmed_pid.is_empty() {
        return Err(Status::invalid_argument("PID cannot be empty"));
    }

    let Some(pcb) = processes.iter().find(|p| p.pid == trimmed_pid) else {
        return Err(Status::not_found("Process not found"));
    };

    if !is_master && pcb.tenant_id.as_deref() != Some(caller_tenant) {
        return Err(Status::permission_denied(
            "Citadel isolation: cannot watch another tenant's process",
        ));
    }

    Ok(())
}

impl AnkRpcServer {
    pub fn from_state(state: &AppState) -> Self {
        let _ = GRPC_PROCESS_START.get_or_init(Instant::now);
        Self {
            state: state.clone(),
        }
    }

    async fn validate_auth(&self, auth: &CitadelAuth) -> Result<bool, Status> {
        let citadel = self.state.citadel.lock().await;
        authenticate_enclave_auth(&citadel.enclave, auth).await
    }

    async fn authenticate_request<T>(
        &self,
        request: &Request<T>,
    ) -> Result<(CitadelAuth, bool), Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;
        let is_master = self.validate_auth(&auth).await?;
        Ok((auth, is_master))
    }
}

#[tonic::async_trait]
impl KernelService for AnkRpcServer {
    async fn submit_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        let (auth, _is_master) = self.authenticate_request(&request).await?;

        let req = request.into_inner();
        let mut core_pcb = CorePCB::new("Remote Task".to_string(), 5, req.prompt);
        core_pcb.tenant_id = Some(auth.tenant_id.clone());
        core_pcb.public_id = Some(auth.public_id.clone());
        core_pcb.session_key = Some(auth.session_key.clone());

        let pid = core_pcb.pid.clone();

        if let Err(e) = self
            .state
            .scheduler_tx
            .send(SchedulerEvent::ScheduleTask(Box::new(core_pcb)))
            .await
        {
            return Err(Status::internal(format!("Failed to register task: {}", e)));
        }

        Ok(Response::new(TaskResponse {
            pid,
            accepted: true,
            message: "Task successfully submitted to Cognitive Scheduler".to_string(),
        }))
    }

    type WatchTaskStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<TaskEvent, Status>> + Send>>;

    async fn watch_task(
        &self,
        request: Request<TaskSubscription>,
    ) -> Result<Response<Self::WatchTaskStream>, Status> {
        let (auth, is_master) = self.authenticate_request(&request).await?;

        let req = request.into_inner();
        let pid = req.pid.trim().to_string();

        // CORE-370: Verify that `pid` exists in the scheduler AND belongs to `auth.tenant_id`
        // before touching `event_broker` (eliminates cross-tenant IDOR and unbounded channel DoS).
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state
            .scheduler_tx
            .send(SchedulerEvent::ListProcesses(reply_tx))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let active_processes = reply_rx
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        authorize_watch_pid(&active_processes, &pid, &auth.tenant_id, is_master)?;

        let (tx, rx) = mpsc::channel(100);
        let mut broadcast_rx = {
            let mut broker = self.state.event_broker.write().await;
            let broadcast_tx = broker.entry(pid).or_insert_with(|| {
                let (btx, _) = tokio::sync::broadcast::channel(1024);
                btx
            });
            broadcast_tx.subscribe()
        };

        tokio::spawn(async move {
            while let Ok(event) = broadcast_rx.recv().await {
                if tx.send(Ok(event)).await.is_err() {
                    break;
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::WatchTaskStream))
    }

    async fn get_system_status(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        let start = GRPC_PROCESS_START.get_or_init(Instant::now);

        let authorized_telemetry = if let Some(auth) = request.extensions().get::<CitadelAuth>() {
            self.validate_auth(auth).await?;
            true
        } else {
            request
                .remote_addr()
                .map(|addr| addr.ip().is_loopback())
                .unwrap_or(false)
        };

        let is_init = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .admin_exists()
            .await
            .unwrap_or(false);

        let state_code = if is_init { 1 } else { 0 };
        if !authorized_telemetry {
            return Ok(Response::new(SystemStatus {
                state: state_code,
                ..Default::default()
            }));
        }

        let (cpu_load, vram_allocated_mb, vram_total_mb) = {
            let mut monitor = self.state.hal.hardware.lock().await;
            let hw = monitor.get_status();
            (hw.cpu_usage, hw.used_mem_mb, hw.total_mem_mb)
        };

        let (total_processes, active_workers) = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            if self
                .state
                .scheduler_tx
                .send(SchedulerEvent::GetStats(tx))
                .await
                .is_ok()
            {
                if let Ok(stats) = rx.await {
                    (stats.total_processes, stats.active_workers)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            }
        };

        Ok(Response::new(SystemStatus {
            cpu_load,
            vram_allocated_mb: vram_allocated_mb as f32,
            vram_total_mb: vram_total_mb as f32,
            total_processes,
            active_workers,
            uptime: format_uptime(start.elapsed().as_secs()),
            loaded_models: Vec::new(),
            state: state_code,
        }))
    }

    async fn list_processes(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ProcessList>, Status> {
        // CORE-370: Enforce enclave credential verification (previously skipped!).
        let (auth, is_master) = self.authenticate_request(&request).await?;

        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.state
            .scheduler_tx
            .send(SchedulerEvent::ListProcesses(reply_tx))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let core_processes = reply_rx
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let processes = core_processes
            .into_iter()
            .filter(|p| is_master || p.tenant_id.as_deref() == Some(auth.tenant_id.as_str()))
            .map(|p| ProtoPcb {
                pid: p.pid,
                process_name: p.process_name,
                state: match p.state {
                    ank_core::ProcessState::New | ank_core::ProcessState::Ready => {
                        ProtoProcessState::StatePending as i32
                    }
                    ank_core::ProcessState::Running => ProtoProcessState::StateRunning as i32,
                    ank_core::ProcessState::WaitingSyscall
                    | ank_core::ProcessState::WaitingWorkers => {
                        ProtoProcessState::StateBlocked as i32
                    }
                    ank_core::ProcessState::Completed => ProtoProcessState::StateCompleted as i32,
                    ank_core::ProcessState::Failed => ProtoProcessState::StateTerminated as i32,
                    ank_core::ProcessState::Preempted => ProtoProcessState::StateBlocked as i32,
                },
                ..Default::default()
            })
            .collect();

        Ok(Response::new(ProcessList { processes }))
    }

    async fn initialize_master_admin(
        &self,
        request: Request<AdminSetupRequest>,
    ) -> Result<Response<AdminSetupResponse>, Status> {
        let req = request.into_inner();
        let citadel = self.state.citadel.lock().await;
        bootstrap_grpc_master(
            &citadel.enclave,
            &req.username,
            &req.passphrase,
            &req.setup_token,
        )
        .await?;

        Ok(Response::new(AdminSetupResponse {
            success: true,
            message: "Master Admin initialized".to_string(),
        }))
    }

    async fn create_tenant(
        &self,
        request: Request<TenantCreateRequest>,
    ) -> Result<Response<TenantCreateResponse>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can create tenants"));
        }

        let username = request.into_inner().username.trim().to_string();
        if username.is_empty() {
            return Err(Status::invalid_argument("Username cannot be empty"));
        }

        let citadel = self.state.citadel.lock().await;
        let (port, pass) = citadel
            .enclave
            .create_tenant(&username)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(TenantCreateResponse {
            success: true,
            tenant_id: username.clone(),
            temporary_passphrase: pass,
            network_port: port,
            message: format!("Tenant '{}' created", username),
        }))
    }

    async fn reset_tenant_password(
        &self,
        request: Request<ank_proto::v1::PasswordResetRequest>,
    ) -> Result<Response<Empty>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can reset passwords"));
        }

        let req = request.into_inner();
        let citadel = self.state.citadel.lock().await;
        reset_grpc_tenant_password(&citadel.enclave, &req.tenant_id, &req.new_passphrase).await?;
        Ok(Response::new(Empty {}))
    }

    type TeleportProcessStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<TaskEvent, Status>> + Send>>;
    async fn teleport_process(
        &self,
        _req: Request<ProtoPcb>,
    ) -> Result<Response<Self::TeleportProcessStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_siren_voices(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::SirenVoiceList>, Status> {
        // TODO(CORE-080-P3): post-launch
        Ok(Response::new(ank_proto::v1::SirenVoiceList {
            voices: vec![],
        }))
    }

    async fn configure_engine(
        &self,
        _req: Request<ank_proto::v1::EngineConfigRequest>,
    ) -> Result<Response<Empty>, Status> {
        // TODO(CORE-080-P3): post-launch
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_tenants(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::ListTenantsResponse>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can list tenants"));
        }
        let citadel = self.state.citadel.lock().await;
        let tenants = citadel
            .enclave
            .list_tenants()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ank_proto::v1::ListTenantsResponse {
            tenants,
        }))
    }

    async fn delete_tenant(
        &self,
        request: Request<ank_proto::v1::TenantDeleteRequest>,
    ) -> Result<Response<Empty>, Status> {
        let (auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can delete tenants"));
        }

        let target_id = request.into_inner().tenant_id.trim().to_string();
        if target_id.is_empty() {
            return Err(Status::invalid_argument("tenant_id cannot be empty"));
        }
        if target_id == auth.tenant_id {
            return Err(Status::invalid_argument(
                "Cannot delete active master admin account",
            ));
        }

        {
            let citadel = self.state.citadel.lock().await;
            citadel
                .enclave
                .delete_tenant(&target_id)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

        {
            let router = self.state.router.read().await;
            if let Err(e) = router.purge_tenant_keys(&target_id).await {
                tracing::warn!(
                    tenant_id = %target_id,
                    error = %e,
                    "Failed to purge tenant keys from KeyPool during gRPC delete_tenant"
                );
            }
            router.invalidate_sticky(&target_id).await;
        }
        ank_http::ws::chat::invalidate_onboarding_cache(&target_id);

        Ok(Response::new(Empty {}))
    }

    async fn add_global_key(
        &self,
        request: Request<ank_proto::v1::GlobalKeyRequest>,
    ) -> Result<Response<Empty>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can add global keys"));
        }
        let req = request.into_inner();
        let entry = ank_core::router::key_pool::ApiKeyEntry {
            key_id: uuid::Uuid::new_v4().to_string(),
            provider: ank_core::router::normalize_provider_id(&req.provider),
            api_key: req.api_key,
            api_url: req.api_url,
            label: req.label,
            is_active: true,
            rate_limited_until: None,
            active_models: None,
            is_free_tier: false,
        };
        let router = self.state.router.read().await;
        router
            .add_global_key(entry)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        router.invalidate_all_sticky().await;
        Ok(Response::new(Empty {}))
    }

    async fn list_global_keys(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied(
                "Only Master can list global keys",
            ));
        }
        let router = self.state.router.read().await;
        let keys = router
            .list_global_keys()
            .await
            .into_iter()
            .map(|k| ank_proto::v1::KeyInfo {
                key_id: k.key_id,
                provider: k.provider,
                api_key: "***".to_string(),
                api_url: k.api_url,
                label: k.label,
                is_active: k.is_active,
                rate_limited_until: None,
            })
            .collect();
        Ok(Response::new(ank_proto::v1::KeyListResponse { keys }))
    }

    async fn delete_key(
        &self,
        request: Request<ank_proto::v1::DeleteKeyRequest>,
    ) -> Result<Response<Empty>, Status> {
        let (auth, is_master) = self.authenticate_request(&request).await?;
        let req = request.into_inner();
        let tenant_filter = if is_master {
            req.tenant_id.as_deref()
        } else {
            Some(auth.tenant_id.as_str())
        };
        let router = self.state.router.read().await;
        router
            .delete_key(&req.key_id, tenant_filter)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        if let Some(tid) = tenant_filter {
            router.invalidate_sticky(tid).await;
        } else {
            router.invalidate_all_sticky().await;
        }
        Ok(Response::new(Empty {}))
    }

    async fn list_my_keys(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        let (auth, _is_master) = self.authenticate_request(&request).await?;
        let router = self.state.router.read().await;
        let keys = router
            .list_tenant_keys(&auth.tenant_id)
            .await
            .into_iter()
            .map(|k| ank_proto::v1::KeyInfo {
                key_id: k.key_id,
                provider: k.provider,
                api_key: "***".to_string(),
                api_url: k.api_url,
                label: k.label,
                is_active: k.is_active,
                rate_limited_until: None,
            })
            .collect();
        Ok(Response::new(ank_proto::v1::KeyListResponse { keys }))
    }

    async fn sync_router_catalog(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<Empty>, Status> {
        let (_auth, is_master) = self.authenticate_request(&request).await?;
        if !is_master {
            return Err(Status::permission_denied("Only Master can sync catalog"));
        }

        if let Some(syncer) = &self.state.catalog_syncer {
            syncer
                .sync_now()
                .await
                .map_err(|e| Status::internal(format!("Sync failed: {}", e)))?;
        }

        Ok(Response::new(Empty {}))
    }

    async fn list_router_models(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::ModelListResponse>, Status> {
        let (_auth, _is_master) = self.authenticate_request(&request).await?;
        let router = self.state.router.read().await;
        let models = router
            .list_models_for_catalog()
            .await
            .into_iter()
            .map(|m| ank_proto::v1::ModelInfo {
                model_id: m.model_id,
                provider: m.provider,
                display_name: m.display_name,
                context_window: m.context_window,
                cost_input_per_mtok: m.cost_input_per_mtok as f32,
                cost_output_per_mtok: m.cost_output_per_mtok as f32,
                is_local: m.is_local,
                task_scores: Some(ank_proto::v1::ModelTaskScores {
                    chat: m.task_scores.chat as u32,
                    coding: m.task_scores.coding as u32,
                    planning: m.task_scores.planning as u32,
                    analysis: m.task_scores.analysis as u32,
                }),
            })
            .collect();
        Ok(Response::new(ank_proto::v1::ModelListResponse {
            models,
            synced_at: String::new(),
        }))
    }

    async fn get_siren_config(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::SirenConfig>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn set_siren_config(
        &self,
        _req: Request<ank_proto::v1::SirenConfigRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}

#[allow(clippy::result_large_err)]
pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();

    let tenant_id = metadata
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let session_key = metadata
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match (tenant_id, session_key) {
        (Some(tid), Some(key)) => {
            let hash = ank_http::citadel::hash_passphrase(&key);
            let mut req = req;
            req.extensions_mut().insert(CitadelAuth {
                tenant_id: tid,
                session_key: hash,
                public_id: "obfuscated".to_string(),
            });
            Ok(req)
        }
        // No headers at all — public request, handler decides if auth is required
        (None, None) => Ok(req),
        // Partial headers — Citadel Protocol violation
        _ => Err(Status::unauthenticated(
            "Citadel Protocol violation: partial credentials",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    #[tokio::test]
    async fn test_grpc_bootstrap_requires_setup_token_and_hashes_passphrase() -> anyhow::Result<()>
    {
        let enclave = MasterEnclave::new_in_memory().await?;
        enclave.store_setup_token("SETUP-GRPC-TOKEN", 15).await?;

        // Empty setup_token must be rejected (no silent bypass!)
        let empty_err = bootstrap_grpc_master(&enclave, "admin", "supersecret", "").await;
        assert_eq!(empty_err.map_err(|s| s.code()), Err(Code::Unauthenticated));
        assert!(!enclave.admin_exists().await?);

        // Wrong setup_token must be rejected
        let wrong_err = bootstrap_grpc_master(&enclave, "admin", "supersecret", "SETUP-BAD").await;
        assert_eq!(wrong_err.map_err(|s| s.code()), Err(Code::Unauthenticated));
        assert!(!enclave.admin_exists().await?);

        // Valid setup_token initializes Master Admin with SHA-256 pre-hash
        bootstrap_grpc_master(&enclave, "admin", "supersecret", "SETUP-GRPC-TOKEN")
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        assert!(enclave.admin_exists().await?);

        // Forged session key must fail `authenticate_enclave_auth`
        let forged = CitadelAuth {
            tenant_id: "admin".to_string(),
            session_key: ank_http::citadel::hash_passphrase("wrong-key"),
            public_id: "obfuscated".to_string(),
        };
        assert_eq!(
            authenticate_enclave_auth(&enclave, &forged)
                .await
                .map_err(|s| s.code()),
            Err(Code::Unauthenticated)
        );

        // Real session key injected via `auth_interceptor` hashes identically and authenticates as Master
        let valid = CitadelAuth {
            tenant_id: "admin".to_string(),
            session_key: ank_http::citadel::hash_passphrase("supersecret"),
            public_id: "obfuscated".to_string(),
        };
        let is_master = authenticate_enclave_auth(&enclave, &valid)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        assert!(is_master);

        Ok(())
    }

    #[tokio::test]
    async fn test_grpc_reset_tenant_password_hashes_properly() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;
        let (_port, _temp_pass) = enclave.create_tenant("alice").await?;

        reset_grpc_tenant_password(&enclave, "alice", "newpass123")
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let alice_auth = CitadelAuth {
            tenant_id: "alice".to_string(),
            session_key: ank_http::citadel::hash_passphrase("newpass123"),
            public_id: "obfuscated".to_string(),
        };
        let is_master = authenticate_enclave_auth(&enclave, &alice_auth)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        assert!(!is_master);
        Ok(())
    }

    #[test]
    fn test_authorize_watch_pid_blocks_idor_and_unknown_pids() {
        let mut alice_pcb = CorePCB::new("Alice Task".to_string(), 5, "hello".to_string());
        alice_pcb.pid = "pid-alice-1".to_string();
        alice_pcb.tenant_id = Some("alice".to_string());

        let processes = vec![alice_pcb];

        // Unknown PID rejected with NotFound (prevents unbounded event_broker allocation)
        assert_eq!(
            authorize_watch_pid(&processes, "pid-ghost", "alice", false).map_err(|s| s.code()),
            Err(Code::NotFound)
        );

        // Tenant B attempting to watch Tenant A's PID rejected with PermissionDenied
        assert_eq!(
            authorize_watch_pid(&processes, "pid-alice-1", "bob", false).map_err(|s| s.code()),
            Err(Code::PermissionDenied)
        );

        // Owner and Master Admin succeed
        assert!(authorize_watch_pid(&processes, "pid-alice-1", "alice", false).is_ok());
        assert!(authorize_watch_pid(&processes, "pid-alice-1", "admin", true).is_ok());
    }
}
