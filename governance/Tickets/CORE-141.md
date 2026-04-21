# CORE-141 — Feature: Google Integrations — Calendar, Drive, Gmail via syscalls

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `kernel/`
**Crate:** `ank-core`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Kernel Engineer
**Depende de:** CORE-138 (OAuth tokens)

---

## Contexto

Con Google conectado, el agente tiene acceso a Calendar, Drive y Gmail sin que
el tenant configure nada extra. Este ticket agrega tres syscalls nuevas que el
LLM puede emitir cuando el usuario pide información o acción sobre estos servicios.

**Principio:** Read-only en MVP. Escritura (crear eventos, enviar emails) en fase posterior
— requiere scopes adicionales y más validación.

---

## Nuevas Syscalls

### `GoogleCalendarList` — Listar próximos eventos

**Trigger del LLM:**
```
[SYS_CALL_PLUGIN("google_calendar", {"days": 7, "max_results": 10})]
```

**Implementación:**
```rust
// GET https://www.googleapis.com/calendar/v3/calendars/primary/events
// params: timeMin=now, timeMax=now+days, maxResults, singleEvents=true, orderBy=startTime
// Headers: Authorization: Bearer <token>
```

**Resultado retornado al LLM:**
```json
{
  "events": [
    {
      "title": "Reunión de equipo",
      "start": "2026-04-22T10:00:00-03:00",
      "end": "2026-04-22T11:00:00-03:00",
      "location": "Sala B",
      "description": "...",
      "attendees": ["ana@example.com", "bob@example.com"]
    }
  ]
}
```

---

### `GoogleDriveList` — Buscar/listar archivos en Drive

**Trigger del LLM:**
```
[SYS_CALL_PLUGIN("google_drive", {"query": "presupuesto 2026", "max_results": 5})]
```

**Implementación:**
```rust
// GET https://www.googleapis.com/drive/v3/files
// params: q="name contains 'presupuesto'" (si hay query),
//         fields=id,name,mimeType,modifiedTime,webViewLink,size
//         orderBy=modifiedTime desc, pageSize=max_results
```

**Resultado:**
```json
{
  "files": [
    {
      "id": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms",
      "name": "Presupuesto Q1 2026",
      "type": "spreadsheet",
      "modified": "2026-04-10T15:32:00Z",
      "url": "https://docs.google.com/spreadsheets/d/..."
    }
  ]
}
```

---

### `GmailList` — Listar emails recientes o buscar

**Trigger del LLM:**
```
[SYS_CALL_PLUGIN("gmail", {"query": "from:jefe@empresa.com unread", "max_results": 5})]
```

**Implementación:**
```rust
// GET https://gmail.googleapis.com/gmail/v1/users/me/messages
// params: q=query, maxResults=max_results
// Luego GET /messages/{id} para cada mensaje (snippet + headers relevantes)
// Retornar solo: from, subject, snippet, date — NUNCA el body completo
// (privacidad + tokens del LLM)
```

**Resultado:**
```json
{
  "emails": [
    {
      "from": "jefe@empresa.com",
      "subject": "Revisión del proyecto",
      "snippet": "Hola, necesito que revises el documento antes...",
      "date": "2026-04-21T09:15:00Z",
      "unread": true
    }
  ]
}
```

---

## Implementación en `syscalls/mod.rs`

Agregar tres variantes al enum `Syscall`:

```rust
GoogleCalendar {
    days: u8,         // días hacia adelante, default 7
    max_results: u8,  // default 10
},
GoogleDrive {
    query: String,    // búsqueda por nombre (vacío = recientes)
    max_results: u8,  // default 5
},
Gmail {
    query: String,    // Gmail search query (vacío = inbox reciente)
    max_results: u8,  // default 5
},
```

Agregar regex en los `LazyLock`:

```rust
static GCAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("google_calendar",\s*(\{.*?\})\)\]"#)
        .expect("FATAL")
});
static GDRIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("google_drive",\s*(\{.*?\})\)\]"#)
        .expect("FATAL")
});
static GMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("gmail",\s*(\{.*?\})\)\]"#)
        .expect("FATAL")
});
```

En `SyscallExecutor::execute()`, para cada caso:
1. Abrir `TenantDB` del tenant
2. Llamar a `crate::oauth::get_or_refresh_token(http_client, &db, "google")`
3. Si retorna error "not connected": retornar mensaje al LLM para que informe al usuario
4. Hacer la llamada a la API de Google con el token
5. Retornar resultado estructurado como `[SYSTEM_RESULT: {...}]`

---

## Instrucciones en `SYSTEM_PROMPT_MASTER`

En `build_prompt()`, agregar sección condicional si Google está conectado.
Esto requiere leer el estado de OAuth del enclave en `build_prompt()`:

```rust
// En ws/chat.rs, antes de construir el PCB, leer las conexiones OAuth:
let google_connected = match TenantDB::open(&tenant_id, &session_key_hash) {
    Ok(db) => db.is_oauth_connected("google").unwrap_or(false),
    Err(_) => false,
};
let spotify_connected = match TenantDB::open(&tenant_id, &session_key_hash) {
    Ok(db) => db.is_oauth_connected("spotify").unwrap_or(false),
    Err(_) => false,
};

// Pasar como contexto al PCB (via temp_vars o nuevo campo)
// El HAL los usa en build_prompt()
```

Sección de prompt inyectada cuando Google está conectado:

```
GOOGLE INTEGRATIONS: Tenés acceso a Google Calendar, Drive y Gmail del usuario.
- Para ver eventos: [SYS_CALL_PLUGIN("google_calendar", {"days": 7})]
- Para buscar en Drive: [SYS_CALL_PLUGIN("google_drive", {"query": "<término>"})]
- Para leer emails: [SYS_CALL_PLUGIN("gmail", {"query": "unread", "max_results": 5})]
Usá estas herramientas cuando el usuario pregunte por su agenda, archivos o correos.
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] Sin Google conectado: el agente informa que debe conectar la cuenta
- [ ] "¿Qué tengo en el calendario esta semana?" → `google_calendar` → lista de eventos
- [ ] "Buscame el presupuesto en Drive" → `google_drive` → lista de archivos con links
- [ ] "Tengo emails sin leer de mi jefe?" → `gmail` → lista de emails con snippet
- [ ] Token expirado: se refresca automáticamente sin error visible al usuario
- [ ] Los links de Drive en la respuesta son clickeables en el chat (Markdown)

---

## Scopes requeridos en Google Cloud Console

El operador debe habilitar en su app de Google:
- `https://www.googleapis.com/auth/calendar.readonly`
- `https://www.googleapis.com/auth/drive.readonly`
- `https://www.googleapis.com/auth/gmail.readonly`
- `email profile`

Documentar en `installer/README.md`.

---

## Dependencias

- CORE-138 (OAuth token storage + refresh)

---

## Commit message

```
feat(ank-core): CORE-141 Google integrations — Calendar, Drive, Gmail syscalls via OAuth
```
