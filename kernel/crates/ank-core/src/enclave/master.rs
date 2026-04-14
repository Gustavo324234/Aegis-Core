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

        conn.pragma_update(None, "key", master_key)
            .context("Failed to apply PRAGMA key to master database")?;

        // Verificación básica de integridad y capacidad de desencriptación.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid master key or corrupted master database file")?;

        // SRE-FIX (CORE-090): Usar WAL mode para escrituras concurrentes, pero forzar
        // synchronous=FULL para garantizar que los writes sean visibles inmediatamente
        // a la misma conexión sin necesidad de checkpoint manual.
        // journal_mode=WAL + synchronous=FULL es el modo más seguro para un proceso
        // único con Arc<Mutex<Connection>>.
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

        // SRE-FIX (CORE-090): Forzar checkpoint tras init_schema para que el WAL
        // quede vacío y todas las lecturas posteriores vean el estado actual.
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("Failed to checkpoint WAL after schema init")?;

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
        {
            let conn = self.connection.lock().await;
            conn.execute(
                "INSERT INTO master_admin (id, username, password_hash) VALUES (1, ?1, ?2)",
                [&username, &hash.as_str()],
            )
            .context("Failed to configure Master Admin")?;
        }

        // SRE-FIX (CORE-090): Checkpoint inmediato tras insertar el admin.
        // Garantiza que admin_exists() devuelva true en la misma sesión de proceso,
        // sin necesidad de reiniciar el servicio.
        self.checkpoint().await.context("Failed to checkpoint WAL after initialize_master")?;

        info!("Master admin {} successfully configured.", username);
        Ok(())
    }

    /// SRE-FIX (CORE-090): Fuerza un WAL checkpoint TRUNCATE.
    /// Llamar después de cualquier write crítico (initialize_master, store_setup_token).
    /// Con wal_autocheckpoint=1 esto es redundante pero actúa como garantía explícita.
    async fn checkpoint(&self) -> Result<()> {
        let conn = self.connection.lock().await;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| anyhow::anyhow!("WAL checkpoint failed: {}", e))?;
        Ok(())
    }

    /// Valida que el session_key proporcione matching real con el Master Admin password.
    pub async fn authenticate_master(
        &self,
        username: &str,
        passphrase_or_session: &str,
    ) -> Result<bool> {
        // SECURITY: No development bypass is provided. Use the setup token flow
        // (store_setup_token / validate_and_consume_setup_token) for first-time access.
        // See ADR-023.

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

    /// Valida que el tenant proporcione un login válido no-root.
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

    /// Resetea la contraseña del tenant
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

    /// Elimina un tenant de la base de datos maestra
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
        {
            let conn = self.connection.lock().await;
            conn.execute(
                "INSERT OR REPLACE INTO setup_tokens (token, expires_at) VALUES (?1, datetime('now', '+' || ?2 || ' minutes'))",
                rusqlite::params![token, ttl_minutes],
            )
            .context("Failed to store setup token")?;
        }
        // SRE-FIX (CORE-090): Checkpoint tras guardar el token para que sea
        // visible inmediatamente si el proceso lee la BD desde otra task de Tokio.
        self.checkpoint().await.context("Failed to checkpoint WAL after store_setup_token")?;
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
    #[doc(hidden)]
    pub async fn new_in_memory() -> Result<Self> {
        use anyhow::Context;
        let conn =
            Connection::open(":memory:").context("Failed to open in-memory SQLite for test")?;
        // En memoria no necesita WAL (no hay archivo físico), pero aplicamos los mismos pragmas
        // para consistencia de comportamiento en tests.
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

        // SRE-FIX (CORE-090): admin_exists() debe devolver true INMEDIATAMENTE
        // después de initialize_master, sin reiniciar el proceso.
        assert!(enclave.admin_exists().await?, "admin_exists must be true immediately after initialize_master — no restart required");

        let haxor_sha256 = format!("{:x}", Sha256::digest("haxor".as_bytes()));
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
        assert!(
            e.to_string().contains("PASSWORD_MUST_CHANGE"),
            "Expected PASSWORD_MUST_CHANGE, got: {}",
            e
        );

        let new_pass_raw = "new_secure_pass";
        let sha256_new = format!("{:x}", Sha256::digest(new_pass_raw.as_bytes()));
        enclave.reset_tenant_password("testuser", &sha256_new).await?;
        let is_auth_after_reset = enclave.authenticate_tenant("testuser", &sha256_new).await?;
        assert!(is_auth_after_reset);

        Ok(())
    }

    #[tokio::test]
    async fn test_reset_password_actually_changes_hash() -> anyhow::Result<()> {
        let enclave = MasterEnclave::new_in_memory().await?;

        let (_port, temp_pass) = enclave.create_tenant("alice").await?;

        let new_pass_raw = "super_secret_new_pass";
        let sha256_new = format!("{:x}", Sha256::digest(new_pass_raw.as_bytes()));
        enclave.reset_tenant_password("alice", &sha256_new).await?;

        let sha256_old = format!("{:x}", Sha256::digest(temp_pass.as_bytes()));
        let old_auth = enclave.authenticate_tenant("alice", &sha256_old).await?;
        assert!(!old_auth, "Old temp password should be rejected after reset");

        let new_auth = enclave.authenticate_tenant("alice", &sha256_new).await?;
        assert!(new_auth, "New password should be accepted after reset");

        Ok(())
    }

    /// SRE-FIX (CORE-090): Verifica que admin_exists() devuelve true inmediatamente
    /// después de initialize_master sin ningún restart ni reconexión intermedia.
    /// Este test reproduce el escenario exacto del bug de producción.
    #[tokio::test]
    async fn test_admin_exists_immediately_after_setup() -> anyhow::Result<()> {
        let dir = tempdir().context("Failed to create tempdir")?;
        let db_path = dir.path().join("admin.db");
        let path_str = db_path.to_str().context("Path is not valid UTF-8")?;

        let enclave = MasterEnclave::open(path_str, "test_key").await?;

        // Simular el flujo de setup token → initialize_master → admin_exists
        // exactamente como ocurre en producción (misma conexión, sin restart)
        assert!(!enclave.admin_exists().await?, "Should start uninitialized");

        let token = "test_token_abc123";
        enclave.store_setup_token(token, 30).await?;

        let valid = enclave.validate_and_consume_setup_token(token).await?;
        assert!(valid, "Token should be valid");

        let passphrase_sha256 = format!("{:x}", Sha256::digest("mypassword".as_bytes()));
        enclave.initialize_master("admin", &passphrase_sha256).await?;

        // Este es el assert que fallaba en producción: admin_exists() devolvía false
        // porque el WAL no se había checkpointeado
        assert!(
            enclave.admin_exists().await?,
            "CORE-090: admin_exists() MUST return true immediately after initialize_master"
        );

        // Verificar también que STATE_OPERATIONAL sería devuelto por el servidor
        // (simulando get_public_system_state)
        let exists = enclave.admin_exists().await?;
        let state = if exists { "STATE_OPERATIONAL" } else { "STATE_INITIALIZING" };
        assert_eq!(state, "STATE_OPERATIONAL", "System state must be OPERATIONAL after setup");

        Ok(())
    }
}
