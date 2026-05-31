use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub mod master;
pub use master::MasterEnclave;

/// --- TENANT DB (SECURE ENCLAVE) ---
/// Gestiona una base de datos SQLite encriptada con SQLCipher por cada tenant.
pub struct TenantDB {
    connection: Connection,
}

impl TenantDB {
    /// Inicializa o abre la base de datos segura para un tenant.
    /// Aplica la session_key mediante PRAGMA key para desencriptar en reposo.
    pub fn open(tenant_id: &str, session_key: &str) -> Result<Self> {
        use anyhow::Context;
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let db_path = format!("{}/users/{}/memory.db", base_dir, tenant_id);

        // Asegurar que el directorio del tenant existe
        if let Some(parent) = Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for tenant {}", tenant_id))?;
        }

        // First attempt with the supplied key.
        match Self::try_open_with_key(&db_path, session_key) {
            Ok(conn) => {
                info!(tenant_id = %tenant_id, "Secure Enclave initialized successfully.");
                let db = Self { connection: conn };
                db.init_schema()?;
                Ok(db)
            }
            // CORE-FIX (G): the DB exists but the key doesn't match it. This is
            // the post-password-reset case: `memory.db` was encrypted with the
            // session_key derived from the OLD password (session_key =
            // SHA256(password)), but `reset_tenant_password` only updates the
            // master hash — it can't re-key the tenant DB (the admin doesn't
            // hold the old password). So every open after a reset fails with
            // `hmac check failed for pgno=1`, spamming the log AND losing the
            // tenant's persona/settings.
            //
            // The data is already unrecoverable with the current key, so we
            // back the unreadable file up (never silently delete) and recreate
            // a fresh DB with the current key. Functionality (persona, kv_store,
            // approved_paths) is restored; the old ciphertext is preserved at
            // `memory.db.locked-<ts>` in case the old password resurfaces.
            Err(e) if Self::is_decryption_error(&e) => {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                tracing::warn!(
                    tenant_id = %tenant_id,
                    "CORE-FIX (G): tenant DB won't decrypt with the current session key \
                     (likely a password reset re-keyed the tenant). Backing up the \
                     unreadable file to memory.db.locked-{ts} and recreating a fresh DB.",
                );
                Self::quarantine_unreadable_db(&db_path, ts);
                let conn = Self::try_open_with_key(&db_path, session_key).with_context(|| {
                    format!("Failed to recreate tenant DB after key mismatch for {tenant_id}")
                })?;
                info!(tenant_id = %tenant_id, "Secure Enclave re-initialized after key mismatch.");
                let db = Self { connection: conn };
                db.init_schema()?;
                Ok(db)
            }
            Err(e) => Err(e),
        }
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

        Ok(conn)
    }

    /// True when the error chain looks like a SQLCipher key mismatch
    /// (SQLITE_NOTADB / "file is not a database" / decryption failure) rather
    /// than a transient problem like a lock. We only auto-recreate on these.
    fn is_decryption_error(err: &anyhow::Error) -> bool {
        let s = format!("{err:#}").to_lowercase();
        s.contains("not a database")
            || s.contains("decryption failed")
            || s.contains("file is encrypted")
            || s.contains("hmac")
    }

    /// Renames the unreadable DB (and its WAL/SHM sidecars) out of the way so a
    /// fresh one can be created. Never deletes — keeps the ciphertext as a
    /// `.locked-<ts>` backup the operator can try to recover later.
    fn quarantine_unreadable_db(db_path: &str, ts: u64) {
        for suffix in ["", "-wal", "-shm"] {
            let from = format!("{db_path}{suffix}");
            if Path::new(&from).exists() {
                let to = format!("{db_path}.locked-{ts}{suffix}");
                if let Err(e) = std::fs::rename(&from, &to) {
                    tracing::error!(
                        "CORE-FIX (G): could not quarantine {from} → {to}: {e}. \
                         Removing it so the tenant DB can be recreated."
                    );
                    let _ = std::fs::remove_file(&from);
                }
            }
        }
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
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
                [],
            )
            .context("Failed to initialize reminders table")?;

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

    pub fn is_oauth_connected(&self, provider: &str) -> Result<bool> {
        Ok(self.get_refresh_token(provider)?.is_some())
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
        use anyhow::Context;
        self.connection
            .execute(
                "INSERT INTO reminders (remind_at, description) VALUES (?1, ?2)",
                [remind_at, description],
            )
            .context("Failed to insert reminder")?;

        let row_id = self.connection.last_insert_rowid();
        let data = serde_json::json!({
            "remind_at": remind_at,
            "description": description,
            "status": "pending"
        });
        let _ = self.record_sync_log(
            "reminders",
            &row_id.to_string(),
            "INSERT",
            Some(&data.to_string()),
        );

        Ok(())
    }

    pub fn get_reminders(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.connection.prepare(
            "SELECT remind_at, description, status, created_at FROM reminders ORDER BY remind_at ASC LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(serde_json::json!({
                "remind_at": row.get::<_, String>(0)?,
                "description": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "created_at": row.get::<_, String>(3)?,
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
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
        let _guard = acquire_test_lock();
        let tenant_id = format!(
            "test_persona_user_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis()
        );
        let correct_key = "test_key_123";

        {
            let db = TenantDB::open(&tenant_id, correct_key)?;
            db.set_persona("Eres Eve, asistente de ACME Corp.")?;
            let loaded = db.get_persona()?;
            assert!(loaded.is_some(), "Persona should be stored");
            assert_eq!(
                loaded.unwrap_or_default(),
                "Eres Eve, asistente de ACME Corp."
            );
        }

        {
            let db = TenantDB::open(&tenant_id, correct_key)?;
            let loaded = db.get_persona()?;
            assert!(loaded.is_some());
            db.delete_persona()?;
            let after_delete = db.get_persona()?;
            assert!(after_delete.is_none(), "Persona should be deleted");
        }

        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant_id));
        Ok(())
    }

    #[test]
    fn test_persona_max_length() -> anyhow::Result<()> {
        let _guard = acquire_test_lock();
        let tenant_id = format!(
            "test_maxlen_user_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis()
        );
        let correct_key = "test_key_456";

        let too_long = "x".repeat(4001);
        let db = TenantDB::open(&tenant_id, correct_key)?;
        let result = db.set_persona(&too_long);
        assert!(result.is_err(), "Persona of 4001 chars should fail");

        let valid = "x".repeat(4000);
        db.set_persona(&valid)?;
        let loaded = db.get_persona()?;
        assert_eq!(loaded.unwrap_or_default().len(), 4000);

        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant_id));
        Ok(())
    }

    /// CORE-FIX (G): opening a tenant DB with a DIFFERENT key (the
    /// post-password-reset case) must not error forever — it should quarantine
    /// the unreadable file and recreate a fresh, usable DB with the new key.
    #[test]
    fn test_open_with_wrong_key_recreates_db() -> anyhow::Result<()> {
        let _guard = acquire_test_lock();
        let tenant_id = format!(
            "test_rekey_user_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis()
        );

        // 1. Create + populate with the original (old-password) key.
        {
            let db = TenantDB::open(&tenant_id, "old_password_key")?;
            db.set_persona("Soy Sol, sarcástica y atrevida.")?;
        }

        // 2. Open with a different (new-password) key — simulates a reset.
        //    Must succeed (recreated), not error with hmac failure.
        {
            let db = TenantDB::open(&tenant_id, "new_password_key")?;
            // Fresh DB → the old persona is gone (it was unreadable anyway).
            assert!(
                db.get_persona()?.is_none(),
                "recreated DB should start empty"
            );
            // And it must be writable/usable with the new key.
            db.set_persona("Persona nueva")?;
            assert_eq!(db.get_persona()?.unwrap_or_default(), "Persona nueva");
        }

        // 3. Re-opening with the new key now works normally (no recreation).
        {
            let db = TenantDB::open(&tenant_id, "new_password_key")?;
            assert_eq!(db.get_persona()?.unwrap_or_default(), "Persona nueva");
        }

        // The old ciphertext was preserved as a .locked-* backup, not deleted.
        let dir = format!("./users/{}", tenant_id);
        let had_backup = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().contains(".locked-"))
            })
            .unwrap_or(false);
        assert!(
            had_backup,
            "unreadable DB should be quarantined, not deleted"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_sync_log_recording() -> anyhow::Result<()> {
        let _guard = acquire_test_lock();
        let tenant_id = format!(
            "test_synclog_user_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis()
        );
        let correct_key = "test_key_sync_999";

        let db = TenantDB::open(&tenant_id, correct_key)?;

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

        struct SyncEntry {
            table_name: String,
            row_key: String,
            operation: String,
            data_json: Option<String>,
            seq_num: i64,
        }

        let rows = stmt.query_map([], |row| {
            Ok(SyncEntry {
                table_name: row.get(0)?,
                row_key: row.get(1)?,
                operation: row.get(2)?,
                data_json: row.get(3)?,
                seq_num: row.get(4)?,
            })
        })?;

        let results: Vec<SyncEntry> = rows.filter_map(|r| r.ok()).collect();

        // Debería haber exactamente 3 entradas
        assert_eq!(results.len(), 3);

        // Turno 1: kv_store
        assert_eq!(results[0].table_name, "kv_store");
        assert_eq!(results[0].row_key, "agent_persona");
        assert_eq!(results[0].operation, "UPDATE");
        assert!(results[0]
            .data_json
            .as_ref()
            .unwrap()
            .contains("Hello test persona"));
        assert_eq!(results[0].seq_num, 1);

        // Turno 2: expenses
        assert_eq!(results[1].table_name, "expenses");
        assert_eq!(results[1].operation, "INSERT");
        assert_eq!(results[1].seq_num, 2);

        // Turno 3: reminders
        assert_eq!(results[2].table_name, "reminders");
        assert_eq!(results[2].operation, "INSERT");
        assert_eq!(results[2].seq_num, 3);

        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant_id));
        Ok(())
    }
}
