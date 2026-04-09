# CORE-013 — ank-http: endpoints REST router + status + workspace + providers

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/routes/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Alta
**Estado:** COMPLETED
**Depende de:** CORE-012

---

## Contexto

Segunda mitad de los endpoints REST. Cubre el CMR (Cognitive Model Router),
telemetría, workspace (file upload) y providers externos.

**Especificación:** `Aegis-Shell/bff/main.py` líneas 350–700

---

## Endpoints a implementar

### `src/routes/router_api.rs` — CMR

| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/router/keys/global` | Admin | Agregar key global al KeyPool |
| `GET` | `/api/router/keys/global` | Admin | Listar keys globales |
| `DELETE` | `/api/router/keys/global/:id` | Admin | Eliminar key global |
| `POST` | `/api/router/keys/tenant` | Tenant | Agregar key de tenant |
| `GET` | `/api/router/keys/tenant` | Tenant | Listar keys de tenant |
| `DELETE` | `/api/router/keys/tenant/:id` | Tenant | Eliminar key de tenant |
| `GET` | `/api/router/models` | Tenant | Listar modelos del catálogo |
| `POST` | `/api/router/sync` | Admin | Forzar sync del catálogo |
| `GET` | `/api/router/status` | Tenant | Estado del router |

### `src/routes/status.rs` — Telemetría

| Método | Path | Auth | Descripción |
|---|---|---|---|
| `GET` | `/api/status` | Tenant (header x-citadel-key) | Métricas del kernel |
| `GET` | `/api/system/state` | Pública | Estado general del sistema |
| `GET` | `/api/system/sync_version` | Pública | Versión del sistema |
| `GET` | `/health` | Pública | Health check del servidor |

### `src/routes/workspace.rs` — File upload

| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/workspace/upload` | Tenant (form) | Subir archivo al workspace del tenant |

El upload valida `tenant_id` con regex `^[a-zA-Z0-9_-]+$`, escribe en
`{data_dir}/users/{tenant_id}/workspace/{safe_filename}`.

### `src/routes/providers.rs` — Providers externos

| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/providers/models` | Pública | Listar modelos de un provider externo |

Soporta: `anthropic` (lista hardcodeada), `gemini` (lista hardcodeada),
`ollama` (query a localhost:11434), cualquier otro (query a `/v1/models`).

### `src/routes/siren_api.rs` — Siren voice config

| Método | Path | Descripción |
|---|---|---|
| `GET` | `/api/siren/config` | Config de voz del tenant |
| `POST` | `/api/siren/config` | Actualizar config de voz |
| `GET` | `/api/siren/voices` | Listar voces disponibles |

Estos endpoints llaman a los RPCs `GetSirenConfig`, `SetSirenConfig`, `ListSirenVoices`
del kernel directamente (sin traducción — los tipos ya están en `ank-core`).

---

## Notas de implementación

**File upload — path safety:**
```rust
// Validar tenant_id
if !regex!(r"^[a-zA-Z0-9_-]+$").is_match(&tenant_id) {
    return Err(AegisHttpError::BadRequest("Invalid tenant_id".into()));
}
// Sanitizar filename
let safe_name = original_name
    .chars()
    .map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' })
    .collect::<String>();
// Resolver path dentro del data_dir (no fuera)
let base = state.config.data_dir.join("users").join(&tenant_id).join("workspace");
let file_path = base.join(&safe_name);
if !file_path.starts_with(&base) {
    return Err(AegisHttpError::BadRequest("Path traversal detected".into()));
}
```

---

## Criterios de aceptación

- [ ] `GET /api/status` retorna métricas con formato compatible con el frontend actual
- [ ] `GET /health` retorna `{ "status": "Aegis HTTP Online" }`
- [ ] `POST /api/workspace/upload` sube el archivo al path correcto del tenant
- [ ] `POST /api/workspace/upload` rechaza `tenant_id` con caracteres inválidos
- [ ] `POST /api/workspace/upload` rechaza filenames con path traversal
- [ ] `POST /api/providers/models` retorna lista de modelos para cada provider soportado
- [ ] `GET /api/router/models` retorna el catálogo completo
- [ ] `POST /api/router/keys/global` agrega key al KeyPool
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-Shell/bff/main.py` líneas 350–700 — comportamiento a reimplementar
