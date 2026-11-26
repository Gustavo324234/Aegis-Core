# CORE-168 — feat: TerminalExecutor — Ejecución de Comandos con Streaming

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-167 (para leer allowlist y project_root)
**Estado:** ✅ Completo

---

## Contexto

Los agentes de Epic 43 necesitan ejecutar comandos de sistema para verificar su trabajo (`cargo build`, `npm run build`, `git status`, etc.) y para implementar cambios reales de código. El `TerminalExecutor` es el componente que gestiona esa ejecución de forma segura.

El output de cada comando se streama línea por línea via un canal Tokio para que el WebSocket pueda enviarlo al tenant en tiempo real (evento `terminal_output`, CORE-175).

---

## Cambios Requeridos

### Crear `ank-core/src/executor/mod.rs` y `ank-core/src/executor/terminal.rs`

```rust
// executor/mod.rs
pub mod terminal;
pub use terminal::{TerminalExecutor, TerminalLine, LineKind};
```

```rust
// executor/terminal.rs

#[derive(Debug, Clone, Serialize)]
pub enum LineKind {
    Stdout,
    Stderr,
    System,   // mensajes del propio Aegis (ej: "Command timed out")
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalLine {
    pub kind: LineKind,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct TerminalExecutor {
    project_root: PathBuf,
    allowed_commands: Vec<String>,
    timeout_secs: u64,
}

impl TerminalExecutor {
    pub fn new(project_root: PathBuf, allowed_commands: Vec<String>) -> Self {
        Self {
            project_root,
            allowed_commands,
            timeout_secs: 300,  // 5 minutos máximo
        }
    }

    /// Ejecuta el comando y streama output via `tx`.
    /// Retorna el exit code al completar.
    /// Falla inmediatamente si el comando no está en la allowlist.
    pub async fn exec(
        &self,
        command: &str,
        args: &[&str],
        tx: tokio::sync::mpsc::Sender<TerminalLine>,
    ) -> anyhow::Result<i32> {
        self.validate_command(command)?;
        
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .current_dir(&self.project_root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Leer stdout y stderr en tareas separadas, enviando cada línea via tx
        // Usar tokio::io::BufReader + lines() para leer línea por línea
        // Usar tokio::time::timeout para el timeout global
        // ...
        
        let exit_code = child.wait().await?.code().unwrap_or(-1);
        Ok(exit_code)
    }

    /// Valida que el primer token del comando (el binario) esté en la allowlist.
    fn validate_command(&self, command: &str) -> anyhow::Result<()> {
        let binary = command.split_whitespace().next().unwrap_or("");
        if self.allowed_commands.iter().any(|a| a == binary) {
            Ok(())
        } else {
            anyhow::bail!("Command '{}' not in terminal allowlist", binary)
        }
    }
}
```

### Reglas de seguridad a implementar en `validate_command`

Además de la allowlist, rechazar explícitamente patrones peligrosos:
- Cualquier comando que contenga `&&`, `||`, `;`, `|` (previene chaining)
- Cualquier argumento que contenga `../` (path traversal)
- Argumentos que contengan `$(` o backtick (previene command substitution)
- Si el binary es `git`, los subcomandos permitidos son: `status`, `log`, `diff`, `branch`, `checkout`, `add`, `commit`, `push`, `pull`, `fetch`. No `git config --global`, no `git remote set-url`.

### Agregar al `lib.rs` de `ank-core`

```rust
pub mod executor;
```

---

## Criterios de Aceptación

- [x] `TerminalExecutor::exec("cargo", &["build"], tx)` funciona y streama las líneas de output
- [x] `TerminalExecutor::exec("rm", &["-rf", "/"], tx)` retorna `Err` inmediatamente (no en allowlist)
- [x] Comandos con `&&` o `;` en los args son rechazados
- [x] Path traversal `../` en args es rechazado
- [x] El timeout de 300 segundos cancela el proceso y envía una `TerminalLine` de tipo `System`
- [x] Stdout y Stderr se leen concurrentemente (no primero uno, luego el otro)
- [x] Cuando el proceso termina, el canal `tx` se cierra limpiamente
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*