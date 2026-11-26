use anyhow::Result;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::info;

pub mod master;
pub use master::MasterEnclave;

const DEFAULT_TENANT_DB_POOL_SIZE: usize = 64;
const TENANT_DB_IDLE_TTL: Duration = Duration::from_secs(15 * 60);

type PoolCacheKey = (String, String);

struct PooledEntry {
    key: PoolCacheKey,
    conn: Connection,
    last_used: Instant,
}

#[derive(Default)]
struct TenantDbPool {
    idle: VecDeque<PooledEntry>,
    initialized_schemas: HashSet<String>,
    generations: HashMap<String, u64>,
}

impl TenantDbPool {
    fn normalize_path(path: &str) -> String {
        path.replace('\\', "/")
    }

    fn generation_of(&self, db_path: &str) -> u64 {
        self.generations.get(db_path).copied().unwrap_or(0)
    }

    fn purge_expired(&mut self, now: Instant) {
        self.idle
            .retain(|entry| now.saturating_duration_since(entry.last_used) <= TENANT_DB_IDLE_TTL);
    }

    fn take(&mut self, key: &PoolCacheKey, now: Instant) -> Option<(Connection, u64)> {
        self.purge_expired(now);
        let idx = self.idle.iter().rposition(|entry| &entry.key == key)?;
        let entry = self.idle.remove(idx)?;
        let generation = self.generation_of(&key.0);
        Some((entry.conn, generation))
    }

    fn put(&mut self, key: PoolCacheKey, conn: Connection, expected_gen: u64, capacity: usize) {
        if capacity == 0 || self.generation_of(&key.0) != expected_gen {
            return;
        }
        if !Path::new(&key.0).exists() {
            return;
        }
        while self.idle.len() >= capacity {
            self.idle.pop_front();
        }
        self.idle.push_back(PooledEntry {
            key,
            conn,
            last_used: Instant::now(),
        });
    }

    fn evict_path(&mut self, db_path: &str) {
        let next_gen = self.generation_of(db_path).saturating_add(1);
        self.generations.insert(db_path.to_string(), next_gen);
        let norm_target = Self::normalize_path(db_path);
        self.idle
            .retain(|entry| Self::normalize_path(&entry.key.0) != norm_target);
        self.initialized_schemas.remove(db_path);
    }

    fn evict_tenant(&mut self, tenant_id: &str, current_db_path: &str) {
        self.evict_path(current_db_path);
        let suffix = format!("/users/{}/memory.db", tenant_id);
        let mut affected_paths: HashSet<String> = HashSet::new();
        for entry in &self.idle {
            if Self::normalize_path(&entry.key.0).ends_with(&suffix) {
                affected_paths.insert(entry.key.0.clone());
            }
        }
        for path in &self.initialized_schemas {
            if Self::normalize_path(path).ends_with(&suffix) {
                affected_paths.insert(path.clone());
            }
        }
        for path in affected_paths {
            self.evict_path(&path);
        }
    }

    #[cfg(test)]
    fn evict_prefix(&mut self, prefix: &str) {
        let norm_prefix = Self::normalize_path(prefix);
        let mut affected: HashSet<String> = HashSet::new();
        for entry in &self.idle {
            if Self::normalize_path(&entry.key.0).starts_with(&norm_prefix) {
                affected.insert(entry.key.0.clone());
            }
        }
        for path in &self.initialized_schemas {
            if Self::normalize_path(path).starts_with(&norm_prefix) {
                affected.insert(path.clone());
            }
        }
        for path in affected {
            self.evict_path(&path);
        }
    }
}

static TENANT_DB_POOL: LazyLock<Mutex<TenantDbPool>> =
    LazyLock::new(|| Mutex::new(TenantDbPool::default()));

fn configured_pool_capacity() -> usize {
    std::env::var("AEGIS_TENANT_DB_POOL_SIZE")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|n| n.clamp(1, 256))
        .unwrap_or(DEFAULT_TENANT_DB_POOL_SIZE)
}

fn tenant_db_path(tenant_id: &str) -> String {
    let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
    format!("{}/users/{}/memory.db", base_dir, tenant_id)
}

fn key_fingerprint(session_key: &str) -> String {
    hex::encode(Sha256::digest(session_key.as_bytes()))
}

/// CORE-401: Escribe secretos o llaves privadas en disco con permisos restrictivos
/// (`0600` en Unix) para evitar lectura por otros usuarios locales.
pub fn write_secret_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content)?;
        file.sync_all()?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, content)
    }
}

/// --- TENANT DB (SECURE ENCLAVE) ---
/// Gestiona una base de datos SQLite encriptada con SQLCipher por cada tenant,
/// reutilizando conexiones descifradas mediante un pool LRU en memoria (ADR-034 / CORE-402).
pub struct TenantDB {
    connection: ManuallyDrop<Connection>,
    pool_key: Option<PoolCacheKey>,
    generation: u64,
}

impl Drop for TenantDB {
    fn drop(&mut self) {
        // SAFETY: `self.connection` is only taken once in `Drop::drop` and never accessed after.
        let conn = unsafe { ManuallyDrop::take(&mut self.connection) };
        if let Some(key) = self.pool_key.take() {
            let cap = configured_pool_capacity();
            let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            pool.put(key, conn, self.generation, cap);
        }
    }
}

impl TenantDB {
    /// Inicializa o reutiliza una conexión SQLCipher descifrada desde el pool LRU para un tenant.
    /// Aplica la `session_key` mediante PRAGMA key en misses y ejecuta `init_schema()` una sola vez
    /// por ruta inicializada.
    /// CORE-399: Rechaza `session_key` vacío o `"default"` y NUNCA cuarentena ni
    /// sobrescribe `memory.db` automáticamente ante un fallo de descifrado.
    pub fn open(tenant_id: &str, session_key: &str) -> Result<Self> {
        use anyhow::Context;
        let trimmed_key = session_key.trim();
        if trimmed_key.is_empty() || trimmed_key == "default" {
            anyhow::bail!("Invalid or empty session key for tenant {}", tenant_id);
        }

        let db_path = tenant_db_path(tenant_id);
        let cache_key = (db_path.clone(), key_fingerprint(session_key));
        let disk_exists = Path::new(&db_path).exists();

        let (generation, schema_already_initialized) = {
            let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            if !disk_exists {
                // If an external cleanup removed the DB file, clear any stale cached handles/schema state.
                pool.evict_path(&db_path);
                (pool.generation_of(&db_path), false)
            } else if let Some((conn, generation)) = pool.take(&cache_key, Instant::now()) {
                return Ok(Self {
                    connection: ManuallyDrop::new(conn),
                    pool_key: Some(cache_key),
                    generation,
                });
            } else {
                (
                    pool.generation_of(&db_path),
                    pool.initialized_schemas.contains(&db_path),
                )
            }
        };

        // Asegurar que el directorio del tenant existe
        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for tenant {}", tenant_id))?;
        }

        let conn = Self::try_open_with_key(&db_path, session_key)?;
        info!(tenant_id = %tenant_id, "Secure Enclave initialized successfully.");

        let mut db = Self {
            connection: ManuallyDrop::new(conn),
            pool_key: Some(cache_key),
            generation,
        };

        if !schema_already_initialized {
            if let Err(err) = db.init_schema() {
                db.pool_key = None;
                return Err(err);
            }
            let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            if pool.generation_of(&db_path) == generation {
                pool.initialized_schemas.insert(db_path);
            }
        }

        Ok(db)
    }

    /// CORE-402: Ejecuta un closure de lectura/escritura sobre `TenantDB` dentro de
    /// `tokio::task::spawn_blocking`, evitando bloquear workers async de Tokio.
    pub async fn with_async<F, T>(tenant_id: String, session_key: String, f: F) -> Result<T>
    where
        F: FnOnce(&TenantDB) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(move || {
            let db = Self::open(&tenant_id, &session_key)?;
            f(&db)
        })
        .await
        .map_err(|e| anyhow::anyhow!("TenantDB blocking task failed: {}", e))?
    }

    /// CORE-402: Expulsa del pool LRU todas las conexiones cacheadas e inicializaciones de esquema
    /// asociadas a `tenant_id` (obligatorio al rotar clave, cuarentenar o eliminar tenant).
    pub fn evict_tenant(tenant_id: &str) {
        let db_path = tenant_db_path(tenant_id);
        let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
        pool.evict_tenant(tenant_id, &db_path);
    }

    /// CORE-403: Cierra ordenadamente todas las conexiones en el pool LRU durante el apagado.
    pub fn evict_all() {
        let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
        pool.idle.clear();
        pool.initialized_schemas.clear();
        pool.generations.clear();
    }

    #[cfg(test)]
    pub(crate) fn evict_prefix_for_test(prefix: &str) {
        let mut pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
        pool.evict_prefix(prefix);
    }

    /// Opens the DB at `path`, applies the SQLCipher key and verifies it can be
    /// read. Returns the live connection on success.
    fn try_open_with_key(path: &str, session_key: &str) -> Result<Connection> {
        use anyhow::Context;
        let conn =
            Connection::open(path).with_context(|| format!("Failed to open database at {path}"))?;

        // Add busy_timeout to prevent "database is locked" during concurrent access.
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .context("Failed to set busy timeout")?;

        // PRAGMA key must be the first statement and must not return rows.
        conn.pragma_update(None, "key", session_key)
            .context("Failed to apply PRAGMA key for encryption")?;

        // SQLCipher doesn't validate the key until the data is accessed.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid session key or corrupted database")?;

        let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;");

        Ok(conn)
    }

    /// CORE-399: Respalda explícitamente la base cifrada de un tenant tras un reset de
    /// contraseña administrativo (renombra `memory.db`, `-wal`, `-shm` a `.locked-<ts>`),
    /// preservando el ciphertext anterior sin borrarlo.
    pub fn quarantine_for_password_reset(tenant_id: &str) -> Result<Option<String>> {
        Self::evict_tenant(tenant_id);
        let db_path = tenant_db_path(tenant_id);
        if !Path::new(&db_path).exists() {
            return Ok(None);
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup_base = format!("{db_path}.locked-{ts}");

        for suffix in ["", "-wal", "-shm"] {
            let from = format!("{db_path}{suffix}");
            if Path::new(&from).exists() {
                let to = format!("{backup_base}{suffix}");
                std::fs::rename(&from, &to)?;
            }
        }
        tracing::warn!(
            tenant_id = %tenant_id,
            backup = %backup_base,
            "CORE-399: Tenant DB quarantined to .locked-* after explicit admin password reset."
        );
        Ok(Some(backup_base))
    }

    /// Crea las tablas necesarias para el estado del Kernel si no existen.
    fn init_schema(&self) -> Result<()> {
        use anyhow::Context;
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize kv_store table")?;

        // Domain: Ledger (Finanzas)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS expenses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                amount REAL NOT NULL,
                description TEXT NOT NULL,
                category TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize expenses table")?;

        // Domain: Chronos (Tiempo/Recordatorios)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                remind_at DATETIME NOT NULL,
                description TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                recurrence TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize reminders table")?;

        // Migración idempotente para bases existentes sin columna `recurrence`
        if let Err(e) = self.connection.execute(
            "ALTER TABLE reminders ADD COLUMN recurrence TEXT NOT NULL DEFAULT ''",
            [],
        ) {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("duplicate column") {
                return Err(anyhow::anyhow!(
                    "Failed to migrate reminders.recurrence: {}",
                    e
                ));
            }
        }

        // CORE-375: Tabla personal_facts para memoria a largo plazo del tenant
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS personal_facts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL DEFAULT 'general',
                fact TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'chat',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize personal_facts table")?;

        // Epic 44: Developer Workspace — configuración del workspace por tenant
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS workspace_config (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
                [],
            )
            .context("Failed to initialize workspace_config table")?;

        // Epic 44: PR Manager — PRs gestionados por Aegis
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS managed_prs (
                pr_number         INTEGER PRIMARY KEY,
                title             TEXT NOT NULL,
                branch            TEXT NOT NULL,
                base_branch       TEXT NOT NULL DEFAULT 'main',
                url               TEXT NOT NULL,
                merge_mode        TEXT NOT NULL DEFAULT 'manual',
                auto_fix_ci       INTEGER NOT NULL DEFAULT 1,
                auto_fix_attempts INTEGER NOT NULL DEFAULT 0,
                status            TEXT NOT NULL DEFAULT 'open',
                created_at        TEXT NOT NULL,
                updated_at        TEXT NOT NULL
            )",
                [],
            )
            .context("Failed to initialize managed_prs table")?;

        // Replicación y Sincronización (Fase 3)
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS sync_metadata (
                device_id TEXT PRIMARY KEY,
                last_seq_num INTEGER NOT NULL DEFAULT 0,
                last_sync_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize sync_metadata table")?;

        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                table_name TEXT NOT NULL,
                row_key TEXT NOT NULL,
                operation TEXT NOT NULL,
                data_json TEXT,
                client_id TEXT NOT NULL DEFAULT 'server',
                seq_num INTEGER NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize sync_log table")?;

        Ok(())
    }

    /// Registra una entrada en el diario de sincronización para replicación delta.
    pub fn record_sync_log(
        &self,
        table_name: &str,
        row_key: &str,
        operation: &str,
        data_json: Option<&str>,
    ) -> Result<()> {
        use anyhow::Context;
        // Obtener el próximo seq_num de forma monótona
        let next_seq: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(seq_num), 0) + 1 FROM sync_log",
                [],
                |row| row.get(0),
            )
            .unwrap_or(1);

        self.connection.execute(
            "INSERT INTO sync_log (table_name, row_key, operation, data_json, client_id, seq_num, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'server', ?5, CURRENT_TIMESTAMP)",
            rusqlite::params![table_name, row_key, operation, data_json, next_seq],
        ).with_context(|| format!("Failed to record sync log entry for {}", table_name))?;

        Ok(())
    }

    /// Inserta o actualiza un valor en el almacén seguro.
    pub fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        use anyhow::Context;
        self.connection.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            [key, value],
        ).with_context(|| format!("Failed to set KV: {}", key))?;

        // Evitamos registrar los tokens OAuth temporales o estados de onboarding
        if !key.starts_with("oauth_") && !key.starts_with("onboarding_") {
            let data = serde_json::json!({ "value": value });
            let _ = self.record_sync_log("kv_store", key, "UPDATE", Some(&data.to_string()));
        }

        Ok(())
    }

    /// Recupera un valor del almacén seguro.
    pub fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .connection
            .prepare("SELECT value FROM kv_store WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_oauth_token(
        &self,
        provider: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in_secs: u64,
        scope: &str,
    ) -> Result<()> {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .saturating_add(expires_in_secs);

        self.set_kv(&format!("oauth_{}_access_token", provider), access_token)?;
        self.set_kv(&format!("oauth_{}_expiry", provider), &expiry.to_string())?;
        self.set_kv(&format!("oauth_{}_scope", provider), scope)?;
        if let Some(rt) = refresh_token {
            self.set_kv(&format!("oauth_{}_refresh_token", provider), rt)?;
        }
        Ok(())
    }

    pub fn get_valid_access_token(&self, provider: &str) -> Result<Option<String>> {
        let token = self.get_kv(&format!("oauth_{}_access_token", provider))?;
        let expiry = self.get_kv(&format!("oauth_{}_expiry", provider))?;
        match (token, expiry) {
            (Some(t), Some(exp)) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                let exp_secs: u64 = exp.parse().unwrap_or(0);
                if now + 60 < exp_secs {
                    Ok(Some(t))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    pub fn get_refresh_token(&self, provider: &str) -> Result<Option<String>> {
        self.get_kv(&format!("oauth_{}_refresh_token", provider))
    }

    // ── Internal accessor (Epic 44) ──────────────────────────────────────
    pub fn connection(&self) -> &rusqlite::Connection {
        &self.connection
    }

    // ── Managed PRs helpers (Epic 44) ─────────────────────────────────────
    pub fn pr_set_merge_mode(&self, pr_number: u64, mode: &str) -> Result<()> {
        use anyhow::Context;
        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "UPDATE managed_prs SET merge_mode = ?1, updated_at = ?2 WHERE pr_number = ?3",
                rusqlite::params![mode, now, pr_number as i64],
            )
            .context("Failed to update merge_mode")?;
        Ok(())
    }

    pub fn pr_set_auto_fix_ci(&self, pr_number: u64, enabled: bool) -> Result<()> {
        use anyhow::Context;
        let now = chrono::Utc::now().to_rfc3339();
        self.connection
            .execute(
                "UPDATE managed_prs SET auto_fix_ci = ?1, updated_at = ?2 WHERE pr_number = ?3",
                rusqlite::params![enabled as i64, now, pr_number as i64],
            )
            .context("Failed to update auto_fix_ci")?;
        Ok(())
    }

    // ── Workspace Config (Epic 44) ────────────────────────────────────────
    pub fn workspace_config_set(&self, key: &str, value: &str) -> Result<()> {
        use anyhow::Context;
        self.connection
            .execute(
                "INSERT OR REPLACE INTO workspace_config (key, value) VALUES (?1, ?2)",
                [key, value],
            )
            .with_context(|| format!("Failed to set workspace_config key: {}", key))?;
        Ok(())
    }

    pub fn workspace_config_get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .connection
            .prepare("SELECT value FROM workspace_config WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_oauth_scope(&self, provider: &str) -> Result<Option<String>> {
        self.get_kv(&format!("oauth_{}_scope", provider))
    }

    pub fn set_oauth_email(&self, provider: &str, email: &str) -> Result<()> {
        self.set_kv(&format!("oauth_{}_email", provider), email)
    }

    pub fn get_oauth_email(&self, provider: &str) -> Result<Option<String>> {
        self.get_kv(&format!("oauth_{}_email", provider))
    }

    pub fn set_oauth_client_config(
        &self,
        provider: &str,
        client_id: &str,
        client_secret: Option<&str>,
        redirect_uri: Option<&str>,
    ) -> Result<()> {
        let trimmed_id = client_id.trim();
        if !trimmed_id.is_empty() {
            self.set_kv(&format!("oauth_{}_client_id", provider), trimmed_id)?;
        }
        if let Some(secret) = client_secret {
            let trimmed_secret = secret.trim();
            if !trimmed_secret.is_empty() {
                self.set_kv(&format!("oauth_{}_client_secret", provider), trimmed_secret)?;
            }
        }
        if let Some(uri) = redirect_uri {
            self.set_kv(&format!("oauth_{}_redirect_uri", provider), uri.trim())?;
        }
        Ok(())
    }

    pub fn get_oauth_client_config(
        &self,
        provider: &str,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        let client_id = self
            .get_kv(&format!("oauth_{}_client_id", provider))?
            .filter(|s| !s.trim().is_empty());
        let client_secret = self
            .get_kv(&format!("oauth_{}_client_secret", provider))?
            .filter(|s| !s.trim().is_empty());
        let redirect_uri = self
            .get_kv(&format!("oauth_{}_redirect_uri", provider))?
            .filter(|s| !s.trim().is_empty());
        Ok((client_id, client_secret, redirect_uri))
    }

    pub fn clear_oauth_client_config(&self, provider: &str) -> Result<()> {
        for suffix in &["client_id", "client_secret", "redirect_uri"] {
            let _ = self.connection.execute(
                "DELETE FROM kv_store WHERE key = ?1",
                [&format!("oauth_{}_{}", provider, suffix)],
            );
        }
        Ok(())
    }

    pub fn is_oauth_connected(&self, provider: &str) -> Result<bool> {
        Ok(self.get_refresh_token(provider)?.is_some()
            || self.get_valid_access_token(provider)?.is_some())
    }

    pub fn revoke_oauth(&self, provider: &str) -> Result<()> {
        for suffix in &["access_token", "refresh_token", "expiry", "scope", "email"] {
            let _ = self.connection.execute(
                "DELETE FROM kv_store WHERE key = ?1",
                [&format!("oauth_{}_{}", provider, suffix)],
            );
        }
        Ok(())
    }
}

const PERSONA_KEY: &str = "agent_persona";
const PERSONA_MAX_LEN: usize = 4000;
const ONBOARDING_STEP_KEY: &str = "onboarding_step";
const ONBOARDING_NAME_KEY: &str = "onboarding_pending_name";

impl TenantDB {
    pub fn set_persona(&self, persona: &str) -> Result<()> {
        anyhow::ensure!(
            persona.len() <= PERSONA_MAX_LEN,
            "Persona exceeds maximum length of {} characters",
            PERSONA_MAX_LEN
        );
        self.set_kv(PERSONA_KEY, persona)
    }

    pub fn get_persona(&self) -> Result<Option<String>> {
        self.get_kv(PERSONA_KEY)
    }

    pub fn delete_persona(&self) -> Result<()> {
        use anyhow::Context;
        self.connection
            .execute("DELETE FROM kv_store WHERE key = ?1", [PERSONA_KEY])
            .context("Failed to delete agent persona")?;
        Ok(())
    }

    /// Retorna el step actual del onboarding:
    /// None = no iniciado, Some("name") = esperando nombre, Some("style") = esperando estilo
    pub fn get_onboarding_step(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_STEP_KEY)
    }

    pub fn set_onboarding_step(&self, step: &str) -> Result<()> {
        self.set_kv(ONBOARDING_STEP_KEY, step)
    }

    pub fn clear_onboarding(&self) -> Result<()> {
        self.connection.execute(
            "DELETE FROM kv_store WHERE key IN (?1, ?2)",
            [ONBOARDING_STEP_KEY, ONBOARDING_NAME_KEY],
        )?;
        Ok(())
    }

    pub fn set_onboarding_name(&self, name: &str) -> Result<()> {
        self.set_kv(ONBOARDING_NAME_KEY, name)
    }

    pub fn get_onboarding_name(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_NAME_KEY)
    }

    // --- LEDGER METHODS ---
    pub fn add_expense(
        &self,
        amount: f64,
        description: &str,
        category: Option<&str>,
    ) -> Result<()> {
        use anyhow::Context;
        self.connection
            .execute(
                "INSERT INTO expenses (amount, description, category) VALUES (?1, ?2, ?3)",
                (amount, description, category),
            )
            .context("Failed to insert expense")?;

        let row_id = self.connection.last_insert_rowid();
        let data = serde_json::json!({
            "amount": amount,
            "description": description,
            "category": category
        });
        let _ = self.record_sync_log(
            "expenses",
            &row_id.to_string(),
            "INSERT",
            Some(&data.to_string()),
        );

        Ok(())
    }

    pub fn get_expenses(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.connection.prepare(
            "SELECT amount, description, category, created_at FROM expenses ORDER BY created_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(serde_json::json!({
                "amount": row.get::<_, f64>(0)?,
                "description": row.get::<_, String>(1)?,
                "category": row.get::<_, Option<String>>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // --- CHRONOS METHODS ---
    pub fn add_reminder(&self, remind_at: &str, description: &str) -> Result<()> {
        self.add_reminder_with_recurrence(remind_at, description, None)?;
        Ok(())
    }

    pub fn add_reminder_with_recurrence(
        &self,
        remind_at: &str,
        description: &str,
        recurrence: Option<&str>,
    ) -> Result<i64> {
        use anyhow::Context;
        let rec = recurrence.map(normalize_recurrence).unwrap_or_default();
        self.connection
            .execute(
                "INSERT INTO reminders (remind_at, description, recurrence) VALUES (?1, ?2, ?3)",
                rusqlite::params![remind_at, description, rec],
            )
            .context("Failed to insert reminder")?;

        let row_id = self.connection.last_insert_rowid();
        let data = serde_json::json!({
            "id": row_id,
            "remind_at": remind_at,
            "description": description,
            "status": "pending",
            "recurrence": rec
        });
        let _ = self.record_sync_log(
            "reminders",
            &row_id.to_string(),
            "INSERT",
            Some(&data.to_string()),
        );

        Ok(row_id)
    }

    pub fn get_reminders(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, remind_at, description, status, COALESCE(recurrence, ''), created_at
             FROM reminders
             ORDER BY CASE WHEN status = 'pending' THEN 0 ELSE 1 END, remind_at ASC, id ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "remind_at": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "status": row.get::<_, String>(3)?,
                "recurrence": row.get::<_, String>(4)?,
                "created_at": row.get::<_, String>(5)?,
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn resolve_reminder_ids(&self, id_or_match: &str, only_pending: bool) -> Result<Vec<i64>> {
        let trimmed = id_or_match.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let id_candidate = trimmed.trim_start_matches('#').trim();
        let status_clause = if only_pending {
            "status != 'completed' AND "
        } else {
            ""
        };
        if let Ok(id) = id_candidate.parse::<i64>() {
            let sql = format!("SELECT id FROM reminders WHERE {}id = ?1", status_clause);
            let mut stmt = self.connection.prepare(&sql)?;
            let ids: Vec<i64> = stmt
                .query_map([id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(ids);
        }

        let pattern = format!("%{}%", trimmed.to_lowercase());
        let sql = format!(
            "SELECT id FROM reminders WHERE {}LOWER(description) LIKE ?1 ORDER BY id ASC",
            status_clause
        );
        let mut stmt = self.connection.prepare(&sql)?;
        let ids: Vec<i64> = stmt
            .query_map([pattern], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    pub fn update_reminder(
        &self,
        id_or_match: &str,
        new_remind_at: Option<&str>,
        new_description: Option<&str>,
        new_recurrence: Option<&str>,
    ) -> Result<usize> {
        let ids = self.resolve_reminder_ids(id_or_match, false)?;
        if ids.is_empty() {
            return Ok(0);
        }
        let norm_rec = new_recurrence.map(normalize_recurrence);
        let mut updated = 0usize;
        for id in ids {
            let mut count = 0usize;
            if let Some(remind_at) = new_remind_at.filter(|s| !s.trim().is_empty()) {
                count += self.connection.execute(
                    "UPDATE reminders SET remind_at = ?1, status = 'pending' WHERE id = ?2",
                    rusqlite::params![remind_at.trim(), id],
                )?;
            }
            if let Some(desc) = new_description.filter(|s| !s.trim().is_empty()) {
                let c = self.connection.execute(
                    "UPDATE reminders SET description = ?1 WHERE id = ?2",
                    rusqlite::params![desc.trim(), id],
                )?;
                count = count.max(c);
            }
            if let Some(ref rec) = norm_rec {
                let c = self.connection.execute(
                    "UPDATE reminders SET recurrence = ?1, status = 'pending' WHERE id = ?2",
                    rusqlite::params![rec, id],
                )?;
                count = count.max(c);
            }
            if count > 0 {
                updated += 1;
                let data = serde_json::json!({
                    "id": id,
                    "remind_at": new_remind_at,
                    "description": new_description,
                    "recurrence": norm_rec,
                });
                let _ = self.record_sync_log(
                    "reminders",
                    &id.to_string(),
                    "UPDATE",
                    Some(&data.to_string()),
                );
            }
        }
        Ok(updated)
    }

    pub fn delete_reminder(&self, id_or_match: &str) -> Result<usize> {
        let ids = self.resolve_reminder_ids(id_or_match, false)?;
        if ids.is_empty() {
            return Ok(0);
        }
        let mut deleted = 0usize;
        for id in ids {
            let n = self
                .connection
                .execute("DELETE FROM reminders WHERE id = ?1", [id])?;
            if n > 0 {
                deleted += n;
                let _ = self.record_sync_log("reminders", &id.to_string(), "DELETE", None);
            }
        }
        Ok(deleted)
    }

    pub fn fire_due_reminders(
        &self,
        now_local: chrono::DateTime<chrono::Local>,
    ) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.connection.prepare(
            "SELECT id, remind_at, description, COALESCE(recurrence, '')
             FROM reminders
             WHERE status = 'pending'
             ORDER BY remind_at ASC, id ASC",
        )?;
        let rows: Vec<(i64, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let now_naive = now_local.naive_local();
        let mut fired = Vec::new();

        for (id, remind_at, desc, recurrence) in rows {
            let Some(scheduled) = parse_reminder_datetime(&remind_at) else {
                continue;
            };
            if scheduled > now_naive {
                continue;
            }

            let norm_rec = normalize_recurrence(&recurrence);
            if let Some(mut next_dt) = compute_next_occurrence(scheduled, &norm_rec) {
                while next_dt <= now_naive {
                    match compute_next_occurrence(next_dt, &norm_rec) {
                        Some(step) => next_dt = step,
                        None => break,
                    }
                }
                let next_str = next_dt.format("%Y-%m-%d %H:%M").to_string();
                let changed = self.connection.execute(
                    "UPDATE reminders SET remind_at = ?1 WHERE id = ?2 AND remind_at = ?3 AND status = 'pending'",
                    rusqlite::params![next_str, id, remind_at],
                )?;
                if changed > 0 {
                    fired.push(serde_json::json!({
                        "id": id,
                        "description": desc,
                        "remind_at": remind_at,
                        "next_remind_at": next_str,
                        "recurrence": norm_rec,
                    }));
                }
            } else {
                let changed = self.connection.execute(
                    "UPDATE reminders SET status = 'completed' WHERE id = ?1 AND status = 'pending'",
                    [id],
                )?;
                if changed > 0 {
                    fired.push(serde_json::json!({
                        "id": id,
                        "description": desc,
                        "remind_at": remind_at,
                        "next_remind_at": serde_json::Value::Null,
                        "recurrence": "",
                    }));
                }
            }
        }

        Ok(fired)
    }

    // --- CORE-375: PERSONAL ASSISTANT METHODS ---

    pub fn update_personal_fact(
        &self,
        id: i64,
        category: Option<&str>,
        fact: &str,
        source: &str,
    ) -> Result<bool> {
        use anyhow::Context;
        let trimmed_fact = fact.trim();
        if trimmed_fact.is_empty() {
            anyhow::bail!("El hecho a actualizar no puede estar vacío");
        }

        let existing_cat: Option<String> = self
            .connection
            .query_row(
                "SELECT category FROM personal_facts WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .ok();
        let Some(old_cat) = existing_cat else {
            return Ok(false);
        };

        let cat = category
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&old_cat);
        let src = if source.trim().is_empty() {
            "chat"
        } else {
            source.trim()
        };

        let rows = self
            .connection
            .execute(
                "UPDATE personal_facts \
                 SET category = ?1, fact = ?2, source = ?3, created_at = CURRENT_TIMESTAMP \
                 WHERE id = ?4",
                rusqlite::params![cat, trimmed_fact, src, id],
            )
            .context("Failed to update personal fact")?;

        if rows > 0 {
            let data = serde_json::json!({
                "id": id,
                "category": cat,
                "fact": trimmed_fact,
                "source": src
            });
            let _ = self.record_sync_log(
                "personal_facts",
                &id.to_string(),
                "UPDATE",
                Some(&data.to_string()),
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn deduplicate_personal_facts(&self) -> Result<usize> {
        let mut stmt = self
            .connection
            .prepare("SELECT id, category, fact FROM personal_facts ORDER BY id DESC")?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let mut kept: Vec<(i64, String, String)> = Vec::new();
        let mut removed = 0usize;
        for (id, cat, fact) in rows {
            if kept
                .iter()
                .any(|(_, kc, kf)| facts_share_topic(&cat, &fact, kc, kf))
            {
                if self.delete_personal_fact(id).unwrap_or(false) {
                    removed += 1;
                }
            } else {
                kept.push((id, cat, fact));
            }
        }
        Ok(removed)
    }

    pub fn upsert_personal_fact(
        &self,
        target_id: Option<i64>,
        category: &str,
        fact: &str,
        source: &str,
    ) -> Result<PersonalFactUpsertOutcome> {
        use anyhow::Context;
        let trimmed_fact = fact.trim();
        if trimmed_fact.is_empty() {
            anyhow::bail!("El hecho a recordar no puede estar vacío");
        }

        let cat = if category.trim().is_empty() {
            "general"
        } else {
            category.trim()
        };
        let src = if source.trim().is_empty() {
            "chat"
        } else {
            source.trim()
        };

        if let Some(tid) = target_id {
            let prev: Option<String> = self
                .connection
                .query_row(
                    "SELECT fact FROM personal_facts WHERE id = ?1",
                    [tid],
                    |row| row.get(0),
                )
                .ok();
            if let Some(previous_fact) = prev {
                self.update_personal_fact(tid, Some(cat), trimmed_fact, src)?;
                // Eliminar duplicados antiguos del mismo tópico si existieran
                let mut stmt = self
                    .connection
                    .prepare("SELECT id, category, fact FROM personal_facts WHERE id != ?1")?;
                let others: Vec<(i64, String, String)> = stmt
                    .query_map([tid], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .filter_map(|r| r.ok())
                    .collect();
                drop(stmt);
                for (oid, ocat, ofact) in others {
                    if facts_share_topic(cat, trimmed_fact, &ocat, &ofact) {
                        let _ = self.delete_personal_fact(oid);
                    }
                }
                return Ok(PersonalFactUpsertOutcome::Updated {
                    id: tid,
                    previous_fact,
                });
            }
        }

        let mut stmt = self
            .connection
            .prepare("SELECT id, category, fact FROM personal_facts ORDER BY id DESC")?;
        let existing_rows: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        if let Some((exact_id, _, _)) = existing_rows
            .iter()
            .find(|(_, _, ef)| ef.trim().eq_ignore_ascii_case(trimmed_fact))
        {
            let primary_id = *exact_id;
            for (oid, ocat, ofact) in &existing_rows {
                if *oid != primary_id && facts_share_topic(cat, trimmed_fact, ocat, ofact) {
                    let _ = self.delete_personal_fact(*oid);
                }
            }
            return Ok(PersonalFactUpsertOutcome::AlreadyKnown { id: primary_id });
        }

        let matching: Vec<(i64, String)> = existing_rows
            .iter()
            .filter(|(_, ecat, efact)| facts_share_topic(cat, trimmed_fact, ecat, efact))
            .map(|(eid, _, efact)| (*eid, efact.clone()))
            .collect();

        if let Some((primary_id, previous_fact)) = matching.first().cloned() {
            self.update_personal_fact(primary_id, Some(cat), trimmed_fact, src)?;
            for (dup_id, _) in matching.iter().skip(1) {
                let _ = self.delete_personal_fact(*dup_id);
            }
            return Ok(PersonalFactUpsertOutcome::Updated {
                id: primary_id,
                previous_fact,
            });
        }

        self.connection
            .execute(
                "INSERT INTO personal_facts (category, fact, source) VALUES (?1, ?2, ?3)",
                [cat, trimmed_fact, src],
            )
            .context("Failed to insert personal fact")?;

        let row_id = self.connection.last_insert_rowid();
        let data = serde_json::json!({
            "category": cat,
            "fact": trimmed_fact,
            "source": src
        });
        let _ = self.record_sync_log(
            "personal_facts",
            &row_id.to_string(),
            "INSERT",
            Some(&data.to_string()),
        );

        Ok(PersonalFactUpsertOutcome::Inserted { id: row_id })
    }

    pub fn add_personal_fact(&self, category: &str, fact: &str, source: &str) -> Result<bool> {
        let outcome = self.upsert_personal_fact(None, category, fact, source)?;
        Ok(matches!(
            outcome,
            PersonalFactUpsertOutcome::Inserted { .. } | PersonalFactUpsertOutcome::Updated { .. }
        ))
    }

    pub fn get_personal_facts(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let _ = self.deduplicate_personal_facts();
        let mut stmt = self.connection.prepare(
            "SELECT id, category, fact, source, created_at FROM personal_facts ORDER BY id DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "category": row.get::<_, String>(1)?,
                "fact": row.get::<_, String>(2)?,
                "source": row.get::<_, String>(3)?,
                "created_at": row.get::<_, String>(4)?,
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn delete_personal_fact(&self, id: i64) -> Result<bool> {
        let rows = self
            .connection
            .execute("DELETE FROM personal_facts WHERE id = ?1", [id])?;
        if rows > 0 {
            let _ = self.record_sync_log("personal_facts", &id.to_string(), "DELETE", None);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn query_expenses_summary(
        &self,
        category_filter: Option<&str>,
    ) -> Result<serde_json::Value> {
        let filter = category_filter
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "all" && *s != "todos" && *s != "todas");

        let mut stmt = self.connection.prepare(
            "SELECT id, amount, description, category, created_at FROM expenses ORDER BY id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "general".to_string()),
                row.get::<_, String>(4)?,
            ))
        })?;

        let mut all_entries = Vec::new();
        for r in rows {
            let (id, amount, desc, cat, created_at) = r?;
            all_entries.push((id, amount, desc, cat, created_at));
        }

        let filtered: Vec<_> = if let Some(cat_f) = filter {
            let cat_lower = cat_f.to_lowercase();
            all_entries
                .into_iter()
                .filter(|(_, _, _, cat, _)| cat.to_lowercase() == cat_lower)
                .collect()
        } else {
            all_entries
        };

        let mut total = 0.0;
        let mut by_cat: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for (_, amount, _, cat, _) in &filtered {
            total += amount;
            *by_cat.entry(cat.clone()).or_insert(0.0) += amount;
        }

        let start_idx = if filtered.len() > 15 {
            filtered.len() - 15
        } else {
            0
        };
        let mut recent_entries = Vec::new();
        for (id, amount, desc, cat, created_at) in filtered[start_idx..].iter().rev() {
            recent_entries.push(serde_json::json!({
                "id": id,
                "amount": amount,
                "description": desc,
                "category": cat,
                "created_at": created_at,
            }));
        }

        Ok(serde_json::json!({
            "total": total,
            "totals_by_category": by_cat,
            "recent_entries": recent_entries
        }))
    }

    pub fn complete_reminder(&self, match_text: &str) -> Result<usize> {
        let ids = self.resolve_reminder_ids(match_text, true)?;
        if ids.is_empty() {
            return Ok(0);
        }
        let mut updated = 0usize;
        for id in ids {
            let n = self.connection.execute(
                "UPDATE reminders SET status = 'completed' WHERE id = ?1 AND status != 'completed'",
                [id],
            )?;
            if n > 0 {
                updated += n;
                let data = serde_json::json!({ "id": id, "status": "completed" });
                let _ = self.record_sync_log(
                    "reminders",
                    &id.to_string(),
                    "UPDATE",
                    Some(&data.to_string()),
                );
            }
        }
        Ok(updated)
    }

    pub fn build_personal_memory_prompt_block(&self) -> Result<String> {
        let facts = self.get_personal_facts(30)?;
        let expenses = self.query_expenses_summary(None)?;
        let mut rem_stmt = self.connection.prepare(
            "SELECT id, remind_at, description, COALESCE(recurrence, '')
             FROM reminders
             WHERE status = 'pending'
             ORDER BY remind_at ASC, id ASC
             LIMIT 10",
        )?;
        let pending_reminders: Vec<(i64, String, String, String)> = rem_stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut block = String::new();
        block.push_str("\n\n[MEMORIA A LARGO PLAZO DEL USUARIO]\n");
        if facts.is_empty() {
            block.push_str("- Sin hechos registrados aún.\n");
        } else {
            for f in facts {
                let fact_id = f.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let fact_text = f.get("fact").and_then(|v| v.as_str()).unwrap_or_default();
                let category = f
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general");
                block.push_str(&format!("- [#{}] [{}] {}\n", fact_id, category, fact_text));
            }
        }

        block.push_str("\n[RESUMEN FINANCIERO Y RECORDATORIOS]\n");
        let total = expenses
            .get("total")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        block.push_str(&format!("- Gastos totales acumulados: ${:.2}\n", total));
        if let Some(by_cat) = expenses
            .get("totals_by_category")
            .and_then(|v| v.as_object())
        {
            if !by_cat.is_empty() {
                let cat_strs: Vec<String> = by_cat
                    .iter()
                    .map(|(k, v)| format!("{}: ${:.0}", k, v.as_f64().unwrap_or(0.0)))
                    .collect();
                block.push_str(&format!("- Desglose: {}\n", cat_strs.join(", ")));
            }
        }

        if pending_reminders.is_empty() {
            block.push_str("- Recordatorios pendientes: ninguno.\n");
        } else {
            block.push_str("- Recordatorios pendientes:\n");
            for (id, remind_at, desc, recurrence) in pending_reminders {
                if recurrence.is_empty() {
                    block.push_str(&format!("  * [#{}] {} ({})\n", id, desc, remind_at));
                } else {
                    block.push_str(&format!(
                        "  * [#{}] {} ({}, recurrente: {})\n",
                        id, desc, remind_at, recurrence
                    ));
                }
            }
        }

        Ok(block)
    }

    /// CORE-408: Context-aware personal memory block for chat turns.
    /// Injects financial totals and reminder lists only when the query touches
    /// those domains (or when a reminder is due soon), and ranks long-term
    /// facts by lexical relevance when the tenant has > 8 facts.
    pub fn build_contextual_personal_memory_block(&self, query: &str) -> Result<String> {
        let lower = fold_spanish_lowercase(query);
        let wants_finance = [
            "$",
            "gast",
            "compr",
            "pag",
            "cobr",
            "sueldo",
            "ingres",
            "plata",
            "dinero",
            "presupuesto",
            "finanz",
            "super",
            "nafta",
            "alquiler",
            "ledger",
            "resumen",
            "cuanto llev",
            "cuanto gast",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let wants_reminders = [
            "recorda",
            "alarma",
            "avis",
            "agenda",
            "pendiente",
            "turno",
            "cita",
            "clase",
            "horario",
            "reunion",
            "hoy",
            "manana",
            "semana",
            "viernes",
            "lunes",
            "martes",
            "miercoles",
            "jueves",
            "sabado",
            "domingo",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let wants_all_facts = [
            "que sabes de mi",
            "que recordas",
            "mi memoria",
            "mis datos",
            "mis notas",
            "sobre mi",
            "olvid",
            "actualiza",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let all_facts = self.get_personal_facts(30)?;
        let selected_facts: Vec<serde_json::Value> = if all_facts.len() <= 8 || wants_all_facts {
            all_facts
        } else {
            let q_tokens = normalize_fact_topic_tokens(query);
            let mut scored: Vec<(usize, usize, serde_json::Value)> = all_facts
                .into_iter()
                .enumerate()
                .map(|(idx, f)| {
                    let fact_txt = f.get("fact").and_then(|v| v.as_str()).unwrap_or_default();
                    let cat = f
                        .get("category")
                        .and_then(|v| v.as_str())
                        .unwrap_or("general");
                    let f_tokens = normalize_fact_topic_tokens(fact_txt);
                    let overlap = q_tokens.iter().filter(|t| f_tokens.contains(*t)).count();
                    let cat_bonus = if matches!(cat, "preferencias" | "salud") {
                        1
                    } else {
                        0
                    };
                    (overlap * 3 + cat_bonus, usize::MAX - idx, f)
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
            scored.into_iter().take(8).map(|(_, _, f)| f).collect()
        };

        let mut block = String::new();
        if !selected_facts.is_empty() {
            block.push_str("\n[MEMORIA A LARGO PLAZO DEL USUARIO]\n");
            for f in &selected_facts {
                let fact_id = f.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let fact_text = f.get("fact").and_then(|v| v.as_str()).unwrap_or_default();
                let category = f
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("general");
                block.push_str(&format!("- [#{}] [{}] {}\n", fact_id, category, fact_text));
            }
        }

        if wants_finance {
            let expenses = self.query_expenses_summary(None)?;
            let total = expenses
                .get("total")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            block.push_str("\n[RESUMEN FINANCIERO]\n");
            block.push_str(&format!("- Gastos acumulados: ${:.2}\n", total));
            if let Some(by_cat) = expenses
                .get("totals_by_category")
                .and_then(|v| v.as_object())
            {
                if !by_cat.is_empty() {
                    let cat_strs: Vec<String> = by_cat
                        .iter()
                        .map(|(k, v)| format!("{}: ${:.0}", k, v.as_f64().unwrap_or(0.0)))
                        .collect();
                    block.push_str(&format!("- Desglose: {}\n", cat_strs.join(", ")));
                }
            }
        }

        if wants_reminders {
            let mut rem_stmt = self.connection.prepare(
                "SELECT id, remind_at, description, COALESCE(recurrence, '')
                 FROM reminders
                 WHERE status = 'pending'
                 ORDER BY remind_at ASC, id ASC
                 LIMIT 8",
            )?;
            let pending_reminders: Vec<(i64, String, String, String)> = rem_stmt
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            if !pending_reminders.is_empty() {
                block.push_str("\n[RECORDATORIOS PENDIENTES]\n");
                for (id, remind_at, desc, recurrence) in pending_reminders {
                    if recurrence.is_empty() {
                        block.push_str(&format!("* [#{}] {} ({})\n", id, desc, remind_at));
                    } else {
                        block.push_str(&format!(
                            "* [#{}] {} ({}, recurrente: {})\n",
                            id, desc, remind_at, recurrence
                        ));
                    }
                }
            }
        }

        Ok(block)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonalFactUpsertOutcome {
    Inserted { id: i64 },
    Updated { id: i64, previous_fact: String },
    AlreadyKnown { id: i64 },
}

fn fold_spanish_lowercase(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.trim().to_lowercase().chars() {
        let mapped = match ch {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => ch,
        };
        out.push(mapped);
    }
    out
}

fn stem_spanish_token(tok: &str) -> String {
    if tok.len() > 6 {
        if let Some(stem) = tok.strip_suffix("ciones") {
            return format!("{}cion", stem);
        }
    }
    if tok.len() > 4 {
        if let Some(stem) = tok.strip_suffix("ces") {
            return format!("{}z", stem);
        }
        if let Some(stem) = tok.strip_suffix("es") {
            if matches!(
                stem.chars().last(),
                Some('r' | 'n' | 'l' | 'd' | 'j' | 's' | 'z')
            ) {
                return stem.to_string();
            }
        }
    }
    if tok.len() > 3 {
        if let Some(stem) = tok.strip_suffix('s') {
            return stem.to_string();
        }
    }
    tok.to_string()
}

pub fn normalize_fact_topic_tokens(fact: &str) -> Vec<String> {
    const STOP_AND_QUANTIFIERS: &[&str] = &[
        "un",
        "uno",
        "una",
        "unos",
        "unas",
        "dos",
        "tres",
        "cuatro",
        "cinco",
        "seis",
        "siete",
        "ocho",
        "nueve",
        "diez",
        "primer",
        "primero",
        "segunda",
        "segundo",
        "tercer",
        "tercera",
        "tercero",
        "otro",
        "otra",
        "otros",
        "otras",
        "mas",
        "menos",
        "varios",
        "varias",
        "solo",
        "solamente",
        "actualmente",
        "ahora",
        "hoy",
        "el",
        "la",
        "los",
        "las",
        "de",
        "del",
        "al",
        "a",
        "en",
        "con",
        "por",
        "para",
        "sin",
        "sobre",
        "es",
        "soy",
        "sos",
        "son",
        "fue",
        "era",
        "ser",
        "esta",
        "estoy",
        "estan",
        "estar",
        "tiene",
        "tengo",
        "tenes",
        "tienes",
        "tener",
        "hay",
        "hace",
        "hacer",
        "da",
        "doy",
        "dar",
        "dicta",
        "dicto",
        "dictar",
        "trabaja",
        "trabajo",
        "trabajar",
        "usuario",
        "persona",
        "que",
        "como",
        "cuando",
        "donde",
        "y",
        "e",
        "o",
        "u",
        "pero",
        "tambien",
        "ademas",
        "su",
        "sus",
        "mi",
        "mis",
        "tu",
        "tus",
        "me",
        "te",
        "se",
        "le",
        "les",
        "lo",
        "nos",
    ];

    let folded = fold_spanish_lowercase(fact);
    let mut tokens: Vec<String> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .filter(|t| !STOP_AND_QUANTIFIERS.contains(t))
        .map(stem_spanish_token)
        .filter(|t| t.chars().count() >= 3)
        .collect();

    tokens.sort();
    tokens.dedup();
    tokens
}

pub fn facts_share_topic(cat_a: &str, fact_a: &str, cat_b: &str, fact_b: &str) -> bool {
    let clean_a = fact_a.trim();
    let clean_b = fact_b.trim();
    if clean_a.eq_ignore_ascii_case(clean_b) {
        return true;
    }

    let tokens_a = normalize_fact_topic_tokens(clean_a);
    let tokens_b = normalize_fact_topic_tokens(clean_b);
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return false;
    }

    if tokens_a == tokens_b {
        return true;
    }

    let intersection = tokens_a.iter().filter(|t| tokens_b.contains(*t)).count();
    if intersection < 2 {
        return false;
    }

    let union = tokens_a.len() + tokens_b.len() - intersection;
    let min_len = tokens_a.len().min(tokens_b.len());
    let jaccard = intersection as f64 / union as f64;
    let containment = intersection as f64 / min_len as f64;

    let ca = cat_a.trim().to_lowercase();
    let cb = cat_b.trim().to_lowercase();
    let same_or_general = ca == cb || ca == "general" || cb == "general";

    if same_or_general {
        jaccard >= 0.60 || containment >= 0.80
    } else {
        jaccard >= 0.75 || (containment >= 0.99 && min_len >= 2)
    }
}

const WEEKDAY_SPECS: &[(&[&str], &str, chrono::Weekday)] = &[
    (
        &["weekly:mon", "lunes", "monday", "lun"],
        "weekly:mon",
        chrono::Weekday::Mon,
    ),
    (
        &["weekly:tue", "martes", "tuesday", "mar"],
        "weekly:tue",
        chrono::Weekday::Tue,
    ),
    (
        &[
            "weekly:wed",
            "miercoles",
            "miércoles",
            "wednesday",
            "mie",
            "mié",
        ],
        "weekly:wed",
        chrono::Weekday::Wed,
    ),
    (
        &["weekly:thu", "jueves", "thursday", "jue"],
        "weekly:thu",
        chrono::Weekday::Thu,
    ),
    (
        &["weekly:fri", "viernes", "friday", "vie"],
        "weekly:fri",
        chrono::Weekday::Fri,
    ),
    (
        &["weekly:sat", "sabado", "sábado", "saturday", "sab", "sáb"],
        "weekly:sat",
        chrono::Weekday::Sat,
    ),
    (
        &["weekly:sun", "domingo", "sunday", "dom"],
        "weekly:sun",
        chrono::Weekday::Sun,
    ),
];

fn match_weekday(text: &str) -> Option<(&'static str, chrono::Weekday)> {
    let lower = text.to_lowercase();
    for (aliases, canonical, wd) in WEEKDAY_SPECS {
        if aliases
            .iter()
            .any(|a| lower.contains(a) && (a.starts_with("weekly:") || has_token(&lower, a)))
        {
            return Some((*canonical, *wd));
        }
    }
    None
}

fn has_token(haystack_lower: &str, needle: &str) -> bool {
    haystack_lower
        .split(|c: char| !c.is_alphanumeric() && c != ':')
        .any(|tok| tok == needle)
}

/// Normaliza una descripción de recurrencia libre en un identificador canónico.
pub fn normalize_recurrence(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    if lower.is_empty()
        || matches!(
            lower.as_str(),
            "none" | "once" | "unico" | "único" | "ninguna" | "ninguno" | "no"
        )
    {
        return String::new();
    }
    if lower == "weekdays"
        || lower.contains("lunes a viernes")
        || lower.contains("dias habiles")
        || lower.contains("días hábiles")
    {
        return "weekdays".to_string();
    }
    if lower == "daily"
        || lower.contains("todos los dias")
        || lower.contains("todos los días")
        || lower.contains("cada dia")
        || lower.contains("cada día")
        || lower == "diario"
        || lower == "diariamente"
    {
        return "daily".to_string();
    }
    if lower == "monthly" || lower.contains("cada mes") || lower.contains("mensual") {
        return "monthly".to_string();
    }
    if let Some((canonical, _)) = match_weekday(&lower) {
        return canonical.to_string();
    }
    if lower == "weekly" || lower.contains("semanal") || lower.contains("cada semana") {
        return "weekly".to_string();
    }
    lower
}

/// Extrae `(hora, minuto)` de referencias horarias como `"17hs"`, `"17:30"`, `"a las 9"`.
fn extract_time_hm(text: &str) -> Option<(u32, u32)> {
    let lower = text.trim().to_lowercase();
    for raw_tok in lower.split_whitespace() {
        let tok = raw_tok.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':');
        if let Some((h_str, rest)) = tok.split_once(':') {
            let m_digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !h_str.is_empty()
                && h_str.len() <= 2
                && h_str.chars().all(|c| c.is_ascii_digit())
                && m_digits.len() == 2
            {
                if let (Ok(h), Ok(m)) = (h_str.parse::<u32>(), m_digits.parse::<u32>()) {
                    if h < 24 && m < 60 {
                        return Some((h, m));
                    }
                }
            }
        }
        if let Some(num_str) = tok
            .strip_suffix("hs")
            .or_else(|| tok.strip_suffix('h'))
            .filter(|s| !s.is_empty() && s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit()))
        {
            if let Ok(h) = num_str.parse::<u32>() {
                if h < 24 {
                    return Some((h, 0));
                }
            }
        }
    }

    let words: Vec<&str> = lower.split_whitespace().collect();
    for pair in words.windows(2) {
        if let Ok(h) = pair[0].parse::<u32>() {
            if h < 24 && matches!(pair[1].trim_end_matches('.'), "hs" | "h" | "horas") {
                return Some((h, 0));
            }
        }
        if pair[0] == "las" {
            let num_part = pair[1].trim_matches(|c: char| !c.is_ascii_digit());
            if !num_part.is_empty() && num_part.len() <= 2 {
                if let Ok(h) = num_part.parse::<u32>() {
                    if h < 24 {
                        return Some((h, 0));
                    }
                }
            }
        }
    }
    None
}

/// Parsea `remind_at` en `NaiveDateTime` si incluye fecha y hora concretas.
pub fn parse_reminder_datetime(remind_at: &str) -> Option<chrono::NaiveDateTime> {
    let trimmed = remind_at.trim();
    for fmt in [
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, fmt) {
            return Some(dt);
        }
    }
    None
}

/// Convierte una referencia temporal natural (`"hoy 17hs"`, `"mañana 10:00"`, etc.) a `YYYY-MM-DD HH:MM`.
pub fn normalize_remind_at(
    time_hint: &str,
    recurrence: &str,
    now_local: chrono::DateTime<chrono::Local>,
) -> String {
    let trimmed = time_hint.trim();
    if let Some(dt) = parse_reminder_datetime(trimmed) {
        return dt.format("%Y-%m-%d %H:%M").to_string();
    }
    if chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();
    let hm = extract_time_hm(&lower);
    let today = now_local.date_naive();

    let mut date_opt: Option<chrono::NaiveDate> = None;
    for tok in lower.split_whitespace() {
        let clean = tok.trim_matches(|c: char| !c.is_ascii_digit() && c != '-');
        if clean.len() == 10 {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(clean, "%Y-%m-%d") {
                date_opt = Some(d);
                break;
            }
        }
    }

    if date_opt.is_none() {
        if lower.contains("pasado mañana") || lower.contains("pasado manana") {
            date_opt = today.checked_add_days(chrono::Days::new(2));
        } else if lower.contains("mañana") || lower.contains("manana") || lower.contains("tomorrow")
        {
            date_opt = today.checked_add_days(chrono::Days::new(1));
        } else if has_token(&lower, "hoy") || has_token(&lower, "today") {
            date_opt = Some(today);
        } else if let Some((_, target_wd)) =
            match_weekday(recurrence).or_else(|| match_weekday(&lower))
        {
            use chrono::Datelike;
            let mut days_ahead = (target_wd.num_days_from_monday() as i64
                - today.weekday().num_days_from_monday() as i64)
                .rem_euclid(7);
            if days_ahead == 0 {
                let explicit_next = lower.contains("próximo")
                    || lower.contains("proximo")
                    || lower.contains("que viene");
                let time_passed = hm
                    .and_then(|(h, m)| chrono::NaiveTime::from_hms_opt(h, m, 0))
                    .map(|t| t <= now_local.time())
                    .unwrap_or(false);
                if explicit_next || time_passed {
                    days_ahead = 7;
                }
            }
            date_opt = today.checked_add_days(chrono::Days::new(days_ahead as u64));
        } else if let Some((h, m)) = hm {
            if let Some(t) = chrono::NaiveTime::from_hms_opt(h, m, 0) {
                date_opt = if t > now_local.time() {
                    Some(today)
                } else {
                    today.checked_add_days(chrono::Days::new(1))
                };
            }
        }
    }

    match (date_opt, hm) {
        (Some(d), Some((h, m))) => format!("{} {:02}:{:02}", d.format("%Y-%m-%d"), h, m),
        (Some(d), None) => d.format("%Y-%m-%d").to_string(),
        _ if trimmed.is_empty() => "pendiente".to_string(),
        _ => trimmed.to_string(),
    }
}

/// Calcula la próxima ocurrencia futura para un recordatorio recurrente.
pub fn compute_next_occurrence(
    current: chrono::NaiveDateTime,
    recurrence: &str,
) -> Option<chrono::NaiveDateTime> {
    use chrono::Datelike;
    let rec = normalize_recurrence(recurrence);
    if rec.is_empty() {
        return None;
    }
    match rec.as_str() {
        "daily" => Some(current + chrono::Duration::days(1)),
        "weekdays" => {
            let mut next = current + chrono::Duration::days(1);
            while matches!(next.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
                next += chrono::Duration::days(1);
            }
            Some(next)
        }
        "weekly" => Some(current + chrono::Duration::days(7)),
        "monthly" => current
            .date()
            .checked_add_months(chrono::Months::new(1))
            .map(|d| chrono::NaiveDateTime::new(d, current.time())),
        other => {
            if let Some((_, target_wd)) = match_weekday(other) {
                let mut days = (target_wd.num_days_from_monday() as i64
                    - current.weekday().num_days_from_monday() as i64)
                    .rem_euclid(7);
                if days == 0 {
                    days = 7;
                }
                Some(current + chrono::Duration::days(days))
            } else {
                None
            }
        }
    }
}

/// CORE-375: Normalización semántica de categorías de gastos
pub fn normalize_expense_category(raw_category: &str, description: &str) -> String {
    let combined = format!(
        "{} {}",
        raw_category.to_lowercase(),
        description.to_lowercase()
    );

    let comida_keywords = [
        "super",
        "súper",
        "supermercado",
        "carniceria",
        "carnicería",
        "verduleria",
        "verdulería",
        "almacen",
        "almacén",
        "restaurante",
        "comida",
        "delivery",
        "panaderia",
    ];
    for kw in &comida_keywords {
        if combined.contains(kw) {
            return "comida".to_string();
        }
    }

    let transporte_keywords = [
        "nafta",
        "combustible",
        "uber",
        "cabify",
        "taxi",
        "sube",
        "colectivo",
        "peaje",
        "estacionamiento",
    ];
    for kw in &transporte_keywords {
        if combined.contains(kw) {
            return "transporte".to_string();
        }
    }

    let hogar_keywords = ["luz", "gas", "agua", "internet", "alquiler", "expensas"];
    for kw in &hogar_keywords {
        if combined.contains(kw) {
            return "hogar".to_string();
        }
    }

    let salud_keywords = ["farmacia", "medico", "médico", "dentista", "gimnasio"];
    for kw in &salud_keywords {
        if combined.contains(kw) {
            return "salud".to_string();
        }
    }

    let trimmed = raw_category.trim();
    if !trimmed.is_empty() {
        trimmed.to_lowercase()
    } else {
        "general".to_string()
    }
}

// --- CORE-276: Approved external paths ---
const APPROVED_PATHS_KEY: &str = "approved_paths";
const AUTONOMOUS_PROJECTS_KEY: &str = "autonomous_projects";

impl TenantDB {
    /// Returns the list of external paths approved by the user for specialist access.
    pub fn get_approved_paths(&self) -> Result<Vec<String>> {
        match self.get_kv(APPROVED_PATHS_KEY)? {
            Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
            None => Ok(vec![]),
        }
    }

    /// Adds a path to the approved list. Idempotent.
    pub fn add_approved_path(&self, path: &str) -> Result<()> {
        let mut paths = self.get_approved_paths().unwrap_or_default();
        if !paths.iter().any(|p| p == path) {
            paths.push(path.to_string());
        }
        let json = serde_json::to_string(&paths)
            .map_err(|e| anyhow::anyhow!("Failed to serialize approved_paths: {}", e))?;
        self.set_kv(APPROVED_PATHS_KEY, &json)
    }

    /// Revokes approval for a path.
    pub fn remove_approved_path(&self, path: &str) -> Result<()> {
        let paths: Vec<String> = self
            .get_approved_paths()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p != path)
            .collect();
        let json = serde_json::to_string(&paths)
            .map_err(|e| anyhow::anyhow!("Failed to serialize approved_paths: {}", e))?;
        self.set_kv(APPROVED_PATHS_KEY, &json)
    }

    // --- Per-project autonomous mode (skip path-approval gate) ---

    /// Project IDs the user marked as "autonomous". Specialists working in these
    /// projects skip the external-path approval gate — full filesystem access,
    /// no per-path prompts. Opt-in, per project.
    pub fn get_autonomous_projects(&self) -> Result<Vec<String>> {
        match self.get_kv(AUTONOMOUS_PROJECTS_KEY)? {
            Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
            None => Ok(vec![]),
        }
    }

    /// Whether a specific project is in autonomous mode.
    pub fn is_project_autonomous(&self, project_id: &str) -> bool {
        self.get_autonomous_projects()
            .map(|ps| ps.iter().any(|p| p == project_id))
            .unwrap_or(false)
    }

    /// Enable or disable autonomous mode for a project. Idempotent.
    pub fn set_project_autonomous(&self, project_id: &str, enabled: bool) -> Result<()> {
        let mut projects = self.get_autonomous_projects().unwrap_or_default();
        let present = projects.iter().any(|p| p == project_id);
        if enabled && !present {
            projects.push(project_id.to_string());
        } else if !enabled && present {
            projects.retain(|p| p != project_id);
        }
        let json = serde_json::to_string(&projects)
            .map_err(|e| anyhow::anyhow!("Failed to serialize autonomous_projects: {}", e))?;
        self.set_kv(AUTONOMOUS_PROJECTS_KEY, &json)
    }
}

#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn acquire_test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub(crate) struct ScopedTestDataDir {
    prev: Option<String>,
    tempdir: tempfile::TempDir,
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl ScopedTestDataDir {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let guard = acquire_test_lock();
        let prev = std::env::var("AEGIS_DATA_DIR").ok();
        let tempdir = tempfile::tempdir()?;
        std::env::set_var("AEGIS_DATA_DIR", tempdir.path());
        Ok(Self {
            prev,
            tempdir,
            _guard: guard,
        })
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        self.tempdir.path()
    }
}

#[cfg(test)]
impl Drop for ScopedTestDataDir {
    fn drop(&mut self) {
        TenantDB::evict_prefix_for_test(&self.tempdir.path().to_string_lossy());
        match &self.prev {
            Some(old) => std::env::set_var("AEGIS_DATA_DIR", old),
            None => std::env::remove_var("AEGIS_DATA_DIR"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use tempfile::tempdir;

    #[test]
    fn test_secure_enclave_decryption_failure() -> anyhow::Result<()> {
        let _guard = acquire_test_lock();
        let dir = tempdir().context("Failed to create tempdir")?;
        let base_path = dir.path();

        // Cambiamos el CWD o simplemente usamos una ruta controlada para el test
        let tenant_id = "test_user_789";
        let correct_key = "secure_pass_123";
        let wrong_key = "hacker_attack_456";

        // 1. Crear la DB con la llave correcta
        // Mocking the path for the test
        let db_path = base_path.join(format!("{}_memory.db", tenant_id));

        {
            let conn = Connection::open(&db_path).context("Failed to open test database")?;
            conn.pragma_update(None, "key", correct_key)
                .context("Failed to set correct key")?;
            conn.execute("CREATE TABLE test (id INTEGER)", [])
                .context("Failed to create test table")?;
            conn.execute("INSERT INTO test VALUES (1)", [])
                .context("Failed to insert test data")?;
        }

        // 2. Intentar abrir con la llave incorrectA y verificar fallo de desencriptación
        let conn_fail =
            Connection::open(&db_path).context("Failed to open database for wrong key test")?;
        conn_fail
            .pragma_update(None, "key", wrong_key)
            .context("Failed to set wrong key")?;

        // SQLCipher fallará aquí (file is not a database)
        let result = conn_fail.query_row("SELECT count(*) FROM test", [], |_| Ok(()));

        assert!(
            result.is_err(),
            "La base de datos NO debería permitir acceso con llave incorrecta"
        );
        Ok(())
    }

    #[test]
    fn test_tenant_db_persistence() {
        // En un entorno real './users' se crearía, aquí usamos tempdir para no ensuciar
        // Pero el struct TenantDB usa rutas relativas fijas, así que este test es de integración 'light'
        // NOTA: Para tests unitarios puros, TenantDB debería aceptar un 'base_path'.
        // Sin embargo, sigo la orden del usuario de usar ./users/{tenant_id}/memory.db.
    }

    #[test]
    fn test_persona_set_get_delete() -> anyhow::Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_persona_user";
        let correct_key = "test_key_123";

        {
            let db = TenantDB::open(tenant_id, correct_key)?;
            db.set_persona("Eres Eve, asistente de ACME Corp.")?;
            let loaded = db.get_persona()?;
            assert!(loaded.is_some(), "Persona should be stored");
            assert_eq!(
                loaded.unwrap_or_default(),
                "Eres Eve, asistente de ACME Corp."
            );
        }

        {
            let db = TenantDB::open(tenant_id, correct_key)?;
            let loaded = db.get_persona()?;
            assert!(loaded.is_some());
            db.delete_persona()?;
            let after_delete = db.get_persona()?;
            assert!(after_delete.is_none(), "Persona should be deleted");
        }

        Ok(())
    }

    #[test]
    fn test_persona_max_length() -> anyhow::Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_maxlen_user";
        let correct_key = "test_key_456";

        let too_long = "x".repeat(4001);
        {
            let db = TenantDB::open(tenant_id, correct_key)?;
            let result = db.set_persona(&too_long);
            assert!(result.is_err(), "Persona of 4001 chars should fail");

            let valid = "x".repeat(4000);
            db.set_persona(&valid)?;
            let loaded = db.get_persona()?;
            assert_eq!(loaded.unwrap_or_default().len(), 4000);
        }

        Ok(())
    }

    /// CORE-399: opening a tenant DB with a wrong, empty, or `"default"` key must
    /// fail without quarantining or recreating `memory.db`, so the tenant's original
    /// encrypted data remains intact for the real session key.
    #[test]
    fn test_open_with_wrong_key_fails_without_quarantine() -> anyhow::Result<()> {
        let scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_no_quarantine_user";

        {
            let db = TenantDB::open(tenant_id, "old_password_key")?;
            db.set_persona("Soy Aegis, directo y conciso.")?;
        }

        for bad_key in ["", "   ", "default", "wrong_password_key"] {
            let err = TenantDB::open(tenant_id, bad_key);
            assert!(
                err.is_err(),
                "TenantDB::open must reject invalid/wrong key '{bad_key}'"
            );
        }

        let user_dir = scoped.path().join("users").join(tenant_id);
        let had_backup = std::fs::read_dir(&user_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().contains(".locked-"))
            })
            .unwrap_or(false);
        assert!(
            !had_backup,
            "TenantDB::open must NEVER auto-quarantine memory.db"
        );

        let db = TenantDB::open(tenant_id, "old_password_key")?;
        assert_eq!(
            db.get_persona()?.unwrap_or_default(),
            "Soy Aegis, directo y conciso."
        );
        Ok(())
    }

    #[test]
    fn test_explicit_quarantine_for_password_reset() -> anyhow::Result<()> {
        let scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_explicit_reset_user";

        {
            let db = TenantDB::open(tenant_id, "old_password_key")?;
            db.set_persona("Persona anterior")?;
        }

        let backup = TenantDB::quarantine_for_password_reset(tenant_id)?;
        assert!(backup.is_some(), "Expected .locked-* backup to be created");

        let db = TenantDB::open(tenant_id, "new_password_key")?;
        assert!(db.get_persona()?.is_none());
        db.set_persona("Persona nueva")?;
        assert_eq!(db.get_persona()?.unwrap_or_default(), "Persona nueva");

        let user_dir = scoped.path().join("users").join(tenant_id);
        let had_backup = std::fs::read_dir(&user_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().contains(".locked-"))
            })
            .unwrap_or(false);
        assert!(had_backup, "Expected .locked-* backup file in user_dir");
        Ok(())
    }

    #[test]
    fn test_sync_log_recording() -> anyhow::Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_synclog_user";
        let correct_key = "test_key_sync_999";

        struct SyncEntry {
            table_name: String,
            row_key: String,
            operation: String,
            data_json: Option<String>,
            seq_num: i64,
        }

        let results: Vec<SyncEntry> = {
            let db = TenantDB::open(tenant_id, correct_key)?;

            // 1. KV write
            db.set_kv("agent_persona", "Hello test persona")?;

            // 2. Gasto write
            db.add_expense(450.0, "Almuerzo de negocios", Some("comida"))?;

            // 3. Recordatorio write
            db.add_reminder("2026-06-01 12:00:00", "Reunión de alineación")?;

            // Verificar entradas en sync_log
            let mut stmt = db.connection.prepare(
                "SELECT table_name, row_key, operation, data_json, seq_num FROM sync_log ORDER BY seq_num ASC"
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(SyncEntry {
                    table_name: row.get(0)?,
                    row_key: row.get(1)?,
                    operation: row.get(2)?,
                    data_json: row.get(3)?,
                    seq_num: row.get(4)?,
                })
            })?;

            rows.filter_map(|r| r.ok()).collect()
        };

        // Debería haber exactamente 3 entradas
        assert_eq!(results.len(), 3);

        // Turno 1: kv_store
        assert_eq!(results[0].table_name, "kv_store");
        assert_eq!(results[0].row_key, "agent_persona");
        assert_eq!(results[0].operation, "UPDATE");
        assert!(results[0]
            .data_json
            .as_ref()
            .map(|s| s.contains("Hello test persona"))
            .unwrap_or(false));
        assert_eq!(results[0].seq_num, 1);

        // Turno 2: expenses
        assert_eq!(results[1].table_name, "expenses");
        assert_eq!(results[1].operation, "INSERT");
        assert_eq!(results[1].seq_num, 2);

        // Turno 3: reminders
        assert_eq!(results[2].table_name, "reminders");
        assert_eq!(results[2].operation, "INSERT");
        assert_eq!(results[2].seq_num, 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_tenant_purges_physical_directory() -> Result<()> {
        let _env = ScopedTestDataDir::new()?;
        use sha2::Digest;
        let master = MasterEnclave::new_in_memory().await?;

        let tenant_id = "test_purge_tenant";
        let (port, temp_pass) = master.create_tenant(tenant_id).await?;
        assert!(port > 0);

        let pass_hash = format!("{:x}", sha2::Sha256::digest(temp_pass.as_bytes()));
        let session_key = format!("sk_{}", pass_hash);

        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let tenant_dir = std::path::Path::new(&base_dir)
            .join("users")
            .join(tenant_id);

        {
            let tenant_db = TenantDB::open(tenant_id, &session_key)?;
            tenant_db.set_kv("test_key", "test_val")?;
            assert!(
                tenant_dir.exists(),
                "Tenant physical directory should exist after open"
            );
        }

        // Execute tenant deletion
        master.delete_tenant(tenant_id).await?;

        // Verify physical directory is completely purged
        assert!(
            !tenant_dir.exists(),
            "Tenant physical directory must be purged on delete"
        );

        Ok(())
    }

    #[test]
    fn test_core_375_normalize_expense_category() {
        assert_eq!(
            normalize_expense_category("otros", "compra en el súper"),
            "comida"
        );
        assert_eq!(normalize_expense_category("comida", "almuerzo"), "comida");
        assert_eq!(
            normalize_expense_category("", "15000 de nafta"),
            "transporte"
        );
        assert_eq!(
            normalize_expense_category("general", "viaje en uber al centro"),
            "transporte"
        );
        assert_eq!(
            normalize_expense_category("gastos", "pago de luz y gas"),
            "hogar"
        );
        assert_eq!(
            normalize_expense_category("", "remedios de la farmacia"),
            "salud"
        );
        assert_eq!(normalize_expense_category("ocio", "cine"), "ocio");
        assert_eq!(normalize_expense_category("", "algo raro"), "general");
    }

    #[test]
    fn test_core_375_personal_facts_and_expenses() -> Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_personal_user";
        let key = "test_key_personal_123";

        let db = TenantDB::open(tenant_id, key)?;

        // Facts
        assert!(db.add_personal_fact("preferencias", "Le gusta el café sin azúcar", "chat")?);
        // Duplicate should return false
        assert!(!db.add_personal_fact("preferencias", "le gusta el café sin azúcar", "chat")?);

        let facts = db.get_personal_facts(10)?;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0]["fact"], "Le gusta el café sin azúcar");

        let fact_id = facts[0]["id"].as_i64().unwrap_or(0);
        assert!(fact_id > 0);

        // Expenses
        db.add_expense(5000.0, "Supermercado Coto", Some("comida"))?;
        db.add_expense(3500.0, "Carga de SUBE", Some("transporte"))?;
        db.add_expense(2000.0, "Panadería", Some("comida"))?;

        let summary = db.query_expenses_summary(None)?;
        assert_eq!(summary["total"], 10500.0);
        assert_eq!(summary["totals_by_category"]["comida"], 7000.0);
        assert_eq!(summary["totals_by_category"]["transporte"], 3500.0);

        let filtered_summary = db.query_expenses_summary(Some("comida"))?;
        assert_eq!(filtered_summary["total"], 7000.0);

        // Reminders: add, list (with id + recurrence), update, complete (with empty-guard), delete, due-fire
        let r1 = db.add_reminder_with_recurrence("2026-11-10 10:00", "Dentista a las 10", None)?;
        let r2 = db.add_reminder_with_recurrence(
            "2026-11-20 17:00",
            "Clase refrigeración",
            Some("todos los viernes"),
        )?;
        let listed = db.get_reminders(10)?;
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0]["id"], r1);
        assert_eq!(listed[1]["id"], r2);
        assert_eq!(listed[1]["recurrence"], "weekly:fri");

        // Update by #id
        assert_eq!(
            db.update_reminder(&format!("#{}", r1), Some("2026-11-10 11:30"), None, None)?,
            1
        );
        // Empty string must never complete everything
        assert_eq!(db.complete_reminder("")?, 0);
        assert_eq!(db.complete_reminder("dentista")?, 1);
        assert_eq!(db.delete_reminder(&format!("#{}", r1))?, 1);
        assert_eq!(db.get_reminders(10)?.len(), 1);

        // Prompt block
        let block = db.build_personal_memory_prompt_block()?;
        assert!(block.contains("[MEMORIA A LARGO PLAZO DEL USUARIO]"));
        assert!(block.contains("café sin azúcar"));
        assert!(block.contains("[RESUMEN FINANCIERO Y RECORDATORIOS]"));
        assert!(block.contains("recurrente: weekly:fri"));

        // Due reminder firing: non-recurring completes, weekly:fri advances 7 days
        let _r_once = db.add_reminder_with_recurrence("2020-01-01 09:00", "Viejo único", None)?;
        let _r_past_rec = db.add_reminder_with_recurrence(
            "2026-11-06 17:00",
            "Clase semanal pasada",
            Some("weekly:fri"),
        )?;
        let fixed_now = chrono::DateTime::parse_from_rfc3339("2026-11-13T18:00:00-03:00")
            .unwrap()
            .with_timezone(&chrono::Local);
        let fired = db.fire_due_reminders(fixed_now)?;
        assert_eq!(fired.len(), 2);
        let after = db.get_reminders(10)?;
        let weekly = after
            .iter()
            .find(|r| r["description"] == "Clase semanal pasada")
            .unwrap();
        assert_eq!(weekly["status"], "pending");
        assert!(weekly["remind_at"].as_str().unwrap() > "2026-11-13 18:00");

        // Time normalization helper check
        let norm = normalize_remind_at("hoy 17hs", "todos los viernes", fixed_now);
        assert_eq!(norm, "2026-11-13 17:00");
        assert_eq!(normalize_recurrence("todos los viernes"), "weekly:fri");

        // Delete fact
        assert!(db.delete_personal_fact(fact_id)?);
        assert_eq!(db.get_personal_facts(10)?.len(), 0);

        Ok(())
    }

    #[test]
    fn test_personal_facts_topic_consolidation_and_update() -> Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "test_facts_consolidation";
        let db = TenantDB::open(tenant_id, "test_key_consolidation_123")?;

        // 1. Progressive updates of the same topic must consolidate into ONE note
        let out1 = db.upsert_personal_fact(
            None,
            "trabajo",
            "Es profesor de un curso de refrigeración",
            "chat",
        )?;
        assert!(matches!(out1, PersonalFactUpsertOutcome::Inserted { .. }));

        let out2 = db.upsert_personal_fact(
            None,
            "trabajo",
            "Es profesor de 2 cursos de refrigeración",
            "chat",
        )?;
        assert!(matches!(out2, PersonalFactUpsertOutcome::Updated { .. }));

        let out3 = db.upsert_personal_fact(
            None,
            "trabajo",
            "Es profesor de 3 cursos de refrigeración",
            "chat",
        )?;
        assert!(matches!(out3, PersonalFactUpsertOutcome::Updated { .. }));

        // 2. Distinct fact in the same category must stay separate
        let out_other = db.upsert_personal_fact(
            None,
            "trabajo",
            "Trabaja en su taller por la mañana",
            "chat",
        )?;
        assert!(matches!(
            out_other,
            PersonalFactUpsertOutcome::Inserted { .. }
        ));

        let facts = db.get_personal_facts(20)?;
        assert_eq!(facts.len(), 2);
        let prof_fact = facts
            .iter()
            .find(|f| f["fact"].as_str().unwrap_or("").contains("refrigeración"))
            .unwrap();
        assert_eq!(
            prof_fact["fact"],
            "Es profesor de 3 cursos de refrigeración"
        );

        // 3. Explicit update by ID modifies the existing note
        let prof_id = prof_fact["id"].as_i64().unwrap();
        assert!(db.update_personal_fact(
            prof_id,
            Some("trabajo"),
            "Dicta 4 cursos de refrigeración industrial",
            "user_ui",
        )?);
        let updated_facts = db.get_personal_facts(20)?;
        assert_eq!(updated_facts.len(), 2);
        assert!(updated_facts.iter().any(
            |f| f["id"] == prof_id && f["fact"] == "Dicta 4 cursos de refrigeración industrial"
        ));

        // 4. Prompt block includes [#id] tags so the assistant can reference existing notes
        let block = db.build_personal_memory_prompt_block()?;
        assert!(block.contains(&format!("[#{}]", prof_id)));

        Ok(())
    }

    #[test]
    fn test_write_secret_file_writes_content_and_unix_mode() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let secret_path = dir.path().join("nested").join("secret.key");
        let payload = b"super-secret-material";

        write_secret_file(&secret_path, payload)?;
        let read_back = std::fs::read(&secret_path)?;
        assert_eq!(read_back, payload);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&secret_path)?.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        Ok(())
    }

    #[test]
    fn test_tenant_db_pool_reuses_connection_and_evicts() -> Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "pool_reuse_user";
        let key = "pool_secret_key_123";
        let db_path = tenant_db_path(tenant_id);

        {
            let db = TenantDB::open(tenant_id, key)?;
            db.set_kv("greeting", "hola_aegis")?;
        }

        {
            let pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            assert!(pool.initialized_schemas.contains(&db_path));
            let idle_for_tenant = pool.idle.iter().filter(|e| e.key.0 == db_path).count();
            assert_eq!(idle_for_tenant, 1);
        }

        {
            let db = TenantDB::open(tenant_id, key)?;
            assert_eq!(db.get_kv("greeting")?.as_deref(), Some("hola_aegis"));
            // While checked out, the handle is not in the idle queue.
            let pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            let idle_for_tenant = pool.idle.iter().filter(|e| e.key.0 == db_path).count();
            assert_eq!(idle_for_tenant, 0);
        }

        TenantDB::evict_tenant(tenant_id);
        {
            let pool = TENANT_DB_POOL.lock().unwrap_or_else(|e| e.into_inner());
            assert!(!pool.initialized_schemas.contains(&db_path));
            let idle_for_tenant = pool.idle.iter().filter(|e| e.key.0 == db_path).count();
            assert_eq!(idle_for_tenant, 0);
        }
        Ok(())
    }

    #[test]
    fn test_tenant_db_pool_lru_capacity_evicts_oldest() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let mut pool = TenantDbPool::default();

        let p1 = dir.path().join("t1.db");
        let p2 = dir.path().join("t2.db");
        let p3 = dir.path().join("t3.db");
        for p in [&p1, &p2, &p3] {
            std::fs::write(p, b"")?;
        }

        let k1 = (p1.to_string_lossy().into_owned(), "k1".to_string());
        let k2 = (p2.to_string_lossy().into_owned(), "k2".to_string());
        let k3 = (p3.to_string_lossy().into_owned(), "k3".to_string());

        pool.put(k1.clone(), Connection::open_in_memory()?, 0, 2);
        pool.put(k2.clone(), Connection::open_in_memory()?, 0, 2);
        // Touch k1 so k2 becomes the least-recently-used entry.
        let (reused, gen) = pool
            .take(&k1, Instant::now())
            .ok_or_else(|| anyhow::anyhow!("k1 should be present"))?;
        pool.put(k1.clone(), reused, gen, 2);

        // Adding k3 with capacity=2 must evict k2 and retain k1 + k3.
        pool.put(k3.clone(), Connection::open_in_memory()?, 0, 2);
        assert_eq!(pool.idle.len(), 2);
        assert!(pool.take(&k2, Instant::now()).is_none());
        assert!(pool.take(&k1, Instant::now()).is_some());
        assert!(pool.take(&k3, Instant::now()).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn test_tenant_db_with_async_concurrent() -> Result<()> {
        let _scoped = ScopedTestDataDir::new()?;
        let tenant_id = "async_concurrent_user";
        let key = "async_secret_key_999";

        TenantDB::with_async(tenant_id.to_string(), key.to_string(), |db| {
            db.set_kv("seed", "ok")
        })
        .await?;

        let mut tasks = Vec::new();
        for i in 0..8u32 {
            let tid = tenant_id.to_string();
            let k = key.to_string();
            tasks.push(tokio::spawn(async move {
                TenantDB::with_async(tid, k, move |db| {
                    db.set_kv(&format!("k_{i}"), &format!("v_{i}"))?;
                    db.get_kv("seed")
                })
                .await
            }));
        }

        for t in tasks {
            let val = t.await??;
            assert_eq!(val.as_deref(), Some("ok"));
        }

        let count: i64 = TenantDB::with_async(tenant_id.to_string(), key.to_string(), |db| {
            let c = db.connection().query_row(
                "SELECT count(*) FROM kv_store WHERE key LIKE 'k_%'",
                [],
                |row| row.get(0),
            )?;
            Ok(c)
        })
        .await?;
        assert_eq!(count, 8);
        Ok(())
    }
}
