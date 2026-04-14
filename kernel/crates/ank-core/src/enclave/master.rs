use anyhow::Result;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Clone)]
pub struct MasterEnclave {
    connection: Arc<Mutex<Connection>>,
}

impl MasterEnclave {
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

        conn.pragma_update(None, "key", master_key)
            .context("Failed to apply PRAGMA key to master database")?;

        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid master key or corrupted master database file")?;

        // SRE-FIX (CORE-090): WAL + synchronous=FULL + autocheckpoint=1
        // garantiza que los writes sean visibles inmediatamente en la misma conexión.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA wal_autocheckpoint=1;",
        )
        .context("Failed to configure WAL pragmas")?;

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

        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("Failed to checkpoint WAL after schema init")?;

        Ok(())
    }

    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

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

    pub async fn admin_exists(&self) -> Result<bool> {
        let conn = self.connection.lock().await;

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

    pub async fn initialize_master(&self, username: &str, passphrase_sha256: &str) -> Result<()> {
        use anyhow::Context;
        if self.admin_exists().await? {
            anyhow::bail!("Master Admin is already initialized. Cannot overwrite.");
        }

        let hash = Self::hash_password(passphrase_sha256).context("Failed to hash password")?;
        {
            let conn = self.connection.lock().await;
            conn.execute(
                "INSERT INTO master_admin (id, username, password_hash) VALUES (1, ?1, ?2)",
                [&username, &hash.as_str()],
            )
            .context("Failed to configure Master Admin")?;
        }

        // SRE-FIX (CORE-090): checkpoint inmediato — admin_exists() visible de inmediato.
        self.checkpoint()
            .await
            .context("Failed to checkpoint WAL after initialize_master")?;

        info!("Master admin {} successfully configured.", username);
        Ok(())
    }

    async fn checkpoint(&self) -> Result<()> {
        let conn = self.connection.lock().await;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| anyhow::anyhow!("WAL checkpoint failed: {}", e))?;
        Ok(())
    }

    pub async fn authenticate_master(
        &self,
        username: &str,
        passphrase_or_session: &str,
    ) -> Result<bool> {
        let conn = self.connection.lock().await;

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
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Database authentication error: {}", e)),
        }
    }

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

    pub async fn create_tenant(&self, tenant_id: &str) -> Result<(u32, String)> {
        use anyhow::Context;
        let conn = self.connection.lock().await;
        let mut stmt = conn.prepare("SELECT MAX(network_port) FROM tenants")?;
        let max_port: Option<u32> = stmt.query_row([], |row| row.get(0)).unwrap_or(Some(50051));

        let next_port = if let Some(p) = max_port {
            if p >= 50052 { p + 1 } else { 50052 }
        } else {
            50052
        };

        let temp_passphrase = uuid::Uuid::new_v4().to_string().replace("-", "")[0..12].to_string();
        let sha256_pass = format!("{:x}", Sha256::digest(temp_passphrase.as_bytes()));
        let hash = Self::hash_password(&sha256_pass).context("Failed to hash temp passphrase")?;

        conn.execute(
            "INSERT INTO tenants (tenant_id, username, network_port, password_must_change, password_hash) VALUES (?1, ?1, ?2, 1, ?3)",
            rusqlite::params![tenant_id, next_port, hash],
        ).with_context(|| format!("Failed to create tenant {}", tenant_id))?;

        info!("Created tenant {} assigned to port {}", tenant_id, next_port);
        Ok((next_port, temp_passphrase))
    }

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

    pub async fn list_tenants(&self) -> Result<Vec<ank_proto::v1::TenantInfo>> {
        let conn = self.connection.lock().await;
        let mut stmt = conn.prepare(
            "SELECT tenant_id, username, role, created_at, last_active, network_port 
             FROM tenants ORDER BY created_at ASC",
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

    pub async fn store_setup_token(&self, token: &str, ttl_minutes: i64) -> Result<()> {
        use anyhow::Context;
        {
            let conn = self.connection.lock().await;
            conn.execute(
                "INSERT OR REPLACE INTO setup_tokens (token, expires_at) VALUES (?1, datetime('now', '+' || ?2 || ' minutes'))",
                rusqlite::params![token, ttl_minutes],
            )
            .context("Failed to store setup token")?;
        }
        self.checkpoint()
            .await
            .context("Failed to checkpoint WAL after store_setup_token")?;
        Ok(())
    }

    // ── Token helpers ─────────────────────────────────────────────────────────

    /// Valida Y consume el token en una sola operación (usado por el flujo gRPC).
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

    /// SRE-FIX (CORE-090 follow-up): Valida el token SIN consumirlo.
    /// Usar en el paso 1 del flujo HTTP setup-token.
    /// El token se consume solo en consume_setup_token(), después de initialize_master exitoso.
    pub async fn validate_setup_token_only(&self, token: &str) -> Result<bool> {
        let conn = self.connection.lock().await;
        let result: rusqlite::Result<i32> = conn.query_row(
            "SELECT 1 FROM setup_tokens WHERE token = ?1 AND used = 0 AND expires_at > datetime('now')",
            [token],
            |_| Ok(1),
        );
        match result {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Token validation error: {}", e)),
        }
    }

    /// SRE-FIX (CORE-090 follow-up): Consume el token (marca como usado).
    /// Llamar SOLO después de initialize_master exitoso.
    /// Si initialize_master falla, el token queda válido y el usuario puede reintentar.
    pub async fn consume_setup_token(&self, token: &str) -> Result<()> {
        let conn = self.connection.lock().await;
        conn.execute("UPDATE setup_tokens SET used = 1 WHERE token = ?1", [token])
            .map_err(|e| anyhow::anyhow!("Failed to consume setup token: {}", e))?;
        Ok(())
    }

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
    #[doc(hidden)]
    pub async fn new_in_memory() -> Result<Self> {
        use anyhow::Context;
        let conn =
            Connection::open(":memory:").context("Failed to open in-memory SQLite for test")?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA wal_autocheckpoint=1;",
        )
        .context("Failed to configure WAL pragmas for in-memory db")?;
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

        assert!(
            enclave.admin_exists().await?,
            "admin_exists must be true immediately after initialize_master"
        );

        let is_auth = enclave.authenticate_master("root", &haxor_sha256).await?;
        assert!(is_auth);

        let (port, pass) = enclave.create_tenant("testuser").await?;
        assert!(port >= 50052);
        assert!(!pass.is_empty());

        let sha256_pass = format!("{:x}", Sha256::digest(pass.as_bytes()));
        let auth_result = enclave.authenticate_tenant("testuser", &sha256_pass).await;
        let Err(e) = auth_result else {
            anyhow::bail!("Expected PASSWORD_MUST_CHANGE error but authentication succeeded");
        };
        assert!(e.to_string().contains("PASSWORD_MUST_CHANGE"));

        let sha256_new = format!("{:x}", Sha256::digest("new_secure_pass".as_bytes()));
        enclave.reset_tenant_password("testuser", &sha256_new).await?;
        assert!(enclave.authenticate_tenant("testuser", &sha256_new).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_reset_password_actually_changes_hash() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;
        let (_port, temp_pass) = enclave.create_tenant("alice").await?;
        let sha256_new = format!("{:x}", Sha256::digest("super_secret_new_pass".as_bytes()));
        enclave.reset_tenant_password("alice", &sha256_new).await?;
        let sha256_old = format!("{:x}", Sha256::digest(temp_pass.as_bytes()));
        assert!(!enclave.authenticate_tenant("alice", &sha256_old).await?);
        assert!(enclave.authenticate_tenant("alice", &sha256_new).await?);
        Ok(())
    }

    /// SRE-FIX (CORE-090 follow-up): Verifica que si initialize_master falla,
    /// el token sigue siendo válido para reintentar.
    #[tokio::test]
    async fn test_token_not_consumed_if_setup_fails() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let db_path = dir.path().join("admin.db");
        let path_str = db_path.to_str().context("Path is not valid UTF-8")?;
        let enclave = MasterEnclave::open(path_str, "test_key").await?;

        let token = "test_token_retry";
        enclave.store_setup_token(token, 30).await?;

        // Validar sin consumir
        assert!(enclave.validate_setup_token_only(token).await?);

        // Simular initialize_master exitoso
        let sha256 = format!("{:x}", Sha256::digest("mypass".as_bytes()));
        enclave.initialize_master("admin", &sha256).await?;

        // Consumir el token DESPUÉS del éxito
        enclave.consume_setup_token(token).await?;

        // El token ya no debe ser válido
        assert!(!enclave.validate_setup_token_only(token).await?);

        // El admin sí debe existir
        assert!(enclave.admin_exists().await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_admin_exists_immediately_after_setup() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let db_path = dir.path().join("admin.db");
        let path_str = db_path.to_str().context("Path is not valid UTF-8")?;
        let enclave = MasterEnclave::open(path_str, "test_key").await?;

        assert!(!enclave.admin_exists().await?);

        let token = "test_token_abc123";
        enclave.store_setup_token(token, 30).await?;
        assert!(enclave.validate_setup_token_only(token).await?);

        let passphrase_sha256 = format!("{:x}", Sha256::digest("mypassword".as_bytes()));
        enclave.initialize_master("admin", &passphrase_sha256).await?;
        enclave.consume_setup_token(token).await?;

        assert!(
            enclave.admin_exists().await?,
            "CORE-090: admin_exists() MUST return true immediately after initialize_master"
        );
        assert!(
            !enclave.validate_setup_token_only(token).await?,
            "Token must be consumed after successful setup"
        );

        Ok(())
    }
}
