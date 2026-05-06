# BRIEFING — Kernel Engineer
## CORE-266: ank-server Windows Service Control Manager handshake

**Fecha:** 2026-05-06
**Prioridad:** CRITICAL — bloquea instalación en Windows como servicio
**Rama sugerida:** `fix/core-266-windows-service-handshake`

---

## Contexto

`ank-server` arranca perfectamente cuando se ejecuta directo en Windows:

```
INFO ank_server: Aegis serving HTTP on port 8000
INFO ank_http: Aegis HTTP server listening on http://0.0.0.0:8000
```

Pero falla como servicio de Windows con timeout `%%1053`:

```
Se agotó el tiempo de espera (30000 ms) para la conexión con el servicio AegisOS.
```

**Causa raíz:** el SCM (Service Control Manager) de Windows requiere que el
proceso llame a `SetServiceStatus(SERVICE_RUNNING)` dentro del timeout.
`ank-server` no hace ese handshake — es un binario estándar.

**Approach elegido:** flag `--service`. Es el más simple, predecible y testeable.
Al registrar el servicio en el installer se pasa `--service`, y en ejecución
directa (dev/terminal) el flag no está presente → comportamiento idéntico al actual.

---

## Cambios requeridos — 3 archivos

### 1. `Cargo.toml` (workspace) — agregar dependencia condicional

```toml
[target.'cfg(windows)'.dependencies]
windows-service = "0.7"
```

### 2. `kernel/crates/ank-server/Cargo.toml` — referenciar workspace dep

```toml
[target.'cfg(windows)'.dependencies]
windows-service = { workspace = true }
```

### 3. `kernel/crates/ank-server/src/main.rs` — implementar handshake

**3a.** Refactorizar la lógica del servidor en `async fn run_server() -> Result<()>`.
Todo el código de inicialización actual de `main()` (tracing, DB, scheduler,
HTTP server, tunnel) va dentro de `run_server()`.

**3b.** Agregar módulo Windows al final del archivo:

```rust
#[cfg(windows)]
mod windows_service_impl {
    use super::*;
    use std::time::Duration;
    use windows_service::{
        define_windows_service, service_dispatcher,
        service_control_handler::{self, ServiceControlHandlerResult},
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode,
            ServiceState, ServiceStatus, ServiceType,
        },
    };

    define_windows_service!(ffi_service_main, service_main);

    pub fn run() -> Result<()> {
        service_dispatcher::start("AegisOS", ffi_service_main)
            .map_err(|e| anyhow::anyhow!("SCM dispatcher error: {}", e))
    }

    fn service_main(_args: Vec<std::ffi::OsString>) {
        if let Err(e) = run_service() {
            eprintln!("[ERROR] Windows service failed: {}", e);
            std::process::exit(1);
        }
    }

    fn run_service() -> Result<()> {
        let event_handler = |control_event| match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                std::process::exit(0);
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        };

        let status_handle =
            service_control_handler::register("AegisOS", event_handler)?;

        // Notificar SCM: iniciando
        status_handle.set_service_status(ServiceStatus {
            service_type:     ServiceType::OWN_PROCESS,
            current_state:    ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code:        ServiceExitCode::Win32(0),
            checkpoint:       0,
            wait_hint:        Duration::from_secs(30),
            process_id:       None,
        })?;

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // Notificar SCM: corriendo
            status_handle.set_service_status(ServiceStatus {
                service_type:     ServiceType::OWN_PROCESS,
                current_state:    ServiceState::Running,
                controls_accepted: ServiceControlAccept::STOP,
                exit_code:        ServiceExitCode::Win32(0),
                checkpoint:       0,
                wait_hint:        Duration::from_secs(0),
                process_id:       None,
            })?;

            run_server().await
        })
    }
}
```

**3c.** Modificar `main()` para detectar el flag `--service`:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    load_env_file();

    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("Aegis Core v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    #[cfg(windows)]
    if args.contains(&"--service".to_string()) {
        return windows_service_impl::run();
    }

    run_server().await
}
```

**3d.** Actualizar `install.ps1` — pasar `--service` al registrar el binario:

En la función `Install-AegisService`, cambiar:
```powershell
# Antes:
$binPath = "`"$InstallDir\$BIN_NAME`""

# Después:
$binPath = "`"$InstallDir\$BIN_NAME`" --service"
```

---

## Verification

```bash
# Compilar (cross-compile desde Linux o build nativo en Windows)
cargo build --release -p ank-server --target x86_64-pc-windows-msvc

# En Windows después del build:
Start-Service AegisOS   # debe iniciar sin timeout
Stop-Service AegisOS    # debe detenerse limpiamente
& "C:\Program Files\Aegis\ank-server.exe"  # debe funcionar igual que antes
```

---

## Commit esperado

```
fix(ank-server): CORE-266 implement Windows SCM handshake via --service flag
```
