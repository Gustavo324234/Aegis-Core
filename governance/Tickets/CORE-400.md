# CORE-400: Aislamiento Multi-Tenant y SSRF (`agents.rs`, `chal/mod.rs`, `antigravity_bridge`, `vcm/mod.rs`, `video_api`, `engine.rs`, `admin.rs`)

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer

---

## Description

Corregir todas las brechas de aislamiento multi-tenant y SSRF identificadas en la auditoría:
- `ank-http/src/routes/agents.rs`: validar pertenencia al tenant en `spawn_agent` y verificar ownership de `parent_id`.
- `ank-core/src/chal/mod.rs`: impedir que un tenant no-master elimine global keys en `manage_ai_provider(action="remove")`.
- `ank-http/src/routes/antigravity_bridge.rs` y `antigravity_runner.rs`: restringir mutación de config global a Master Admin y particionar `ide_outbox` por `tenant_id`.
- `ank-core/src/vcm/mod.rs`: evitar fallback de `fetch_git_state` al repo raíz del host (`.`) y fuga de `governance/TICKETS_MASTER.md` para tenants no-master.
- `ank-http/src/routes/video_api.rs` y `video_studio/executor.rs`: exigir `auth.is_master` en `dependencies/install` y reemplazo de `tenant_id == "root" || cfg!(debug_assertions)` en `video_configure_engines`.
- `ank-http/src/routes/engine.rs`: resolver DNS y bloquear IPs internas/metadata en `validate_engine_api_url` / cliente HTTP anti-rebinding.
- `ank-http/src/routes/admin.rs`: validar formato de `tenant_id` (`[a-zA-Z0-9_-]`), validar longitud mínima de contraseña en `reset_password`, y purgar `<data_dir>/users/<id>` y sesiones WhatsApp al eliminar un tenant.

## Acceptance Criteria

- [x] Ningún endpoint o tool permite a un tenant leer o modificar estado, claves, procesos, archivos o colas de otro tenant o del host.
- [x] `validate_engine_api_url` bloquea DNS rebinding / hostnames que resuelven a IPs privadas o metadata.
- [x] Tests de aislamiento cubren cada ruta modificada.
