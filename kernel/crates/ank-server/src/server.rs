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
                    ank_core::ProcessState::WaitingSyscall => {
                        ProtoProcessState::StateBlocked as i32
                    }
                    ank_core::ProcessState::Completed => ProtoProcessState::StateCompleted as i32,
                    ank_core::ProcessState::Failed => ProtoProcessState::StateTerminated as i32,
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
        _req: Request<ank_proto::v1::PasswordResetRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
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
        Ok(Response::new(ank_proto::v1::SirenVoiceList {
            voices: vec![],
        }))
    }

    async fn configure_engine(
        &self,
        _req: Request<ank_proto::v1::EngineConfigRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_tenants(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::ListTenantsResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn delete_tenant(
        &self,
        _req: Request<ank_proto::v1::TenantDeleteRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn add_global_key(
        &self,
        _req: Request<ank_proto::v1::GlobalKeyRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_global_keys(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn delete_key(
        &self,
        _req: Request<ank_proto::v1::DeleteKeyRequest>,
    ) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_my_keys(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::KeyListResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn sync_router_catalog(&self, _req: Request<Empty>) -> Result<Response<Empty>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn list_router_models(
        &self,
        _req: Request<Empty>,
    ) -> Result<Response<ank_proto::v1::ModelListResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
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

    let tenant_id = match metadata.get("x-citadel-tenant") {
        Some(v) => v.to_str().map(|s| s.to_string()).unwrap_or_default(),
        None => return Ok(req),
    };

    let session_key = match metadata.get("x-citadel-key") {
        Some(v) => v.to_str().map(|s| s.to_string()).unwrap_or_default(),
        None => return Ok(req),
    };

    // simplified hashing to match BFF (actually it should be exactly what citadel.rs does)
    let session_key_hash = ank_http::citadel::hash_passphrase(&session_key);

    let mut req = req;
    req.extensions_mut().insert(CitadelAuth {
        tenant_id,
        session_key: session_key_hash,
        public_id: "obfuscated".to_string(), // Placeholder
    });

    Ok(req)
}
