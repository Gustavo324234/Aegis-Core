# CORE-266 — ank-server: Windows Service Control Manager handshake
**Estado:** ✅ Done
**Tipo:** fix
**Prioridad:** Crítica
**Asignado a:** Kernel Engineer

## Context

`ank-server` arranca correctamente en Windows cuando se ejecuta directamente,
pero falla como servicio de Windows con error `%%1053` (timeout):

```
Se agotó el tiempo de espera (30000 ms) para la conexión con el servicio AegisOS.
```

El SCM (Service Control Manager) de Windows requiere que cualquier proceso
registrado como servicio llame a `SetServiceStatus(SERVICE_RUNNING)` dentro
del timeout configurado (por defecto 30s). `ank-server` no implementa el
protocolo de servicios de Windows — es un binario estándar que no hace ese
handshake.

El binario inicia correctamente (HTTP en :8000, DB, scheduler, agentes) pero
el SCM lo mata porque nunca recibe la confirmación de que el servicio está listo.

## Required changes

Agregar soporte nativo para el protocolo de servicios de Windows en `ank-server`.

### 1. Agregar dependencia al workspace

En `Cargo.toml` (workspace):
```toml
[target.'cfg(windows)'.dependencies]
windows-service = "0.7"
```

En `kernel/crates/ank-server/Cargo.toml`:
```toml
[target.'cfg(windows)'.dependencies]
windows-service = { workspace = true }
```

### 2. Modificar `ank-server/src/main.rs`

Detectar si el proceso está corriendo como servicio de Windows y si es así,
ejecutar el loop principal dentro del contexto del SCM.

El patrón estándar con `windows-service`:

```rust
#[cfg(windows)]
fn run_as_windows_service() -> Result<()> {
    use windows_service::{
        define_windows_service,
        service_dispatcher,
        service_control_handler::{self, ServiceControlHandlerResult},
        service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    };
    use std::time::Duration;

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<std::ffi::OsString>) {
        let event_handler = move |control_event| match control_event {
            ServiceControl::Stop => {
                // Señalizar al runtime de tokio que se detenga
                std::process::exit(0);
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        };

        let status_handle = service_control_handler::register("AegisOS", event_handler)
            .expect("Failed to register service control handler");

        // Notificar al SCM que el servicio está iniciando
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(30),
            process_id: None,
        }).expect("Failed to set StartPending status");

        // Inicializar el runtime de tokio y lanzar el servidor
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            // Notificar al SCM que el servicio está corriendo
            status_handle.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Running,
                controls_accepted: ServiceControlAccept::STOP,
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::from_secs(0),
                process_id: None,
            }).expect("Failed to set Running status");

            // Aquí va el mismo código de inicialización que el main() actual
            // (tracing, DB, scheduler, HTTP server, etc.)
            run_server().await.expect("Server error");
        });
    }

    service_dispatcher::start("AegisOS", ffi_service_main)
        .map_err(|e| anyhow::anyhow!("Service dispatcher error: {}", e))
}
```

El `main()` actual debe detectar si está corriendo como servicio:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    load_env_file();

    // En Windows: intentar correr como servicio primero.
    // Si falla (porque se ejecutó directamente), correr normalmente.
    #[cfg(windows)]
    {
        // is_running_as_service() detecta si el padre es el SCM
        if is_running_as_windows_service() {
            return run_as_windows_service();
        }
    }

    // Ejecución directa (terminal, dev)
    run_server().await
}
```

La lógica del servidor (tracing, DB, scheduler, HTTP) debe refactorizarse en
una función `run_server()` para ser llamable desde ambos contextos.

### 3. Detectar si corre como servicio

```rust
#[cfg(windows)]
fn is_running_as_windows_service() -> bool {
    use std::ffi::OsString;
    // El SCM pasa argumentos especiales al servicio; alternativamente,
    // intentar registrar el handler y ver si el SCM responde
    windows_service::service_dispatcher::start("AegisOS", |_| {})
        .is_ok()
}
```

O más simple: pasar un flag `--service` al registrar el servicio en el installer:

```powershell
# En install.ps1, al registrar el servicio:
$binPath = "`"$InstallDir\$BIN_NAME`" --service"
```

Y en Rust:
```rust
if args.contains(&"--service".to_string()) {
    return run_as_windows_service();
}
```

**Recomendación:** el flag `--service` es el approach más simple y predecible.

## Acceptance criteria

- [ ] `cargo build --release -p ank-server` compila en Windows sin errores
- [ ] `Start-Service AegisOS` inicia el servicio exitosamente (sin timeout %%1053)
- [ ] `Stop-Service AegisOS` detiene el servicio limpiamente
- [ ] Ejecutar `ank-server.exe` directamente (sin `--service`) funciona igual que antes
- [ ] En Linux/macOS no hay cambios de comportamiento

## Dependencies

CORE-265 (dotenvy) — ya mergeado.

## Priority

**CRITICAL** — bloquea instalación en Windows como servicio para todos los usuarios.
