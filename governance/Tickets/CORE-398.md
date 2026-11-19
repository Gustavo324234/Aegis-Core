# CORE-398: Hardening HTTP/WS — CORS, `is_direct_local_cli_probe`, `quick_personal_setup` y Auth Pre-Upgrade en WebSockets

- **Tipo:** fix
- **Estado:** 📥 Todo
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer

---

## Description

Cerrar los vectores de exfiltración cross-origin, bypass de `setup_token` y upgrade WebSocket no autenticado:
- Reemplazar `CorsLayer::allow_origin(Any)` en `ank-http/src/routes/mod.rs` por orígenes explícitos configurables / same-origin.
- Endurecer `is_direct_local_cli_probe` (`status.rs`) para rechazar requests con cabeceras de navegador (`Origin`, `Sec-Fetch-Site`) además de cabeceras proxy.
- Exigir `setup_token` válido y validar cabeceras de proxy en `quick_personal_setup` (`auth.rs`), y corregir `client_ip()` para no agrupar tráfico proxificado bajo `127.0.0.1`.
- Mover la autenticación de `ws/chat.rs` y `ws/siren.rs` a antes del handshake `ws.on_upgrade(...)` (igual que `ws/agents.rs`), usando `strip_prefix("session-key.")` y propagando `PASSWORD_MUST_CHANGE`.

## Acceptance Criteria

- [ ] Peticiones cross-origin desde navegadores no acceden a endpoints locales ni drenan `/api/antigravity/ide/outbox/pop` o `/api/system/connection-info`.
- [ ] `quick_personal_setup` exige `setup_token` y no confía ciegamente en `ip.is_loopback()` cuando existen cabeceras proxy.
- [ ] `ws/chat.rs` y `ws/siren.rs` rechazan credenciales inválidas con HTTP 401/403 antes de responder `101 Switching Protocols`.
