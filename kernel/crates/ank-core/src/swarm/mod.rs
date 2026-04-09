use chrono::{DateTime, Utc};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};
/// --- MODULOS ---
pub mod client;
pub mod otp;

/// --- CONSTANTES ---
pub const SWARM_SERVICE_TYPE: &str = "_aegis-ank._tcp.local.";
pub const HEARTBEAT_TOLERANCE_SECONDS: u64 = 15;

/// --- NODE METADATA ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Ready,   // Disponible para tareas
    Busy,    // Cargado
    Suspect, // Desaparecido recientemente (ventana de gracia)
    Offline, // Desconectado oficialmente
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeMetadata {
    pub node_id: String,
    pub instance_name: String,
    pub ip_address: String,
    pub grpc_port: u16,
    pub hardware_tier: u8,
    pub cpu_cores: u32,
    pub vram_gb: u32,
    pub status: NodeStatus,
    pub last_seen: DateTime<Utc>,
}

/// --- SWARM MANAGER ---
/// Gestor de la mente colmena distribuida.
/// Implementa descubrimiento zeroconf (mDNS) para orquestación en LAN.
pub struct SwarmManager {
    /// Tabla de ruteo de nodos. Arc<RwLock> asegura acceso recurrente y seguro.
    pub active_nodes: Arc<RwLock<HashMap<String, NodeMetadata>>>,
    pub local_node_id: String,
    pub local_grpc_port: u16,
    // Métricas de hardware para publicidad
    pub tier: u8,
    pub cpu_cores: u32,
    pub vram_gb: u32,
    daemon: ServiceDaemon,
}

impl SwarmManager {
    pub fn new(
        local_node_id: String,
        local_grpc_port: u16,
        tier: u8,
        cpu_cores: u32,
        vram_gb: u32,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            active_nodes: Arc::new(RwLock::new(HashMap::new())),
            local_node_id,
            local_grpc_port,
            tier,
            cpu_cores,
            vram_gb,
            daemon: ServiceDaemon::new()?,
        })
    }

    /// Inicia el anuncio mDNS de este Kernel en la red local.
    pub async fn start_broadcasting(&self) -> anyhow::Result<()> {
        let instance_name = format!("ank-node-{}", &self.local_node_id[..4]);
        let host_name = format!("{}.local.", instance_name);

        // Prep de metadatos para el TXT Record
        let mut properties = HashMap::new();
        properties.insert("node_id".to_string(), self.local_node_id.clone());
        properties.insert("tier".to_string(), self.tier.to_string());
        properties.insert("cpu_cores".to_string(), self.cpu_cores.to_string());
        properties.insert("vram_gb".to_string(), self.vram_gb.to_string());
        properties.insert("status".to_string(), "ready".to_string());

        let service_info = ServiceInfo::new(
            SWARM_SERVICE_TYPE,
            &instance_name,
            &host_name,
            "", // Detección automática de IP
            self.local_grpc_port,
            Some(properties),
        )?
        .enable_addr_auto();

        self.daemon.register(service_info)?;
        info!(
            "Broadcasting ANK node: {} (Tier {}) on port {}",
            instance_name, self.tier, self.local_grpc_port
        );

        Ok(())
    }

    /// Inicia el bucle de escucha para descubrir otros Kernels.
    pub async fn start_listening(&self) -> anyhow::Result<()> {
        let receiver = self.daemon.browse(SWARM_SERVICE_TYPE)?;
        let active_nodes = self.active_nodes.clone();
        let my_id = self.local_node_id.clone();

        tokio::spawn(async move {
            info!("Swarm Discovery Loop started. Listening for pulses...");

            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let remote_id = info.get_property_val_str("node_id").unwrap_or_default();

                        // Seguridad: Evitar auto-descubrimiento en bucle
                        if remote_id == my_id || remote_id.is_empty() {
                            continue;
                        }

                        let ip = info
                            .get_addresses()
                            .iter()
                            .next()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        let metadata = NodeMetadata {
                            node_id: remote_id.to_string(),
                            instance_name: info.get_fullname().to_string(),
                            ip_address: ip,
                            grpc_port: info.get_port(),
                            hardware_tier: info
                                .get_property_val_str("tier")
                                .and_then(|t| t.parse().ok())
                                .unwrap_or(1),
                            cpu_cores: info
                                .get_property_val_str("cpu_cores")
                                .and_then(|c| c.parse().ok())
                                .unwrap_or(1),
                            vram_gb: info
                                .get_property_val_str("vram_gb")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0),
                            status: NodeStatus::Ready,
                            last_seen: Utc::now(),
                        };

                        let mut nodes = active_nodes.write().await;
                        info!(id = %remote_id, "Sister node synced: {}", metadata.instance_name);
                        nodes.insert(remote_id.to_string(), metadata);
                    }
                    ServiceEvent::ServiceRemoved(_type, fullname) => {
                        // Gestión de desconexión con ventana de gracia
                        let nodes_ref = active_nodes.clone();
                        let name_ref = fullname.clone();

                        tokio::spawn(async move {
                            let mut target_id = None;
                            {
                                let mut nodes = nodes_ref.write().await;
                                if let Some((id, meta)) =
                                    nodes.iter_mut().find(|(_, m)| m.instance_name == name_ref)
                                {
                                    meta.status = NodeStatus::Suspect;
                                    target_id = Some(id.clone());
                                    warn!(id = %id, "Node pulse lost. Entering grace period.");
                                }
                            }

                            if let Some(id) = target_id {
                                tokio::time::sleep(Duration::from_secs(
                                    HEARTBEAT_TOLERANCE_SECONDS,
                                ))
                                .await;

                                let mut nodes = nodes_ref.write().await;
                                if let Some(meta) = nodes.get_mut(&id) {
                                    if meta.status == NodeStatus::Suspect {
                                        info!(id = %id, "Grace period expired. Node marked as Offline.");
                                        meta.status = NodeStatus::Offline;
                                        // En lugar de borrarlo, lo mantenemos como Offline para auditoría de ruteo
                                    }
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
            warn!("Swarm Discovery Loop terminated unexpectedly.");
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_registry_flow() -> anyhow::Result<()> {
        let manager = SwarmManager::new("node-alpha".into(), 50051, 3, 32, 48)?;
        assert_eq!(manager.local_node_id, "node-alpha");

        // Simulación de ruteo
        {
            let mut nodes = manager.active_nodes.write().await;
            nodes.insert(
                "node-beta".into(),
                NodeMetadata {
                    node_id: "node-beta".into(),
                    instance_name: "ank-node-beta.local.".into(),
                    ip_address: "192.168.1.10".into(),
                    grpc_port: 50051,
                    hardware_tier: 2,
                    cpu_cores: 8,
                    vram_gb: 12,
                    status: NodeStatus::Ready,
                    last_seen: Utc::now(),
                },
            );
        }

        let nodes = manager.active_nodes.read().await;
        assert!(nodes.contains_key("node-beta"));
        Ok(())
    }
}
