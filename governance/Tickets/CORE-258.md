# CORE-258 — CLI: soporte multiplataforma (Windows + Linux) en ank-cli

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** Infraestructura  
**Asignado a:** Kernel Engineer  
**Componente:** `kernel/crates/ank-cli/`

**Estado:** ✅ Done
---

## Contexto

El CLI actual (`ank-cli`) está implementado para Linux con systemd. Todos sus comandos asumen `journalctl`, `systemctl`, y rutas POSIX (`/usr/local/bin`, `/etc/aegis/`, `/var/lib/aegis/`). En Windows no funciona.

El objetivo es un CLI unificado en Rust que detecte el OS en compile time y use el backend correcto — Windows Service Manager en Windows, systemd en Linux.

---

## Comandos a implementar

Todos los comandos del `docs/CLI_REFERENCE.md` deben funcionar en ambas plataformas:

| Comando | Descripción |
|---|---|
| `aegis status` | Estado del servicio + health check HTTP |
| `aegis start` | Iniciar el servicio |
| `aegis stop` | Detener el servicio |
| `aegis restart` | Reiniciar el servicio |
| `aegis logs [N]` | Ver últimas N líneas del log (default 50) + follow |
| `aegis token` | Imprimir URL con setup token fresco |
| `aegis version` | Versión instalada |
| `aegis update` | Actualizar al último release estable |
| `aegis update --beta` | Actualizar al nightly |
| `aegis diag` | Diagnóstico completo del sistema |

---

## Arquitectura

Usar un trait `ServiceBackend` con dos implementaciones compiladas condicionalmente:

```rust
trait ServiceBackend {
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    fn status(&self) -> Result<ServiceStatus>;
    fn logs(&self, lines: usize, follow: bool) -> Result<()>;
}

#[cfg(target_os = "windows")]
struct WindowsBackend;   // sc.exe + Windows Event Log / archivo de log

#[cfg(not(target_os = "windows"))]
struct SystemdBackend;   // systemctl + journalctl

fn get_backend() -> Box<dyn ServiceBackend> {
    #[cfg(target_os = "windows")]
    return Box::new(WindowsBackend::new());
    #[cfg(not(target_os = "windows"))]
    return Box::new(SystemdBackend::new());
}
```

---

## Implementación por plataforma

### Windows (`WindowsBackend`)

| Operación | Implementación |
|---|---|
| `start` | `sc.exe start AegisOS` |
| `stop` | `sc.exe stop AegisOS` |
| `restart` | `sc.exe stop AegisOS` + `sc.exe start AegisOS` |
| `status` | `sc.exe query AegisOS` + `GET http://localhost:8000/health` |
| `logs N` | Leer `%ProgramData%\Aegis\logs\aegis.log` — últimas N líneas |
| `logs` (follow) | Tail continuo del mismo archivo |
| `token` | `GET http://localhost:8000/api/admin/setup-token` |
| `version` | `GET http://localhost:8000/health` → campo version |
| `update` | Descargar release de GitHub + reemplazar binario + restart |
| `diag` | sc.exe query + últimas 100 líneas de log + health endpoint |

**Dependencia de log en Windows:** Para que `aegis logs` funcione cuando corre como servicio, el wrapper `start-aegis.bat` (CORE-255) debe redirigir stdout al archivo de log:
```bat
"C:\Program Files\Aegis\ank-server.exe" >> "C:\ProgramData\Aegis\logs\aegis.log" 2>&1
```
El Kernel Engineer debe coordinar este cambio con CORE-255.

### Linux (`SystemdBackend`)

| Operación | Implementación |
|---|---|
| `start` | `systemctl start aegis` |
| `stop` | `systemctl stop aegis` |
| `restart` | `systemctl restart aegis` |
| `status` | `systemctl status aegis` + `GET http://localhost:8000/health` |
| `logs N` | `journalctl -u aegis -n N` |
| `logs` (follow) | `journalctl -u aegis -f` |
| `token` | `GET http://localhost:8000/api/admin/setup-token` |
| `version` | `GET http://localhost:8000/health` → campo version |
| `update` | Descargar release de GitHub + reemplazar binario + restart |
| `diag` | `journalctl -u aegis -n 100` + `systemctl status` + health endpoint |

---

## Output esperado — `aegis status`

```
Aegis OS — Status
─────────────────────────────────────────
Service:  ● Running  (AegisOS)
Uptime:   2h 14m 33s
Version:  0.8.2
HTTP:     ✓ Online   http://localhost:8000
Profile:  Cloud  (openrouter/free)
─────────────────────────────────────────
```

## Output esperado — `aegis diag`

```
Aegis OS — Diagnostic Report
Generated: 2026-05-04T17:00:00Z
─────────────────────────────────────────
SYSTEM
  OS:      Windows 11 Pro (10.0.22631)
  Arch:    x86_64

SERVICE
  Name:    AegisOS
  Status:  Running
  PID:     12345

HEALTH
  HTTP:    200 OK
  Version: 0.8.2
  Uptime:  8042s

RECENT ERRORS (last 20 lines)
  [no errors found]
─────────────────────────────────────────
```

---

## Dependencias en Cargo.toml de ank-cli

```toml
[dependencies]
clap       = { version = "4", features = ["derive"] }
tokio      = { version = "1", features = ["full"] }
reqwest    = { version = "0.11", features = ["json", "blocking"] }
colored    = "2"
anyhow     = "1"
```

No se necesitan dependencias de plataforma adicionales — Windows usa `Command::new("sc.exe")` y Linux usa `Command::new("systemctl")`, ambos disponibles en std.

---

## CI/CD — agregar ank-cli a publish-native.yml

En cada job de build, agregar compilación de `ank-cli`:

```yaml
- name: Build
  run: |
    cargo build --release -p ank-server
    cargo build --release -p ank-cli
```

Empaquetar en el artefacto:
```yaml
# Linux x86_64
cp target/release/ank-cli dist/aegis-linux-x86_64

# Linux arm64
cp target/release/ank-cli dist/aegis-linux-arm64

# Windows
cp target/release/ank-cli.exe dist/aegis-windows-x86_64.exe
```

El installer (`install.sh` en Linux, `install.ps1` en Windows) descarga el binario `aegis` junto al `ank-server` y lo coloca en el PATH.

---

## Criterios de aceptación

- [ ] `aegis status` muestra estado real del servicio y health check en Windows y Linux.
- [ ] `aegis start` / `stop` / `restart` controlan el servicio en ambas plataformas.
- [ ] `aegis logs 50` muestra las últimas 50 líneas en ambas plataformas.
- [ ] `aegis logs` sin argumento hace follow (streaming continuo).
- [ ] `aegis token` imprime la URL con token válido.
- [ ] `aegis version` imprime la versión instalada.
- [ ] `aegis diag` genera un reporte completo con info de sistema, servicio y health.
- [ ] `aegis update` descarga el último release estable y reinicia el servicio.
- [ ] `aegis update --beta` descarga el nightly.
- [ ] El binario compilado se llama `aegis` en Linux y `aegis.exe` en Windows.
- [ ] El CI publica `aegis-linux-x86_64`, `aegis-linux-arm64`, `aegis-windows-x86_64.exe` en el release nightly.
- [ ] `cargo build --workspace` sin errores en Linux y Windows.

## Dependencias

- CORE-255 — el CLI asume que el servicio Windows se llama `AegisOS` y que el log se escribe en `%ProgramData%\Aegis\logs\aegis.log`
