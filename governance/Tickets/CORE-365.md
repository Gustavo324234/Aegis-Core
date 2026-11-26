# CORE-365: Complete aegis-supervisor Windows Service Lifecycle, Bounded Health Checks, Secret Safety, and Process Reaping

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer

## Description
1. `aegis-supervisor/src/windows_service.rs` left `service_run` as an idle stub (`let _ = shutdown_rx.recv();`) that reported `Running` to Windows SCM without ever starting or supervising `ank-server`, and registered the service without `--service`.
2. `health::check_health` used `reqwest::get` with no connect or request timeout, allowing a hung socket in `ank-server` to block `AegisSupervisor::watch` indefinitely, while `watch` never reaped exited child processes via `try_wait()` or handled `Ctrl+C` graceful shutdown.
3. `load_config` unconditionally defaulted `AEGIS_ROOT_KEY` to `"development_key"` even in production mode (shadowing `/etc/aegis/aegis.env` or `C:\ProgramData\Aegis\aegis.env`), and `aegis stop`, `aegis restart`, and `aegis logs` were unimplemented stubs.

## Acceptance Criteria
- [x] Start, supervise, and gracefully stop `AegisSupervisor` inside `windows_service::service_run` on `Stop`/`Shutdown` events and register `--service` in `install_service`.
- [x] Enforce bounded HTTP timeouts in `health::check_health`, detect immediate child exits with `try_wait()` in `AegisSupervisor::watch`, and handle `Ctrl+C` / PID-file lifecycle cleanly.
- [x] Only inject fallback `"development_key"` when `dev_mode` is active (preserving `aegis.env` resolution in production), and implement real `stop`, `restart`, and `logs` commands in `aegis-supervisor`.
