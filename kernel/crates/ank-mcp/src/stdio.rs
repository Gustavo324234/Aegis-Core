use crate::transport::{JsonRpcMessage, McpTransport};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

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
    /// - `env_clear()`: Elimina todas las variables de entorno heredadas del Ring 0.
    /// - `current_dir()`: Confinamiento físico en el workspace del tenant.
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
            .env_clear() // El subproceso nace ciego. No hereda la AEGIS_ROOT_KEY.
            .envs(envs) // Inyección explícita de secretos necesarios.
            .current_dir(cwd) // Jailing a nivel de directorio de trabajo.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Permitimos ver errores del servidor en el log del kernel.

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

    /// Captura el STDOUT y lo convierte en un Stream asíncrono de mensajes. (Regla 2)
    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<JsonRpcMessage>> + Send>> {
        let stdout_mutex = self.stdout.clone();

        let stream = async_stream::try_stream! {
            let stdout = stdout_mutex.lock().await.take()
                .context("Error de estado: El flujo de STDOUT ya ha sido reclamado")?;

            let mut reader = BufReader::new(stdout).lines();

            while let Some(line) = reader.next_line().await.context("Error de lectura en STDOUT")? {
                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<JsonRpcMessage>(&line) {
                    Ok(msg) => yield msg,
                    Err(e) => {
                        error!("Mensaje MCP malformado recibido del servidor: {}", e);
                        // No cortamos el stream por un error de parsing, pero lo reportamos
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
        // Como estamos en Drop, no podemos usar await. start_kill() envía el SIGKILL/Terminate de forma inmediata.
        if let Ok(mut child) = self.child.try_lock() {
            match child.start_kill() {
                Ok(_) => debug!("Proceso servidor MCP terminado exitosamente (Drop)"),
                Err(e) => {
                    // Si ya murió (es lo normal si cerró el stream), start_kill puede fallar con "NotConnected" o similar.
                    // Solo logueamos si es relevante.
                    debug!("Nota: Intento de terminación de proceso hijo: {}", e);
                }
            }
        } else {
            // Este caso es raro pero posible si hay un lock eterno bloqueando el guard de Child.
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
        // Usamos una utilidad de sistema que haga echo de la entrada.
        // En Windows usamos python o powershell, en Unix/Docker 'cat'.
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

        // Escuchamos en una tarea separada
        let mut stream = transport.receive_messages();

        // Enviamos
        transport.send_message(req.clone()).await?;

        // Validamos recepción
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
}
