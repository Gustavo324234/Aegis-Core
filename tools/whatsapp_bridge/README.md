# Aegis WhatsApp Bridge

Local Node.js sidecar built on `@whiskeysockets/baileys` (`6.7.24`) that maintains per-tenant WhatsApp Web (QR) sessions for `ank-server` without exposing WhatsApp credentials outside the host.

> Managed automatically by [`ank_core::whatsapp::WhatsAppManager`](../../kernel/crates/ank-core/src/whatsapp/mod.rs), or run standalone on `127.0.0.1:8100`.

---

## Requirements & Checks

- **Node.js** (`node` on `PATH`, or configured via `AEGIS_NODE_BIN`)

```bash
cd tools/whatsapp_bridge
npm install
npm run check   # Runs node --check index.mjs
```

---

## Environment Variables

### Bridge Process (`index.mjs`)

| Variable | Default | Description |
|---|---|---|
| `WA_BRIDGE_PORT` | `8100` | HTTP port bound on `127.0.0.1` |
| `WA_BRIDGE_SECRET` | — | Shared Bearer secret required on authenticated routes |
| `WA_DATA_DIR` | Configured per host | Per-tenant Baileys auth & directory store directory |

### Kernel Integration (`ank-core::whatsapp`)

| Variable | Default | Description |
|---|---|---|
| `AEGIS_WHATSAPP_PORT` | `http_port + 10` (`8010` prod / `8100` sandbox `8090`) | Local port passed as `WA_BRIDGE_PORT` when auto-spawned |
| `AEGIS_WHATSAPP_BRIDGE_URL` | `http://127.0.0.1:<port>` | Override bridge base URL |
| `AEGIS_WHATSAPP_SECRET` | `<data_dir>/whatsapp/.bridge_secret` | Explicit Bearer secret shared with bridge |
| `AEGIS_WHATSAPP_BRIDGE_DIR` | Auto-discovered | Explicit path to `tools/whatsapp_bridge` |
| `AEGIS_NODE_BIN` | `node` | Custom Node.js executable path |

---

## HTTP API (`127.0.0.1:<WA_BRIDGE_PORT>`)

All routes except `GET /health` require `Authorization: Bearer <secret>` (verified with constant-time comparison).

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | Unauthenticated liveness check (`{ ok: true, service: "aegis-whatsapp-bridge" }`) |
| `GET` | `/status?tenant=...` | Connection state (`disconnected`, `connecting`, `qr_ready`, `connected`, `logged_out`), QR data URL, phone & counts |
| `POST` | `/connect` | Start or warm Baileys socket for `tenant` |
| `POST` | `/disconnect` | Disconnect session (`logout: true` clears saved creds) |
| `GET` | `/chats?tenant=...&search=...&limit=20` | Ranked recent chats (`total_chats`, `total_contacts`, `chats`) |
| `GET` | `/contacts?tenant=...&search=...&limit=50` | Deduplicated merged contact directory (`buildMergedDirectory`) |
| `GET` | `/messages?tenant=...&chat=...&limit=20` | Recent messages for a target JID, phone, or contact name (capped at `60` per chat) |
| `POST` | `/resolve` | Resolve recipient (`{ tenant, to \| target }`); returns `409 AMBIGUOUS_RECIPIENT` + `candidates` when multiple contacts match |
| `POST` | `/send` | Send plain-text message (`{ tenant, to, text }`), spaced by `MIN_BRIDGE_SEND_GAP_MS = 1500` ms |

---

## Contact Resolution & Directory Caps

- **Session Caps**: `MAX_CHATS = 300`, `MAX_CONTACTS = 2000`, `MAX_MESSAGES_PER_CHAT = 60`.
- **Sync Listeners**: Listens to `messaging-history.set` (preserving `conversationTimestamp || lastMsgTimestamp || t`), `chats.*`, `contacts.upsert`, and `contacts.update`.
- **`@lid` / `@s.whatsapp.net` Merge**: `buildMergedDirectory()` extracts normalized phone digits (`extractRealPhoneFromJid`) to merge duplicate `@lid` and `@s.whatsapp.net` records and combines `name`, `verifiedName`, and `notify` fields.
- **Accent-Insensitive Lookup**: `normalizeSearchText` strips accents and punctuation so searches match naturally regardless of diacritics.
- **Persistent Aliases (`TenantDB`)**: Custom nicknames/notes (e.g. `"Mi Señora"`, `"Mamá"`) saved via `manage_whatsapp(action: "save_contact" | "list_contacts")` are stored encrypted per tenant under `KV_CONTACT_ALIASES` and checked automatically during sending and `read_messages_with_aliases`.

---

## Anti-Spam & Confirmation Safety (`ank-core::whatsapp`)

The kernel wraps bridge calls through `ank-http` (`/api/whatsapp/*`) and `manage_whatsapp` with strict safeguards:

1. **Confirmation Gate (On by Default)**: Outgoing sends queue pending confirmations (`PENDING_TTL_SECS = 1800`s, max `25` queued) rather than sending immediately unless `require_confirmation` is disabled via **Settings → Cuentas Conectadas** or `manage_whatsapp(action: "set_confirmation")`.
2. **Per-Tenant Rate Limiting (`WhatsAppRateConfig::default()`)**:
   - `min_interval_secs`: `5` (`sanitize` clamp `2..60`)
   - `max_per_minute`: `6` (`sanitize` clamp `1..20`)
   - `max_per_hour`: `30` (`sanitize` clamp `min..120`)
   - `max_per_day`: `120` (`sanitize` clamp `hour..400`)
   - `max_per_recipient_10m`: `5` (`sanitize` clamp `1..15`)
   - `dedup_window_secs`: `60` (`sanitize` clamp `10..600`)
