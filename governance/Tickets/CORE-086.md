# CORE-086 — Fix: `router_api.rs` — `add_global_key` valida admin por nombre `"root"` hardcodeado sin auth real

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

`POST /api/router/keys/global` en `router_api.rs` tiene dos problemas
de autenticación encadenados:

### Problema 1 — Validación por nombre hardcodeado

```rust
if req.tenant_id != "root" {
    return Err(AegisHttpError::Kernel("Only Master Admin can manage global keys".into()));
}
```

Igual al patrón de CORE-072 y CORE-076 — rechaza a cualquier admin cuyo
nombre no sea exactamente `"root"`.

### Problema 2 — Autenticación contra `authenticate_tenant` en lugar de `authenticate_master`

```rust
citadel.enclave
    .authenticate_tenant(&req.tenant_id, &hash)  // ← INCORRECTO
    .await
```

El Master Admin vive en la tabla `master_admin`. `authenticate_tenant` busca
en la tabla `tenants`. Si el admin se creó solo como master (no como tenant),
**este check siempre falla con `QueryReturnedNoRows`** → 401 aunque las
credenciales sean correctas.

`list_global_keys` y `delete_global_key` usan el extractor `CitadelAuthenticated`
que llama a `authenticate_tenant` también — mismo problema.

### Situación actual

Dado que `ProvidersTab.tsx` enviaba credenciales hardcodeadas `'admin'`/`'session'`
(CORE-083), este endpoint nunca se llamó con credenciales reales. Al corregir
CORE-083, este bug se vuelve bloqueante.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/router_api.rs`

### 1. Reemplazar validación por nombre con `authenticate_master`

```rust
async fn add_global_key(
    State(state): State<AppState>,
    Json(req): Json<KeyAddRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let hash = hash_passphrase(&req.session_key);
    {
        let citadel = state.citadel.lock().await;
        let is_master = citadel
            .enclave
            .authenticate_master(&req.tenant_id, &hash)
            .await
            .map_err(|_| AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized))?;

        if !is_master {
            return Err(AegisHttpError::Citadel(crate::citadel::CitadelError::Unauthorized));
        }
    }
    // ... resto igual ...
}
```

### 2. Migrar `list_global_keys` y `delete_global_key` a un helper de auth admin

Crear un helper privado `require_master_auth(state, headers)` que:
- Lea `x-citadel-tenant` y `x-citadel-key` de headers
- Llame a `authenticate_master` (no `authenticate_tenant`)
- Retorne `(tenant_id, hash)` o `AegisHttpError::Citadel(Unauthorized)`

Reemplazar el extractor `CitadelAuthenticated` en estas rutas por el helper,
ya que `CitadelAuthenticated` solo llama a `authenticate_tenant`.

### 3. Actualizar `KeyAddRequest`

Agregar `session_key` a headers en lugar de body (alineado con CORE-071/083):

```rust
// El session_key viene de headers x-citadel-key, no del body
// Mantener body solo para datos del provider:
pub struct KeyAddRequest {
    pub provider: String,
    pub api_key: String,
    pub api_url: Option<String>,
    pub label: Option<String>,
}
```

---

## Criterios de aceptación

- [ ] `add_global_key` llama a `authenticate_master`, no `authenticate_tenant`
- [ ] No hay comparación de `tenant_id` con el string `"root"` hardcodeado
- [ ] Un Master Admin con nombre distinto de `"root"` puede agregar keys globales
- [ ] Credenciales incorrectas retornan 401
- [ ] `cargo build` pasa sin errores

---

## Dependencias

CORE-083 — el frontend debe enviar credenciales reales antes de que este fix sea testeable.
