# CORE-076 — Fix: `set_hw_profile` sin autenticación real

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

El endpoint `POST /api/system/hw_profile` delega a `set_hw_profile` en
`kernel/crates/ank-http/src/routes/engine.rs`:

```rust
pub async fn set_hw_profile(
    State(_state): State<AppState>,
    Json(body): Json<HwProfileRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    if body.admin_tenant_id != "root" {
        return Err(AegisHttpError::Kernel(
            "Only Master Admin can change HW profiles.".to_string(),
        ));
    }
    std::env::set_var("HW_PROFILE", &body.profile);
    Ok(Json(json!({ "success": true, "profile": body.profile })))
}
```

**Dos problemas críticos:**

1. **Sin verificación de contraseña:** La "validación" consiste únicamente en
   chequear si `admin_tenant_id == "root"`. No se valida ninguna contraseña ni
   session key. Cualquier cliente HTTP puede llamar a este endpoint con
   `{ "admin_tenant_id": "root", "profile": "3" }` y modificar el perfil de
   hardware sin ninguna credencial.

2. **Hardcodeo del nombre `"root"`:** Si el Master Admin se registró con un
   nombre distinto de `"root"` (ej: `"admin"`, `"superuser"`), el endpoint
   siempre rechaza sus requests legítimos — el administrador real no puede
   cambiar su propio perfil de hardware.

El struct `HwProfileRequest` ni siquiera tiene campo de contraseña:
```rust
pub struct HwProfileRequest {
    pub admin_tenant_id: String,
    pub profile: String,
    // ← falta session_key / contraseña
}
```

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/engine.rs`

### 1. Agregar `session_key` al request

```rust
#[derive(Deserialize)]
pub struct HwProfileRequest {
    pub admin_tenant_id: String,
    pub session_key: String,
    pub profile: String,
}
```

### 2. Validar contra `authenticate_master` del enclave

```rust
pub async fn set_hw_profile(
    State(state): State<AppState>,
    Json(body): Json<HwProfileRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let admin_hash = crate::citadel::hash_passphrase(&body.session_key);
    let citadel = state.citadel.lock().await;

    let is_auth = citadel
        .enclave
        .authenticate_master(&body.admin_tenant_id, &admin_hash)
        .await
        .map_err(|e| AegisHttpError::Kernel(e.to_string()))?;

    if !is_auth {
        return Err(AegisHttpError::Citadel(
            crate::citadel::CitadelError::Unauthorized,
        ));
    }

    // Validar perfil permitido
    if !["1", "2", "3"].contains(&body.profile.as_str()) {
        return Err(AegisHttpError::BadRequest("Invalid profile. Use 1, 2 or 3.".into()));
    }

    std::env::set_var("HW_PROFILE", &body.profile);

    Ok(Json(json!({ "success": true, "profile": body.profile })))
}
```

### 3. Actualizar `AdminDashboard.tsx` (Shell Engineer — coordinado)

El componente que llama a este endpoint debe incluir `session_key` en el body.
Abrir sub-task o coordinar con Shell Engineer al cerrar este ticket.

---

## Criterios de aceptación

- [ ] `HwProfileRequest` incluye campo `session_key`
- [ ] Se llama a `authenticate_master` antes de aplicar el cambio
- [ ] Un request sin `session_key` válida retorna 401
- [ ] Un request con `admin_tenant_id` distinto del admin real retorna 401
- [ ] Los perfiles válidos son `"1"`, `"2"`, `"3"` — cualquier otro retorna 400
- [ ] `cargo build` pasa sin errores

---

## Dependencias

Coordinación con Shell Engineer para actualizar el body del fetch en
`AdminDashboard.tsx` o el componente correspondiente que llama a este endpoint.
