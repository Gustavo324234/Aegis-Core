use crate::enclave::master::MasterEnclave;
use anyhow::Result;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Wrapper para el MasterEnclave para seguir el protocolo Citadel.
/// SRE Rule: Citadel actúa como la interfaz de identidad unificada para el Kernel.
#[derive(Clone)]
pub struct Citadel {
    pub enclave: MasterEnclave,
}

impl Citadel {
    pub async fn open(db_path: &str, master_key: &str) -> Result<Self> {
        let enclave = MasterEnclave::open(db_path, master_key).await?;
        Ok(Self { enclave })
    }

    /// Helper para obtener acceso directo a la conexión.
    pub fn db_connection(&self) -> Result<Arc<Mutex<Connection>>> {
        Ok(self.enclave.get_connection())
    }
}

/// Create a new tenant (CITADEL wrapper around MasterEnclave)
pub async fn create_tenant(
    username: &str,
    password: &str,
    role: &str,
    citadel: &Arc<Mutex<Citadel>>,
) -> Result<(String, String, u32)> {
    let citadel_lock = citadel.lock().await;

    // Hash password (Argon2id)
    let password_hash = MasterEnclave::hash_password(password)?;

    // Create tenant in enclave
    let (port, _temp_pass) = citadel_lock.enclave.create_tenant(username).await?;

    // Actualizar registro con role y password_hash final (ANK-ONB-001)
    let conn_lock = citadel_lock.db_connection()?;
    let conn = conn_lock.lock().await;

    conn.execute(
        "UPDATE tenants SET password_hash = ?1, role = ?2, password_must_change = 0 WHERE tenant_id = ?3",
        rusqlite::params![password_hash, role, username],
    )?;

    // Generate session_key placeholder for now
    let session_key = format!("sk_{}", uuid::Uuid::new_v4());

    Ok((username.to_string(), session_key, port))
}

/// Delete a tenant by ID
pub async fn delete_tenant_by_id(tenant_id: &str, citadel: &Arc<Mutex<Citadel>>) -> Result<()> {
    let citadel_lock = citadel.lock().await;
    citadel_lock.enclave.delete_tenant(tenant_id).await?;
    Ok(())
}

/// Reset a tenant password
pub async fn reset_tenant_password(
    tenant_id: &str,
    new_password: &str,
    citadel: &Arc<Mutex<Citadel>>,
) -> Result<()> {
    let citadel_lock = citadel.lock().await;
    citadel_lock
        .enclave
        .reset_tenant_password(tenant_id, new_password)
        .await?;
    Ok(())
}
