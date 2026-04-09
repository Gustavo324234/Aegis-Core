# CORE-012 — ank-http: endpoints REST auth + admin + engine

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/routes/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Alta
**Estado:** TODO
**Depende de:** CORE-011

---

## Contexto

Implementar los endpoints REST que hoy sirve el BFF Python.
Este ticket cubre auth, admin de tenants y configuración de engine.

**Especificación de comportamiento:** `Aegis-Shell/bff/main.py`
Leer ese archivo línea por línea para entender qué hace cada endpoint.
No inventar — reimplementar el comportamiento exacto.

---

## Endpoints a implementar

### `src/routes/auth.rs`

| Método | Path | Descripción |
|---|---|---|
| `POST` | `/api/auth/login` | Citadel Handshake — valida tenant + key vía GetSystemStatus |
| `POST` | `/api/admin/setup` | Bootstrap Master Admin (texto plano → hash → Kernel) |
| `POST` | `/api/admin/setup-token` | Bootstrap con OTP de instalación |

### `src/routes/admin.rs`

| Método | Path | Descripción |
|---|---|---|
| `POST` | `/api/admin/tenant` | Crear tenant |
| `POST` | `/api/admin/tenant/create` | Alias del anterior |
| `GET` | `/api/admin/tenants` | Listar tenants (query: admin_tenant_id, admin_session_key) |
| `DELETE` | `/api/admin/tenant/:id` | Eliminar tenant |
| `POST` | `/api/admin/tenant/delete` | Eliminar tenant (body) |
| `POST` | `/api/admin/reset_password` | Reset password de tenant |

### `src/routes/engine.rs`

| Método | Path | Descripción |
|---|---|---|
| `GET` | `/api/engine/status` | Estado del engine configurado |
| `POST` | `/api/engine/configure` | Configurar engine dinámicamente |
| `POST` | `/api/system/hw_profile` | Cambiar HW profile (solo root) |

---

## Patrón de implementación

Todos los handlers siguen este patrón:

```rust
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<AuthRequest>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    let hash = hash_passphrase(&body.session_key);
    state
        .citadel
        .lock()
        .await
        .enclave
        .authenticate_tenant(&body.tenant_id, &hash)
        .await
        .map_err(AegisHttpError::from)?;
    Ok(Json(json!({
        "message": "Citadel Handshake Successful",
        "status": "authenticated"
    })))
}
```

### `src/error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum AegisHttpError {
    #[error("Citadel: {0}")]
    Citadel(#[from] crate::citadel::CitadelError),
    #[error("Kernel error: {0}")]
    Kernel(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AegisHttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            AegisHttpError::Citadel(e) => return e.clone().into_response(),
            AegisHttpError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AegisHttpError::Kernel(m) => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
            AegisHttpError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
```

---

## Criterios de aceptación

- [ ] `POST /api/auth/login` retorna `{ "status": "authenticated" }` con credenciales válidas
- [ ] `POST /api/auth/login` retorna 401 con credenciales inválidas
- [ ] `POST /api/auth/login` retorna `{ "status": "password_must_change" }` cuando aplica
- [ ] `POST /api/admin/setup` crea el Master Admin correctamente
- [ ] `POST /api/admin/setup-token` valida el OTP de instalación
- [ ] `POST /api/admin/tenant` crea un tenant y retorna `tenant_id` + `temporary_passphrase`
- [ ] `GET /api/admin/tenants` retorna lista de tenants (requiere auth admin)
- [ ] `DELETE /api/admin/tenant/:id` elimina el tenant
- [ ] `POST /api/admin/reset_password` resetea la password
- [ ] `GET /api/engine/status` retorna estado del engine
- [ ] `POST /api/engine/configure` configura el engine y persiste en `engine_config.json`
- [ ] Todos los endpoints usan `AegisHttpError` como tipo de error
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-Shell/bff/main.py` líneas 130–280 — implementación Python a reimplementar
`Aegis-Shell/bff/ank_client.py` — cliente gRPC que llama al kernel (ver los métodos que se invocan)
