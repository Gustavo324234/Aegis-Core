# CORE-391: Conector WhatsApp QR Multi-Tenant con Anti-Spam, Confirmación y Directorio/Alias Inteligente (`tools/whatsapp_bridge/`, `ank-core/whatsapp`, `ank-http`, `shell/ui`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 60
- **Asignado a:** Kernel Engineer + Shell Engineer + AI Architect

---

## Description
Integración completa de WhatsApp mediante sidecar local Node.js (`@whiskeysockets/baileys`) administrado por `ank-server` con aislamiento multi-tenant, controles estrictos contra spam/bloqueos de cuenta, confirmación humana previa por defecto al enviar mensajes, unificación de JIDs `@lid` vs `@s.whatsapp.net` y agenda cifrada de alias personales (ej. `"Mi Señora"`).

## Archivos Creados / Modificados
1. **`tools/whatsapp_bridge/index.mjs` y `package.json`**:
   - Sidecar HTTP local (`127.0.0.1:8100`, `@whiskeysockets/baileys` `6.7.24`) autenticado con Bearer (`constantTimeBearerCheck`) y endpoints `/health`, `/status`, `/connect`, `/disconnect`, `/chats`, `/contacts`, `/messages`, `/resolve` (`409 AMBIGUOUS_RECIPIENT`) y `/send` (`MIN_BRIDGE_SEND_GAP_MS = 1500`).
   - Topes `MAX_CHATS = 300`, `MAX_CONTACTS = 2000`, `MAX_MESSAGES_PER_CHAT = 60`, listeners `messaging-history.set` (conservando timestamps reales) y `contacts.update`.
   - `buildMergedDirectory()` con extracción de dígitos (`extractRealPhoneFromJid`) para deduplicar `@lid` / `@s.whatsapp.net`, fusión de `name`/`verifiedName`/`notify` y búsqueda insensible a acentos (`normalizeSearchText`).
2. **`kernel/crates/ank-core/src/whatsapp/mod.rs`**:
   - `WhatsAppManager` (auto-spawn del bridge, secreto compartido `.bridge_secret` o `AEGIS_WHATSAPP_SECRET`) y `WhatsAppService`.
   - Confirmación previa por defecto (`require_confirmation = true`, cola de `PENDING_TTL_SECS = 1800`s, `MAX_PENDING_ITEMS = 25`), rate limiter configurable (`WhatsAppRateConfig::default()`: `min_interval_secs: 5`, `max_per_minute: 6`, `max_per_hour: 30`, `max_per_day: 120`, `max_per_recipient_10m: 5`, `dedup_window_secs: 60`).
   - Libro cifrado de alias por tenant (`KV_CONTACT_ALIASES`, `normalize_alias_key`, `save_contact_alias`, `list_contact_aliases`, `resolve_contact_alias`, `read_messages_with_aliases`).
3. **`kernel/crates/ank-http/src/routes/whatsapp_api.rs`, `tool_registry.rs`, `chat_agent.md` y `shell/ui/src/components/ConnectedAccountsTab.tsx`**:
   - Rutas `/api/whatsapp/{status, connect, disconnect, settings, chats, pending/:id/confirm, pending/:id DELETE}`.
   - Herramienta `manage_whatsapp` (incluyendo `save_contact`, `list_contacts` y `set_confirmation`) y tarjeta QR / confirmaciones pendientes / límites en `ConnectedAccountsTab.tsx`.

## Acceptance Criteria
- [x] El usuario vincula WhatsApp escaneando QR por tenant desde Settings → Cuentas Conectadas sin exponer credenciales fuera del host.
- [x] Los envíos salientes exigen confirmación por defecto (cancelable o configurable desde UI y chat) y respetan topes anti-spam por minuto/hora/día/destinatario y deduplicación.
- [x] Alias guardados (`save_contact`) y contactos con `@lid`/`@s.whatsapp.net` o tildes se resuelven y fusionan sin falsos duplicados (`npm run check` y test unitario `contact_aliases_save_normalize_and_resolve_roundtrip` PASS).
