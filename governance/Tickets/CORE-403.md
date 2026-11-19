# CORE-403: Estabilidad y Límites SRE (`video_api`, logs acotados, graceful shutdown, `env::set_var` y `AntigravityRunner`)

- **Tipo:** fix
- **Estado:** 📥 Todo
- **Prioridad:** Alta
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer

---

## Description

Resolver riesgos de OOM, bloqueos, shutdown abrupto y UB en runtime:
- `ank-http/src/routes/video_api.rs`: usar streaming (`ReaderStream`) en `serve_project_media`, configurar `DefaultBodyLimit` acorde a `MAX_SCENE_UPLOAD_BYTES` (85 MB), limitar renders concurrentes FFmpeg con `Semaphore`, y emitir eventos de error si falla un task en background.
- `ank-http/src/routes/status.rs`: acotar `query.lines` y leer las últimas N líneas desde el final del archivo en `get_service_logs`; reemplazar `std::process::exit(0)` en restart/stop por apagado ordenado que drene WebSockets, WAL y procesos hijos.
- `ank-server/src/main.rs`: eliminar `std::env::set_var` tras iniciar Tokio multihilo pasando configuración explícita.
- `ank-core/src/agents/antigravity_runner.rs` y `tools/antigravity_bridge/agy_worker.py`: capturar `stderr` del worker, evitar bloqueo del mutex global durante todo el streaming, y pasar `workspace` sin mutar el CWD global vía `os.chdir()`.

## Acceptance Criteria

- [ ] Uploads hasta 85 MB y streaming de MP4/logs operan sin picos de memoria OOM.
- [ ] No existen llamadas a `std::env::set_var` en contexto multihilo ni `std::process::exit(0)` sin drenaje.
- [ ] `AntigravityRunner` soporta sesiones concurrentes en distintos workspaces con captura de errores `stderr`.
