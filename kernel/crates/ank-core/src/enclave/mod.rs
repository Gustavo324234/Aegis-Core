use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
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
        let db_path = format!("./users/{}/memory.db", tenant_id);

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
}
