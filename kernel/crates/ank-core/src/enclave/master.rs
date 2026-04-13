use anyhow::Result;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Master Admin Enclave para gestionar superadministradores y mapeos de Tenant_ID a Puertos.
/// Se persiste de manera segura con SQLCipher.
#[derive(Clone)]
pub struct MasterEnclave {
    // Usamos Arc<Mutex<Connection>> para permitir que múltiples hilos o tareas de Tokio
    // compartan de forma segura la misma conexión bloqueante subyacente de libsqlite3.
    // El Mutex se bloquea por períodos muy cortos sólo durante la ejecución de las sentencias,
    // garantizando acceso exclusivo por tarea y previniendo Race Conditions y Deadlocks.
    connection: Arc<Mutex<Connection>>,
}

impl MasterEnclave {
    /// Inicializa o abre la base de datos maestra (admin.db) en el root
    pub async fn open(db_path: &str, master_key: &str) -> Result<Self> {
        let path = Path::new(db_path);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                use anyhow::Context;
                std::fs::create_dir_all(parent)
                    .with_context(|| "Failed to create directory for admin db".to_string())?;
            }
        }

        use anyhow::Context;
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open master database at {}", db_path))?;

        // Aplicamos la llave. El sistema Aegis pasará una llave estática configurada en variables de entorno,
        // o generada en runtime para encriptar la propia BD maestra si se desea.
        conn.pragma_update(None, "key", master_key)
            .context("Failed to apply PRAGMA key to master database")?;

        // Verificación básica de integridad y capacidad de desencriptación.
        // Si el PRAGMA key falló o la DB está corrupta, esta consulta fallará.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid master key or corrupted master database file")?;

        info!("Master Admin Enclave initialized successfully.");

        let enclave = Self {
            connection: Arc::new(Mutex::new(conn)),
        };
        enclave.init_schema().await?;

        Ok(enclave)
    }

    async fn init_schema(&self) -> Result<()> {
        let conn = self.connection.lock().await;
        use anyhow::Context;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS master_admin (
                id INTEGER PRIMARY KEY DEFAULT 1,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .context("Failed to init master_admin table")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tenants (
                tenant_id TEXT PRIMARY KEY,
                username TEXT NOT NULL DEFAULT '',
                role TEXT NOT NULL DEFAULT 'user',
                network_port INTEGER NOT NULL,
                password_must_change INTEGER NOT NULL DEFAULT 1,
                password_hash TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_active DATETIME
            )",
            [],
        )
        .context("Failed to init tenants table")?;

        // SRE Migration: in case columns are missing from an older schema.
        let _ = conn.execute(
            "ALTER TABLE tenants ADD COLUMN username TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tenants ADD COLUMN role TEXT NOT NULL DEFAULT 'user'",
            [],
        );
        let _ = conn.execute("ALTER TABLE tenants ADD COLUMN last_active DATETIME", []);
        let _ = conn.execute(
            "ALTER TABLE tenants ADD COLUMN password_hash TEXT NOT NULL DEFAULT ''",
            [],
        );

        conn.execute(
            "CREATE TABLE IF NOT EXISTS setup_tokens (
                token TEXT PRIMARY KEY,
                expires_at DATETIME NOT NULL,
                used INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )
        .context("Failed to init setup_tokens table")?;

        Ok(())
    }

    /// Exposes the internal connection lock for the Citadel identity module.
    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    /// Hashea una clave usando Argon2id
    pub fn hash_password(password: &str) -> Result<String> {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Hashing failed: {}", e))?
            .to_string();
        Ok(password_hash)
    }

    /// Verifica si ya existe un master admin configurado de forma robusta.
    /// Devuelve false si la tabla no existe o si no hay registros.
    pub async fn admin_exists(&self) -> Result<bool> {
        let conn = self.connection.lock().await;

        // Primero verificamos que la tabla exista consultando sqlite_master.
        // Si no existe (ej: DB acaba de ser creada pero init_schema no terminó), es false.
        let table_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='master_admin'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !table_exists {
            return Ok(false);
        }

        let count: i64 =
            conn.query_row("SELECT count(*) FROM master_admin", [], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Inicializa el super administrador (solo si no hay ninguno)
    pub async fn initialize_master(&self, username: &str, passphrase_sha256: &str) -> Result<()> {
        use anyhow::Context;
        if self.admin_exists().await? {
            anyhow::bail!("Master Admin is already initialized. Cannot overwrite.");
        }

        let hash = Self::hash_password(passphrase_sha256).context("Failed to hash password")?;
        let conn = self.connection.lock().await;
        conn.execute(
            "INSERT INTO master_admin (id, username, password_hash) VALUES (1, ?1, ?2)",
            [&username, &hash.as_str()],
        )
        .context("Failed to configure Master Admin")?;

        info!("Master admin {} successfully configured.", username);
        Ok(())
    }

    /// Valida que el session_key proporcione matching real con el Master Admin password.
    /// Es vital validar tanto username como password_hash para identidad robusta.
    pub async fn authenticate_master(
        &self,
        username: &str,
        passphrase_or_session: &str,
    ) -> Result<bool> {
        // SECURITY: No development bypass is provided. Use the setup token flow
        // (store_setup_token / validate_and_consume_setup_token) for first-time access.
        // See ADR-023.

        let conn = self.connection.lock().await;

        // Buscamos el hash del admin específico por su username
        let mut stmt =
            conn.prepare("SELECT password_hash FROM master_admin WHERE username = ?1 LIMIT 1")?;

        let hash_result: rusqlite::Result<String> = stmt.query_row([username], |row| row.get(0));

        match hash_result {
            Ok(real_hash) => {
                use argon2::{
                    password_hash::{PasswordHash, PasswordVerifier},
                    Argon2,
                };
                let parsed_hash = match PasswordHash::new(&real_hash) {
                    Ok(ph) => ph,
                    Err(_) => return Ok(false),
                };
                let is_valid = Argon2::default()
                    .verify_password(passphrase_or_session.as_bytes(), &parsed_hash)
                    .is_ok();
                Ok(is_valid)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false), // Admin no encontrado
            Err(e) => Err(anyhow::anyhow!("Database authentication error: {}", e)),
        }
    }

    /// Valida que el tenant proporcione un login válido no-root.
    /// Si las credenciales son correctas pero `password_must_change = 1`, retorna
    /// `Err("PASSWORD_MUST_CHANGE")` para que el BFF pueda redirigir al flujo de cambio de clave.
    pub async fn authenticate_tenant(
        &self,
        tenant_id: &str,
        passphrase_or_session: &str,
    ) -> Result<bool> {
        let conn = self.connection.lock().await;
        let mut stmt = conn.prepare(
            "SELECT password_hash, password_must_change FROM tenants WHERE tenant_id = ?1 LIMIT 1",
        )?;

        let result: rusqlite::Result<(String, i32)> =
            stmt.query_row([tenant_id], |row| Ok((row.get(0)?, row.get(1)?)));

        match result {
            Ok((real_hash, must_change)) => {
                use argon2::{
                    password_hash::{PasswordHash, PasswordVerifier},
                    Argon2,
                };
                let parsed_hash = match PasswordHash::new(&real_hash) {
                    Ok(ph) => ph,
                    Err(_) => return Ok(false),
                };
                let is_valid = Argon2::default()
                    .verify_password(passphrase_or_session.as_bytes(), &parsed_hash)
                    .is_ok();
                if is_valid && must_change == 1 {
                    anyhow::bail!("PASSWORD_MUST_CHANGE");
                }
                Ok(is_valid)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(anyhow::anyhow!(
                "Database tenant authentication error: {}",
                e
            )),
        }
    }

    /// Genera un nuevo tenant con puerto incrementado asignado, y lo registra
    pub async fn create_tenant(&self, tenant_id: &str) -> Result<(u32, String)> {
        use anyhow::Context;
        let conn = self.connection.lock().await;
        // En un escenario real, buscaríamos el último puerto usado.
        let mut stmt = conn.prepare("SELECT MAX(network_port) FROM tenants")?;
        let max_port: Option<u32> = stmt.query_row([], |row| row.get(0)).unwrap_or(Some(50051));

        // Asignamos el siguiente puerto disponible, empezando desde 50052 para los tenants.
        let next_port = if let Some(p) = max_port {
            if p >= 50052 {
                p + 1
            } else {
                50052
            }
        } else {
            50052
        };

        // Generar passphrase temporal, e.g., uuid-base o hash. Usaremos uuid simplificado
        let temp_passphrase = uuid::Uuid::new_v4().to_string().replace("-", "")[0..12].to_string();

        // El BFF enviará SHA256(temp_passphrase). Nosotros guardamos Argon2(SHA256(temp_passphrase)).
        let sha256_pass = format!("{:x}", Sha256::digest(temp_passphrase.as_bytes()));
        let hash = Self::hash_password(&sha256_pass).context("Failed to hash temp passphrase")?;

        conn.execute(
            "INSERT INTO tenants (tenant_id, username, network_port, password_must_change, password_hash) VALUES (?1, ?1, ?2, 1, ?3)",
            rusqlite::params![tenant_id, next_port, hash],
        ).with_context(|| format!("Failed to create tenant {}", tenant_id))?;

        info!(
            "Created tenant {} assigned to port {}",
            tenant_id, next_port
        );

        // Devolvemos el puerto y la contraseña temporal sin encriptar, solo para devolvérsela al cliente ahora
        Ok((next_port, temp_passphrase))
    }

    /// Resetea la contraseña del tenant: hashea y persiste la nueva clave, y limpia el flag
    /// `password_must_change` para que la próxima autenticación no requiera cambio inmediato.
    pub async fn reset_tenant_password(
        &self,
        tenant_id: &str,
        new_passphrase_sha256: &str,
    ) -> Result<()> {
        use anyhow::Context;
        let new_hash =
            Self::hash_password(new_passphrase_sha256).context("Failed to hash new passphrase")?;

        let conn = self.connection.lock().await;
        let rows = conn
            .execute(
                "UPDATE tenants SET password_hash = ?1, password_must_change = 0 WHERE tenant_id = ?2",
                rusqlite::params![new_hash, tenant_id],
            )
            .context("Failed to update tenant password")?;

        if rows == 0 {
            anyhow::bail!("Tenant {} not found.", tenant_id);
        }

        info!("Password reset completed for tenant {}", tenant_id);
        Ok(())
    }

    /// Devuelve una lista de todos los tenants registrados con su información completa.
    pub async fn list_tenants(&self) -> Result<Vec<ank_proto::v1::TenantInfo>> {
        let conn = self.connection.lock().await;
        let mut stmt = conn.prepare(
            "SELECT tenant_id, username, role, created_at, last_active, network_port 
             FROM tenants 
             ORDER BY created_at ASC",
        )?;

        let tenants = stmt
            .query_map([], |row| {
                Ok(ank_proto::v1::TenantInfo {
                    tenant_id: row.get::<_, String>(0)?,
                    username: row.get::<_, String>(1)?,
                    role: row.get::<_, String>(2)?,
                    created_at: row.get::<_, String>(3)?,
                    last_active: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    port: row.get::<_, u32>(5)?,
                })
            })?
            .filter_map(Result::ok)
            .collect();

        Ok(tenants)
    }

    /// Elimina un tenant de la base de datos maestra y su DB individual si existe.
    pub async fn delete_tenant(&self, tenant_id: &str) -> Result<()> {
        use anyhow::Context;
        let conn = self.connection.lock().await;
        let rows = conn
            .execute(
                "DELETE FROM tenants WHERE tenant_id = ?1",
                rusqlite::params![tenant_id],
            )
            .context("Failed to delete tenant")?;

        if rows == 0 {
            anyhow::bail!("Tenant {} not found.", tenant_id);
        }

        info!("Tenant {} successfully deleted from master.", tenant_id);
        Ok(())
    }

    /// ANK-29-001: Almacena un setup token con TTL
    pub async fn store_setup_token(&self, token: &str, ttl_minutes: i64) -> Result<()> {
        use anyhow::Context;
        let conn = self.connection.lock().await;
        conn.execute(
            "INSERT OR REPLACE INTO setup_tokens (token, expires_at) VALUES (?1, datetime('now', '+' || ?2 || ' minutes'))",
            rusqlite::params![token, ttl_minutes],
        )
        .context("Failed to store setup token")?;
        Ok(())
    }

    /// ANK-29-001: Valida y consume un setup token
    pub async fn validate_and_consume_setup_token(&self, token: &str) -> Result<bool> {
        let conn = self.connection.lock().await;
        let result: rusqlite::Result<i32> = conn.query_row(
            "SELECT 1 FROM setup_tokens WHERE token = ?1 AND used = 0 AND expires_at > datetime('now')",
            [token],
            |_| Ok(1),
        );

        match result {
            Ok(_) => {
                conn.execute("UPDATE setup_tokens SET used = 1 WHERE token = ?1", [token])?;
                Ok(true)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Token validation error: {}", e)),
        }
    }

    /// ANK-29-001: Recupera el token actual si existe y no ha expirado
    pub async fn get_setup_token_for_regeneration(&self) -> Result<String> {
        let conn = self.connection.lock().await;
        let token: String = conn.query_row(
            "SELECT token FROM setup_tokens WHERE used = 0 AND expires_at > datetime('now') LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        Ok(token)
    }
}

impl MasterEnclave {
    /// Crea un `MasterEnclave` en memoria para uso exclusivo en tests de integración.
    /// Usa SQLite `:memory:` para garantizar cero side-effects y aislamiento total.
    /// SRE Law: Nunca llamar este método en producción.
    #[doc(hidden)]
    pub async fn new_in_memory() -> Result<Self> {
        use anyhow::Context;
        let conn =
            Connection::open(":memory:").context("Failed to open in-memory SQLite for test")?;
        let enclave = Self {
            connection: Arc::new(Mutex::new(conn)),
        };
        enclave.init_schema().await?;
        Ok(enclave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_master_admin_flow() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let db_path = dir.path().join("admin.db");
        let path_str = db_path.to_str().context("Path is not valid UTF-8")?;

        let enclave = MasterEnclave::open(path_str, "secret_key").await?;

        assert!(!enclave.admin_exists().await?);

        let haxor_sha256 = format!("{:x}", Sha256::digest("haxor".as_bytes()));
        enclave.initialize_master("root", &haxor_sha256).await?;
        let is_auth = enclave.admin_exists().await?;
        assert!(is_auth);

        // Simulate BFF hashing for admin login
        let haxor_sha256 = format!("{:x}", Sha256::digest("haxor".as_bytes()));
        let is_auth = enclave.authenticate_master("root", &haxor_sha256).await?;
        assert!(is_auth);

        let (port, pass) = enclave.create_tenant("testuser").await?;
        assert!(port >= 50052);
        assert!(!pass.is_empty());

        // The tenant is created with password_must_change = 1, so authentication
        // with the temp password must fail with PASSWORD_MUST_CHANGE.
        // We must SHA256 the temp pass because that's what the BFF does.
        let sha256_pass = format!("{:x}", Sha256::digest(pass.as_bytes()));
        let auth_result = enclave.authenticate_tenant("testuser", &sha256_pass).await;
        let Err(e) = auth_result else {
            anyhow::bail!("Expected PASSWORD_MUST_CHANGE error but authentication succeeded");
        };
        assert!(
            e.to_string().contains("PASSWORD_MUST_CHANGE"),
            "Expected PASSWORD_MUST_CHANGE, got: {}",
            e
        );

        // After a password reset the flag must be cleared and the new credential must work.
        let new_pass_raw = "new_secure_pass";
        let sha256_new = format!("{:x}", Sha256::digest(new_pass_raw.as_bytes()));
        enclave
            .reset_tenant_password("testuser", &sha256_new)
            .await?;
        let is_auth_after_reset = enclave.authenticate_tenant("testuser", &sha256_new).await?;
        assert!(is_auth_after_reset);

        Ok(())
    }

    /// Verifies that `reset_tenant_password` persists a new hash so the old
    /// credential is rejected and the new one is accepted.
    #[tokio::test]
    async fn test_reset_password_actually_changes_hash() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;

        // Create a tenant (temp password assigned, password_must_change = 1).
        let (_port, temp_pass) = enclave.create_tenant("alice").await?;

        // Reset to a known new password (pre-hash with SHA256 like the BFF does).
        let new_pass_raw = "super_secret_new_pass";
        let sha256_new = format!("{:x}", Sha256::digest(new_pass_raw.as_bytes()));
        enclave.reset_tenant_password("alice", &sha256_new).await?;

        // Old temporary password (SHA256'd) must be rejected after reset.
        let sha256_old = format!("{:x}", Sha256::digest(temp_pass.as_bytes()));
        let old_auth = enclave.authenticate_tenant("alice", &sha256_old).await?;
        assert!(
            !old_auth,
            "Old temp password should be rejected after reset"
        );

        // New password must be accepted without a PASSWORD_MUST_CHANGE error
        // (reset sets password_must_change = 0).
        let new_auth = enclave.authenticate_tenant("alice", &sha256_new).await?;
        assert!(new_auth, "New password should be accepted after reset");

        Ok(())
    }
}
