use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use tonic::{Request, Response, Status};

use ank_core::{SchedulerEvent, PCB as CorePCB};
use ank_http::AppState;
use ank_proto::v1::kernel_service_server::KernelService;
use ank_proto::v1::{
    AdminSetupRequest, AdminSetupResponse, Empty, Pcb as ProtoPcb, ProcessList,
    ProcessState as ProtoProcessState, SystemStatus, TaskEvent, TaskRequest, TaskResponse,
    TaskSubscription, TenantCreateRequest, TenantCreateResponse,
};

#[derive(Clone, Debug)]
pub struct CitadelAuth {
    pub tenant_id: String,
    pub session_key: String,
    pub public_id: String,
}

pub struct AnkRpcServer {
    state: AppState,
}

impl AnkRpcServer {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }

    async fn validate_auth(&self, auth: &CitadelAuth) -> Result<(), Status> {
        let citadel = self.state.citadel.lock().await;

        // Try master first
        if let Ok(is_master) = citadel
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
        {
            if is_master {
                return Ok(());
            }
        }

        // Try tenant
        match citadel
            .enclave
            .authenticate_tenant(&auth.tenant_id, &auth.session_key)
            .await
        {
            Ok(true) => Ok(()),
            Err(e) if e.to_string().contains("PASSWORD_MUST_CHANGE") => {
                Err(Status::unauthenticated("PASSWORD_MUST_CHANGE"))
            }
            _ => Err(Status::unauthenticated(
                "Citadel AUTH_FAILURE: Access Denied.",
            )),
        }
    }
}

#[tonic::async_trait]
impl KernelService for AnkRpcServer {
    async fn submit_task(
        &self,
        request: Request<TaskRequest>,
    ) -> Result<Response<TaskResponse>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        self.validate_auth(&auth).await?;

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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        self.validate_auth(&auth).await?;

        let req = request.into_inner();
        let pid = req.pid;

        // In Aegis-Core, event_broker stores broadcast::Sender
        let (tx, rx) = mpsc::channel(100);
        let mut broker = self.state.event_broker.write().await;

        let broadcast_tx = broker.entry(pid.clone()).or_insert_with(|| {
            let (btx, _) = tokio::sync::broadcast::channel(1024);
            btx
        });

        let mut broadcast_rx = broadcast_tx.subscribe();

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
        _request: Request<Empty>,
    ) -> Result<Response<SystemStatus>, Status> {
        // Basic implementation for health check / status
        let is_init = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .admin_exists()
            .await
            .unwrap_or(false);

        Ok(Response::new(SystemStatus {
            state: if is_init { 1 } else { 0 }, // 1: Operational, 0: Initializing
            ..Default::default()
        }))
    }

    async fn list_processes(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ProcessList>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

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
            .filter(|p| p.tenant_id.as_deref() == Some(&auth.tenant_id))
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
        let citadel = self.state.citadel.lock().await;
        if citadel.enclave.admin_exists().await.unwrap_or(false) {
            return Err(Status::permission_denied("Already initialized"));
        }

        let req = request.into_inner();

        // Validate setup token if provided
        if !req.setup_token.is_empty() {
            let is_valid = citadel
                .enclave
                .validate_and_consume_setup_token(&req.setup_token)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            if !is_valid {
                return Err(Status::unauthenticated("Invalid setup token"));
            }
        }

        citadel
            .enclave
            .initialize_master(&req.username, &req.passphrase)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(AdminSetupResponse {
            success: true,
            message: "Master Admin initialized".to_string(),
        }))
    }

    async fn create_tenant(
        &self,
        request: Request<TenantCreateRequest>,
    ) -> Result<Response<TenantCreateResponse>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;

        let citadel = self.state.citadel.lock().await;
        let is_master = citadel
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);

        if !is_master {
            return Err(Status::permission_denied("Only Master can create tenants"));
        }

        let (port, pass) = citadel
            .enclave
            .create_tenant(&request.into_inner().username)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(TenantCreateResponse {
            success: true,
            temporary_passphrase: pass,
            network_port: port,
            ..Default::default()
        }))
    }

    // Rest of service methods can be empty or basic for now,
    // but these are the main ones needed for health/setup.
    async fn reset_tenant_password(
        &self,
        request: Request<ank_proto::v1::PasswordResetRequest>,
    ) -> Result<Response<Empty>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let req = request.into_inner();
        let citadel = self.state.citadel.lock().await;
        let is_master = citadel
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
        if !is_master {
            return Err(Status::permission_denied("Only Master can reset passwords"));
        }
        citadel
            .enclave
            .reset_tenant_password(&req.tenant_id, &req.new_passphrase)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let citadel = self.state.citadel.lock().await;
        let is_master = citadel
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
        if !is_master {
            return Err(Status::permission_denied("Only Master can list tenants"));
        }
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let req = request.into_inner();
        let citadel = self.state.citadel.lock().await;
        let is_master = citadel
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
        if !is_master {
            return Err(Status::permission_denied("Only Master can delete tenants"));
        }
        citadel
            .enclave
            .delete_tenant(&req.tenant_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Empty {}))
    }

    async fn add_global_key(
        &self,
        request: Request<ank_proto::v1::GlobalKeyRequest>,
    ) -> Result<Response<Empty>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let req = request.into_inner();
        let is_master = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
        if !is_master {
            return Err(Status::permission_denied("Only Master can add global keys"));
        }
        let entry = ank_core::router::key_pool::ApiKeyEntry {
            key_id: uuid::Uuid::new_v4().to_string(),
            provider: req.provider,
            api_key: req.api_key,
            api_url: req.api_url,
            label: req.label,
            is_active: true,
            rate_limited_until: None,
            active_models: None,
        };
        let router = self.state.router.read().await;
        router
            .add_global_key(entry)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Empty {}))
    }

    async fn list_global_keys(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let is_master = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let req = request.into_inner();
        let is_master = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
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
        Ok(Response::new(Empty {}))
    }

    async fn list_my_keys(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
        let is_master = self
            .state
            .citadel
            .lock()
            .await
            .enclave
            .authenticate_master(&auth.tenant_id, &auth.session_key)
            .await
            .unwrap_or(false);
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
        let auth = request
            .extensions()
            .get::<CitadelAuth>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
        self.validate_auth(&auth).await?;
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
