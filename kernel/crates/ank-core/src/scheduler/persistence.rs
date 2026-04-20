use crate::pcb::PCB;
use anyhow::Result;
use async_trait::async_trait;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::task;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    pub tenant_id: String,
    pub engine_id: String,
    pub voice_id: String,
    pub model_pref: String, // "LocalOnly", "CloudOnly", "HybridSmart"
    pub settings_json: String,
}

#[async_trait]
pub trait StatePersistor: Send + Sync {
    async fn save_pcb(&self, pcb: &PCB) -> Result<()>;
    async fn delete_pcb(&self, pid: &str) -> Result<()>;
    async fn load_all_pcbs(&self) -> Result<Vec<PCB>>;
    async fn flush(&self) -> Result<()>;
    async fn get_voice_profile(&self, tenant_id: &str) -> Result<Option<VoiceProfile>>;
    async fn update_voice_profile(&self, profile: VoiceProfile) -> Result<()>;
}

pub struct SQLCipherPersistor {
    conn: Arc<Mutex<Connection>>,
}

impl SQLCipherPersistor {
    pub fn new(db_path: &str, key: &str) -> Result<Self> {
        use anyhow::Context;
        info!(path = %db_path, "Initializing PersistenceManager (SQLCipher).");
        let conn = Connection::open(db_path).context("Failed to open SQLCipher database")?;

        // Aplicamos la llave de cifrado
        conn.pragma_update(None, "key", key)
            .context("Failed to set SQLCipher key")?;

        // Verificamos integridad (esto fallará si la llave es incorrecta)
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("SQLCipher authentication failed or database corrupted")?;

        // Esquema atómico para PCBs
        conn.execute(
            "CREATE TABLE IF NOT EXISTS process_control_blocks (
                pid TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                data TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .context("Failed to initialize PCB table")?;

        // Esquema para perfiles de voz y preferencias de ANK (Aislamiento Citadel)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tenant_voice_profiles (
                tenant_id TEXT PRIMARY KEY,
                engine_id TEXT NOT NULL,
                voice_id TEXT NOT NULL,
                model_pref TEXT NOT NULL DEFAULT 'HybridSmart',
                settings_json TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .context("Failed to initialize tenant_voice_profiles table")?;

        // Migración CORE-112: Añadir model_pref si no existe (para instalaciones previas)
        let _ = conn.execute("ALTER TABLE tenant_voice_profiles ADD COLUMN model_pref TEXT NOT NULL DEFAULT 'HybridSmart'", []);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl StatePersistor for SQLCipherPersistor {
    async fn save_pcb(&self, pcb: &PCB) -> Result<()> {
        use anyhow::Context;
        let pcb_clone = pcb.clone();
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let json_data = serde_json::to_string(&pcb_clone).context("Failed to serialize PCB")?;
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;

            lock.execute(
                "INSERT OR REPLACE INTO process_control_blocks (pid, state, data, updated_at) 
                 VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
                (&pcb_clone.pid, format!("{:?}", pcb_clone.state), json_data),
            )
            .context("Failed to execute INSERT/REPLACE on PCB table")?;

            debug!(pid = %pcb_clone.pid, "PCB persisted successfully.");
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn delete_pcb(&self, pid: &str) -> Result<()> {
        use anyhow::Context;
        let pid_str = pid.to_string();
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            use anyhow::Context;
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            lock.execute(
                "DELETE FROM process_control_blocks WHERE pid = ?1",
                [&pid_str],
            )
            .context("Failed to delete PCB from disk")?;
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn load_all_pcbs(&self) -> Result<Vec<PCB>> {
        use anyhow::Context;
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            let mut stmt = lock.prepare("SELECT data FROM process_control_blocks")?;
            let pcb_iter = stmt.query_map([], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })?;

            let mut results = Vec::new();
            for pcb_json_res in pcb_iter {
                let json_str = pcb_json_res?;
                let pcb: PCB = serde_json::from_str(&json_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                results.push(pcb);
            }
            Ok(results)
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn flush(&self) -> Result<()> {
        use anyhow::Context;
        let conn = self.conn.clone();
        task::spawn_blocking(move || {
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            lock.pragma_update(None, "wal_checkpoint", "TRUNCATE")
                .context("Failed to flush WAL to disk (checkpoint TRUNCATE)")?;
            info!("Persistence flush completed.");
            Ok(())
        })
        .await
        .context("Spawn blocking failed")?
    }

    async fn get_voice_profile(&self, tenant_id: &str) -> Result<Option<VoiceProfile>> {
        use anyhow::Context;
        let tenant_id = tenant_id.to_string();
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;
            let mut stmt = lock.prepare(
                "SELECT engine_id, voice_id, model_pref, settings_json FROM tenant_voice_profiles WHERE tenant_id = ?1"
            )?;
            let mut rows = stmt.query([&tenant_id])?;

            if let Some(row) = rows.next()? {
                Ok(Some(VoiceProfile {
                    tenant_id,
                    engine_id: row.get(0)?,
                    voice_id: row.get(1)?,
                    model_pref: row.get(2)?,
                    settings_json: row.get(3)?,
                }))
            } else {
                Ok(None)
            }
        })
        .await
        .context("Spawn blocking failed during get_voice_profile")?
    }

    async fn update_voice_profile(&self, profile: VoiceProfile) -> Result<()> {
        use anyhow::Context;
        let conn = self.conn.clone();

        task::spawn_blocking(move || {
            let lock = conn
                .lock()
                .map_err(|_| anyhow::anyhow!("Mutex poison error"))?;

            // Validación: Asegurar que el tenant_id no esté vacío
            if profile.tenant_id.is_empty() {
                return Err(anyhow::anyhow!("Aislamiento Citadel: tenant_id no puede estar vacío"));
            }

            lock.execute(
                "INSERT OR REPLACE INTO tenant_voice_profiles (tenant_id, engine_id, voice_id, model_pref, settings_json, updated_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
                (&profile.tenant_id, &profile.engine_id, &profile.voice_id, &profile.model_pref, &profile.settings_json),
            )
            .context("Failed to execute INSERT/REPLACE on tenant_voice_profiles table")?;

            debug!(tenant_id = %profile.tenant_id, "Voice profile updated successfully.");
            Ok(())
        })
        .await
        .context("Spawn blocking failed during update_voice_profile")?
    }
}

#[cfg(test)]
pub struct MockPersistor;

#[cfg(test)]
#[async_trait]
impl StatePersistor for MockPersistor {
    async fn save_pcb(&self, _pcb: &PCB) -> Result<()> {
        Ok(())
    }
    async fn delete_pcb(&self, _pid: &str) -> Result<()> {
        Ok(())
    }
    async fn load_all_pcbs(&self) -> Result<Vec<PCB>> {
        Ok(Vec::new())
    }
    async fn flush(&self) -> Result<()> {
        Ok(())
    }
    async fn get_voice_profile(&self, _tenant_id: &str) -> Result<Option<VoiceProfile>> {
        Ok(None)
    }
    async fn update_voice_profile(&self, _profile: VoiceProfile) -> Result<()> {
        Ok(())
    }
}
