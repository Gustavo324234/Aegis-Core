use crate::transport::{JsonRpcMessage, McpTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::Stream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Tamaño máximo por línea JSON-RPC leída desde STDOUT del proceso hijo (4 MiB).
const MAX_STDIO_LINE_BYTES: usize = 4 * 1024 * 1024;

/// Variables de entorno mínimas no sensibles del sistema operativo requeridas para que
/// subprocesos (Node/NPX, Python, Winsock en Windows, certificados TLS) arranquen sin heredar
/// secretos `AEGIS_*` del Ring 0.
const SAFE_HOST_ENV_KEYS: &[&str] = &[
    "PATH",
    "PATHEXT",
    "SystemRoot",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "LANG",
    "LC_ALL",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Lee una línea terminada en `\n` acotando el uso de memoria a `max_bytes`.
/// Si una línea excede `max_bytes`, descarta el resto hasta `\n` y retorna `Ok(Some(None))`.
async fn read_bounded_line<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> std::io::Result<Option<Option<String>>> {
    let mut buf = Vec::new();
    let mut overflowed = false;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if buf.is_empty() && !overflowed {
                return Ok(None);
            }
            if overflowed {
                return Ok(Some(None));
            }
            let text = String::from_utf8_lossy(&buf).into_owned();
            return Ok(Some(Some(text)));
        }

        if let Some(newline_idx) = available.iter().position(|&b| b == b'\n') {
            if !overflowed {
                let slice = &available[..newline_idx];
                if buf.len().saturating_add(slice.len()) <= max_bytes {
                    buf.extend_from_slice(slice);
                } else {
                    overflowed = true;
                    buf.clear();
                }
            }
            reader.consume(newline_idx + 1);
            if overflowed {
                return Ok(Some(None));
            }
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            let text = String::from_utf8_lossy(&buf).into_owned();
            return Ok(Some(Some(text)));
        } else {
            let chunk_len = available.len();
            if !overflowed {
                if buf.len().saturating_add(chunk_len) <= max_bytes {
                    buf.extend_from_slice(available);
                } else {
                    overflowed = true;
                    buf.clear();
                }
            }
            reader.consume(chunk_len);
        }
    }
}

/// Implementación de transporte MCP vía StdIO.
/// Diseñado bajo principios SRE: Zero-Trust Isolation y Gestión de Ciclo de Vida forzada.
pub struct StdioTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<Option<ChildStdout>>>,
    child: Arc<Mutex<Child>>,
}

impl StdioTransport {
    /// Crea un nuevo transporte MCP iniciando un proceso hijo.
    ///
    /// # Jailing & Security
    /// - `env_clear()` + whitelist de runtime OS: Bloquea fugas de `AEGIS_ROOT_KEY` / API keys
    ///   sin romper Winsock/Node/Python en Windows y Linux.
    /// - `current_dir()`: Confinamiento físico en el directorio indicado.
    /// - `kill_on_drop(true)`: Garantía adicional contra procesos zombi.
    pub fn new(
        command: &str,
        args: Vec<String>,
        envs: HashMap<String, String>,
        cwd: PathBuf,
    ) -> Result<Self> {
        debug!(
            "Iniciando Servidor MCP vía StdIO: {} {:?} en {}",
            command,
            args,
            cwd.display()
        );

        let mut cmd = Command::new(command);

        // Protocolo Citadel: Zero-Trust Isolation (Regla 1)
        cmd.args(args)
            .env_clear()
            .kill_on_drop(true)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        for key in SAFE_HOST_ENV_KEYS {
            if let Some(val) = std::env::var_os(key) {
                cmd.env(key, val);
            }
        }
        cmd.envs(envs);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Error fatal al spawnear servidor MCP: {}", command))?;

        let stdin = child
            .stdin
            .take()
            .context("Error de hardware: No se pudo capturar STDIN del proceso hijo")?;
        let stdout = child
            .stdout
            .take()
            .context("Error de hardware: No se pudo capturar STDOUT del proceso hijo")?;

        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(Some(stdout))),
            child: Arc::new(Mutex::new(child)),
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    /// Serializa el mensaje, añade el delimitador \n y lo envía al stdin del hijo. (Regla 2)
    async fn send_message(&self, msg: JsonRpcMessage) -> Result<()> {
        let mut json = serde_json::to_string(&msg).context("Error de serialización MCP")?;
        json.push('\n');

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(json.as_bytes())
            .await
            .context("Error IO: Broken Pipe al escribir en STDIN del servidor MCP")?;

        stdin
            .flush()
            .await
            .context("Error IO: Fallo al vaciar el buffer de STDIN")?;

        Ok(())
    }

    /// Captura el STDOUT y lo convierte en un Stream asíncrono de mensajes con límite de memoria. (Regla 2)
    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<JsonRpcMessage>> + Send>> {
        let stdout_mutex = self.stdout.clone();

        let stream = async_stream::try_stream! {
            let stdout = stdout_mutex.lock().await.take()
                .context("Error de estado: El flujo de STDOUT ya ha sido reclamado")?;

            let mut reader = BufReader::new(stdout);

            while let Some(maybe_line) = read_bounded_line(&mut reader, MAX_STDIO_LINE_BYTES)
                .await
                .context("Error de lectura en STDOUT")?
            {
                let Some(line) = maybe_line else {
                    warn!(
                        "ANK-MCP: Línea STDOUT descartada por exceder el límite de {} bytes",
                        MAX_STDIO_LINE_BYTES
                    );
                    continue;
                };

                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<JsonRpcMessage>(&line) {
                    Ok(msg) => yield msg,
                    Err(e) => {
                        error!("Mensaje MCP malformado recibido del servidor: {}", e);
                    }
                }
            }
        };

        Box::pin(stream)
    }
}

/// Zombie Killer: Asegura que el proceso hijo muera si el transporte es dropeado. (Regla 3)
impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.try_lock() {
            match child.start_kill() {
                Ok(_) => debug!("Proceso servidor MCP terminado exitosamente (Drop)"),
                Err(e) => {
                    debug!("Nota: Intento de terminación de proceso hijo: {}", e);
                }
            }
        } else {
            warn!("Fuga potencial de proceso hijo: No se pudo adquirir lock del Child en Drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use std::env;

    #[tokio::test]
    async fn test_stdio_transport_e2e() -> Result<()> {
        let (cmd, args) = if cfg!(windows) {
            (
                "python",
                vec![
                    "-u".into(),
                    "-c".into(),
                    "import sys; [print(line.strip(), flush=True) for line in sys.stdin]".into(),
                ],
            )
        } else {
            ("cat", vec![])
        };

        let cwd = env::current_dir()?;
        let transport = StdioTransport::new(cmd, args, HashMap::new(), cwd)?;

        let req = JsonRpcMessage::Request {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(42),
            method: "ping".to_string(),
            params: None,
        };

        let mut stream = transport.receive_messages();
        transport.send_message(req).await?;

        if let Some(res) = stream.next().await {
            let msg = res?;
            if let JsonRpcMessage::Request { id, method, .. } = msg {
                assert_eq!(id, serde_json::json!(42));
                assert_eq!(method, "ping");
            } else {
                anyhow::bail!("Tipo de mensaje recibido incorrecto: {:?}", msg);
            }
        } else {
            anyhow::bail!("Stream cerrado prematuramente");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_bounded_line_discards_oversized_and_recovers() -> Result<()> {
        let payload = b"0123456789ABCDEF\nok\n";
        let mut reader = BufReader::new(&payload[..]);

        // Max 8 bytes: first line (16 bytes) overflows -> Some(None), second line ("ok") succeeds!
        let first = read_bounded_line(&mut reader, 8).await?;
        assert_eq!(first, Some(None));

        let second = read_bounded_line(&mut reader, 8).await?;
        assert_eq!(second, Some(Some("ok".to_string())));

        let eof = read_bounded_line(&mut reader, 8).await?;
        assert_eq!(eof, None);
        Ok(())
    }
}
