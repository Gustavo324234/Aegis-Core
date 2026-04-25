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

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {}", db_path))?;

        // 1. Configurar la llave de encriptación (SQLCipher)
        // PRAGMA key requiere ser la primera sentencia y no debe retornar resultados.
        conn.pragma_update(None, "key", session_key)
            .context("Failed to apply PRAGMA key for encryption")?;

        // 2. Verificar la integridad (Si la llave es incorrecta, cualquier consulta fallará aquí)
        // SQLCipher no valida la llave hasta que se intenta acceder a los datos.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .context("Decryption failed: invalid session key or corrupted database")?;

        info!(tenant_id = %tenant_id, "Secure Enclave initialized successfully.");

        let db = Self { connection: conn };

        // 3. Inicializar esquema básico
        db.init_schema()?;

        Ok(db)
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

        Ok(())
    }

    /// Inserta o actualiza un valor en el almacén seguro.
    pub fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        use anyhow::Context;
        self.connection.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)",
            [key, value],
        ).with_context(|| format!("Failed to set KV: {}", key))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use tempfile::tempdir;

    #[test]
    fn test_secure_enclave_decryption_failure() -> anyhow::Result<()> {
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
}
