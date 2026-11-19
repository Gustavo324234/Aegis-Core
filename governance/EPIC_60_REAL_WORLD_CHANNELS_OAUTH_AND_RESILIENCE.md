# EPIC 60 — Real-World Channels (WhatsApp & Google Calendar), Chat UX & ReAct Resilience

**Estado:** ✅ Done

## 🎯 Objetivo

Llevar el asistente personal de Aegis del uso en entorno local a **canales reales de comunicación y agenda** (WhatsApp Web vía Baileys y Google Calendar vía OAuth PKCE S256 compartible), pulir la experiencia diaria del chat infinito (CRUD real de recordatorios, limpieza visual sin pérdida de historial, tablas GFM y visibilidad del proveedor/modelo activo), aislar el flujo de pruebas en Windows (`run_sandbox.bat` + `:8090`), y endurecer la resiliencia del bucle cognitivo ReAct ante errores de proveedores, streams vacíos y chunks UTF-8 partidos.

---

## 🏗️ Pilares y Criterios de Aceptación

### Pilar 1: Observabilidad, Herramientas de Chat y Entorno de Sandbox Web
1. **Visibilidad Proveedor/Modelo, Tablas GFM y Tools de Catálogo/Módulos (`CORE-387` — ✅ Done):**
   - Badge de proveedor + modelo activo en `TelemetryDashboard.tsx`, selector agrupado por proveedor y tablas GFM (`remark-gfm`) en `ChatTerminal.tsx`.
   - Herramientas `manage_ai_provider` y `manage_modules` en `ToolRegistry` / `chal`, catálogo ampliado y auto-probe en UI.
2. **Launcher Web Aislado (`:8090`) y Scripts de Actualización Windows (`CORE-388` — ✅ Done):**
   - `tools/agent_sandbox/launch_web_server.py` y `run_sandbox.bat` (`--server-only`) para levantar un `ank-server` aislado en `127.0.0.1:8090` sin afectar `C:\ProgramData\Aegis\aegis.env`.
   - Scripts `update.bat` y `update.ps1` para actualizar binario `ank-server.exe` y `shell/ui/dist` en Windows.

### Pilar 2: Gestión Diaria de Chat, Recordatorios y Agenda Real
3. **CRUD Completo de Recordatorios e Historial Infinito con Limpieza Visual (`CORE-389` — ✅ Done):**
   - Acciones `update` y `delete` más recurrencia en `manage_reminder`, `TenantDB` y `/api/personal/reminders/:id`.
   - Paginación hacia atrás y limpieza visual no destructiva (`/api/chat/history`, `/api/chat/ui-state`, `/api/chat/visual-clear`, botón `"Limpiar vista"` y opción `clean_on_open`).
4. **Google Calendar y OAuth PKCE S256 Compartible (`CORE-390` — ✅ Done):**
   - Flujo Google OAuth web (`/api/oauth/google/config`, `/start`, `/callback`) reutilizable con credenciales propias o compartidas (ej. Client ID de Orion Creative).
   - Herramienta `manage_google_calendar` (`status`, `list`, `create`, `update`, `delete`) y tarjeta de vinculación en `ConnectedAccountsTab.tsx`.

### Pilar 3: WhatsApp Multi-Tenant Seguro y Resiliencia ReAct
5. **Conector WhatsApp QR, Anti-Spam y Directorio/Alias Inteligente (`CORE-391` — ✅ Done):**
   - Sidecar Node.js + `@whiskeysockets/baileys` (`tools/whatsapp_bridge/index.mjs`) gestionado por `WhatsAppManager`, rutas `/api/whatsapp/*` y panel QR/límites en `ConnectedAccountsTab.tsx`.
   - Confirmación por defecto (`require_confirmation`), limitador anti-spam (`WhatsAppRateConfig`), unificación `@lid`/`@s.whatsapp.net` (`buildMergedDirectory`, `GET /contacts`) y alias cifrados por tenant (`KV_CONTACT_ALIASES`, `save_contact` / `list_contacts`).
6. **Failover Dinámico Multi-Proveedor, UTF-8 SSE Seguro y Post-Tool Recovery (`CORE-392` — ✅ Done):**
   - `CognitiveRouter::decide_excluding` (sin sticky cache en failover), `decode_next_sse_line_from_buf` sobre `Vec<u8>`, recuperación ante `stream_was_empty` post-tool, `StreamTextSanitizer` y ajuste de `is_status_query`.
7. **Documentación Técnica y Gobernanza (`CORE-393` — ✅ Done):**
   - Sincronización de `CHANGELOG.md`, `ARCHITECTURE.md`, `README.md`, `README.es.md`, `tools/whatsapp_bridge/README.md`, `TICKETS_MASTER.md` y `tools/sync_tickets_master.py`.

---

## 🛠️ Listado de Tickets Mapeados

* **CORE-387:** Observabilidad Proveedor/Modelo Activo, Tools de Proveedores/Módulos y Tablas GFM en Chat (`ank-core`, `shell/ui`) — ✅ Done
* **CORE-388:** Launcher Web Aislado para Sandbox (`:8090`) y Scripts de Actualización Windows (`tools/agent_sandbox/`, raíz) — ✅ Done
* **CORE-389:** CRUD Completo de Recordatorios e Historial de Chat Infinito con Limpieza Visual (`ank-core`, `ank-http`, `shell/ui`) — ✅ Done
* **CORE-390:** Integración Google Calendar y Google OAuth PKCE S256 Compartible (`ank-core/oauth`, `ank-http`, `shell/ui`) — ✅ Done
* **CORE-391:** Conector WhatsApp QR Multi-Tenant con Anti-Spam, Confirmación y Directorio/Alias Inteligente (`tools/whatsapp_bridge/`, `ank-core/whatsapp`, `ank-http`, `shell/ui`) — ✅ Done
* **CORE-392:** Failover Dinámico Multi-Proveedor en Bucle ReAct, Streaming SSE UTF-8 Seguro y Recuperación Post-Tool (`ank-core/chal`, `ank-core/router`) — ✅ Done
* **CORE-393:** Sincronización de Documentación, Changelog, Arquitectura, Sidecar Docs y Gobernanza (`docs`, `governance/`) — ✅ Done
