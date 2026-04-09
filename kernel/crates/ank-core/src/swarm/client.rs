use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
use tracing::{info, warn};

use crate::pcb::PCB;
use crate::scheduler::SchedulerEvent;
use ank_proto::v1::kernel_service_client::KernelServiceClient;
use ank_proto::v1::{Pcb, ProcessState as ProtoProcessState};

#[derive(Error, Debug)]
pub enum SwarmError {
    #[error("Connection refused for node {0}:{1}")]
    ConnectionRefused(String, u16),

    #[error("Transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),

    #[error("RPC error: {0}")]
    RpcError(#[from] Box<tonic::Status>),

    #[error("Teleportation timeout")]
    Timeout,

    #[error("Internal conversion error: {0}")]
    ConversionError(String),

    #[error("Security context error: {0}")]
    SecurityError(String),
}

impl From<tonic::Status> for SwarmError {
    fn from(s: tonic::Status) -> Self {
        Self::RpcError(Box::new(s))
    }
}

/// Cliente gRPC para la teletransportación de procesos entre nodos del Swarm.
pub struct SwarmClient;

impl SwarmClient {
    /// Teletransporta un PCB a un nodo remoto.
    /// Inicia un stream de eventos que se re-inyectan en el Scheduler local.
    pub async fn teleport(
        &self,
        target_ip: &str,
        target_port: u16,
        pcb: PCB,
        event_tx: mpsc::Sender<SchedulerEvent>,
    ) -> Result<(), SwarmError> {
        let root_key = std::env::var("AEGIS_ROOT_KEY")
            .map_err(|_| SwarmError::SecurityError("Missing AEGIS_ROOT_KEY".to_string()))?;

        // Configuración mTLS ANK-SEC-006
        let cert_path =
            std::env::var("AEGIS_TLS_CERT_PATH").unwrap_or_else(|_| "tls/server.crt".to_string());
        let key_path =
            std::env::var("AEGIS_TLS_KEY_PATH").unwrap_or_else(|_| "tls/server.key".to_string());
        let ca_path =
            std::env::var("AEGIS_TLS_CA_PATH").unwrap_or_else(|_| "tls/ca.crt".to_string());

        let mut endpoint = if target_ip == "127.0.0.1" || target_ip == "localhost" {
            // Local testing typically doesn't use TLS unless explicitly configured
            Endpoint::from_shared(format!("http://{}:{}", target_ip, target_port))?
        } else {
            let uri = format!("https://{}:{}", target_ip, target_port);
            let mut ep = Endpoint::from_shared(uri)?;

            if let (Ok(cert), Ok(key), Ok(ca)) = (
                tokio::fs::read(&cert_path).await,
                tokio::fs::read(&key_path).await,
                tokio::fs::read(&ca_path).await,
            ) {
                info!(
                    "Configuring mTLS for swarm connection to {}:{}",
                    target_ip, target_port
                );
                let identity = Identity::from_pem(cert, key);
                let ca_cert = Certificate::from_pem(ca);
                let tls_config = ClientTlsConfig::new()
                    .domain_name(target_ip) // Requirement for Tonic/Rustls
                    .identity(identity)
                    .ca_certificate(ca_cert);
                ep = ep.tls_config(tls_config)?;
            } else {
                let strict_mode =
                    std::env::var("AEGIS_MTLS_STRICT").unwrap_or_else(|_| "true".to_string());
                if strict_mode.to_lowercase() == "true" {
                    return Err(SwarmError::SecurityError(format!(
                        "mTLS Strict Mode is enabled (AEGIS_MTLS_STRICT=true) but certificates are missing at {}. Plaintext fallback is disabled.",
                        cert_path
                    )));
                }

                warn!(
                    "mTLS certs not found at {}, falling back to plaintext (Insecure!)",
                    cert_path
                );
            }
            ep
        };

        endpoint = endpoint
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(30));

        info!(
            "Connecting to target node for teleportation: {}:{}",
            target_ip, target_port
        );

        let mut client = KernelServiceClient::connect(endpoint)
            .await
            .map_err(|_| SwarmError::ConnectionRefused(target_ip.to_string(), target_port))?;

        // Conversión del PCB nativo al formato Protobuf e inyección de OTP
        let proto_pcb = self.convert_pcb_to_proto(&pcb, root_key.as_bytes())?;
        let remote_pid = pcb.pid.clone();

        info!(pid = %remote_pid, "Initiating PCB teleportation...");

        // Llamada RPC para teletransportar el proceso
        let response = client.teleport_process(proto_pcb).await?;
        let mut stream = response.into_inner();

        // Bucle de recepción del Stream de eventos.
        tokio::spawn(async move {
            info!(pid = %remote_pid, "Receiving teleported process events...");

            while let Ok(Some(event)) = stream.message().await {
                let local_event = SchedulerEvent::RemoteEvent(remote_pid.clone(), event);
                if event_tx.send(local_event).await.is_err() {
                    warn!(pid = %remote_pid, "Scheduler event receiver closed. Dropping remote events.");
                    break;
                }
            }

            warn!(pid = %remote_pid, "Teleported process stream ended.");
        });

        Ok(())
    }

    /// Helper para convertir la estructura interna de ANK al contrato de Protobuf.
    fn convert_pcb_to_proto(&self, pcb: &PCB, root_key: &[u8]) -> Result<Pcb, SwarmError> {
        let tenant_id = pcb.tenant_id.as_ref().ok_or_else(|| {
            SwarmError::ConversionError("PCB missing tenant_id for teleportation".to_string())
        })?;

        // Generar OTP para la migración segura (ANK-SEC-006)
        let otp = crate::swarm::otp::generate_teleport_otp(tenant_id, root_key)
            .map_err(|e| SwarmError::SecurityError(e.to_string()))?;

        Ok(Pcb {
            pid: pcb.pid.clone(),
            parent_pid: pcb.parent_pid.clone().unwrap_or_default(),
            state: match pcb.state {
                crate::pcb::ProcessState::New => ProtoProcessState::StatePending.into(),
                crate::pcb::ProcessState::Ready => ProtoProcessState::StatePending.into(),
                crate::pcb::ProcessState::Running => ProtoProcessState::StateRunning.into(),
                crate::pcb::ProcessState::WaitingSyscall => ProtoProcessState::StateBlocked.into(),
                crate::pcb::ProcessState::Completed => ProtoProcessState::StateCompleted.into(),
                crate::pcb::ProcessState::Failed => ProtoProcessState::StateTerminated.into(),
            },
            quantum_used: pcb.execution_metrics.cycles_executed,
            memory: Some(ank_proto::v1::pcb::MemorySpace {
                instruction_pointer: pcb.program_counter.current_node.clone(),
                context_refs: pcb.memory_pointers.l2_context_refs.clone(),
                registers: pcb.registers.temp_vars.clone(),
            }),
            inlined_context: pcb.inlined_context.clone(),
            created_at: Some(prost_types::Timestamp {
                seconds: pcb.created_at.timestamp(),
                nanos: pcb.created_at.timestamp_subsec_nanos() as i32,
            }),
            last_updated: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: Utc::now().timestamp_subsec_nanos() as i32,
            }),
            priority: pcb.priority,
            process_name: pcb.process_name.clone(),
            tenant_id: tenant_id.clone(),
            teleport_token: Some(otp),
        })
    }
}

use chrono::Utc;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_mtls_strict_mode_blocks_plaintext() {
        // Habilitar strict mode
        env::set_var("AEGIS_MTLS_STRICT", "true");
        // Forzar certificados faltantes
        env::set_var("AEGIS_TLS_CERT_PATH", "/tmp/non_existent.crt");
        env::set_var("AEGIS_ROOT_KEY", "dummy_key");

        let client = SwarmClient;
        // Mock PCB
        let pcb = crate::pcb::PCB::new("Test".into(), 1, "Mock".into());
        let (tx, _rx) = mpsc::channel(1);

        // Intentar conectar a otro nodo
        let result = client.teleport("192.168.1.10", 50051, pcb, tx).await;

        // Debe fallar por SecurityError, no por ConnectionRefused
        assert!(
            matches!(result, Err(SwarmError::SecurityError(msg)) if msg.contains("mTLS Strict Mode is enabled"))
        );

        env::remove_var("AEGIS_MTLS_STRICT");
    }
}
