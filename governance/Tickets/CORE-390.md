# CORE-390: Integración Google Calendar y Google OAuth PKCE S256 Compartible (`ank-core/oauth`, `ank-http`, `shell/ui`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 60
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description
Para consultar y agendar eventos reales del usuario en Google Calendar desde el asistente, se implementó el flujo completo OAuth 2.0 Web con PKCE (`S256`) dentro de Aegis y la herramienta de agente `manage_google_calendar`, permitiendo además reutilizar un Client ID existente (como el de Orion Creative) o configurar credenciales propias por tenant o por entorno.

## Archivos Creados / Modificados
1. **`kernel/crates/ank-http/src/routes/oauth_api.rs` y `kernel/crates/ank-core/src/oauth/mod.rs`**:
   - Endpoints `/api/oauth/google/config`, `/api/oauth/google/start`, `/api/oauth/google/callback`, `/api/oauth/status`, `/api/oauth/tokens` y `DELETE /api/oauth/:provider`.
   - Resolución de credenciales `resolve_client_credentials` priorizando configuración por tenant en `TenantDB` y variables `GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_CLIENT_ID`.
   - Estado PKCE en `state.pending_oauth_flows` (`code_challenge_method=S256`) y callback HTML con `postMessage` (`aegis-oauth-complete`).
2. **`kernel/crates/ank-core/src/oauth/google_calendar.rs` y `kernel/crates/ank-core/src/agents/tool_registry.rs`**:
   - Nuevo módulo `google_calendar.rs` (`normalize_calendar_datetime`, operaciones contra `googleapis.com/calendar/v3/calendars/primary/events`) y herramienta `manage_google_calendar` (`status`, `list`, `create`, `update`, `delete`).
3. **`shell/ui/src/components/ConnectedAccountsTab.tsx`**:
   - Interfaz en Settings → Cuentas Conectadas para guardar/limpiar Client ID/Secret/Redirect URI de Google, copiar Redirect URI y completar el popup OAuth.

## Acceptance Criteria
- [x] El flujo OAuth Web con PKCE S256 guarda tokens cifrados por tenant en `TenantDB` y admite reutilizar un Client ID compartible (ej. Orion Creative).
- [x] `manage_google_calendar` lista, crea, actualiza y elimina eventos reales en Google Calendar con fechas locales o RFC3339.
- [x] `cargo test -p ank-http --lib` valida precedencia de `redirect_uri`, HTML de callback y deserialización OAuth.
