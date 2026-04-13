# CORE-072 — Fix: `isAdmin` determinado en el cliente por nombre de tenant

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer + Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

En `useAegisStore.ts`, la función `authenticate` determina si un usuario es admin
basándose en el nombre del tenant:

```typescript
set({
    isAdmin: tenantId.toLowerCase() === 'root' || tenantId.toLowerCase() === 'admin',
    ...
})
```

**Problemas:**
1. Cualquier usuario que se llame `admin` (o variante de mayúsculas) obtiene
   permisos de admin en el cliente, aunque el backend lo rechace.
2. La lógica está duplicada en el cliente — el backend ya conoce el rol real
   del tenant.
3. El `AdminDashboard` y sus operaciones destructivas (crear/eliminar tenants,
   resetear contraseñas) se muestran basándose en este flag client-side.

Nota: las operaciones del backend sí validan con `authenticate_master()`, por lo
que un "falso admin" no puede ejecutar operaciones destructivas. Pero la UI
muestra el dashboard de admin a cualquier tenant llamado `admin`, creando
confusión y potencial información leakeada.

---

## Cambios requeridos

### Kernel — `kernel/crates/ank-http/src/routes/auth.rs`

Incluir el rol del tenant en la respuesta de `/api/auth/login`:

```rust
// Respuesta actual:
json!({ "message": "...", "status": "authenticated" })

// Respuesta requerida:
json!({
    "message": "Citadel Handshake Successful",
    "status": "authenticated",
    "role": role_string  // "admin" | "tenant"
})
```

El `MasterEnclave` ya tiene información de rol — exponer `tenant.role` en
la respuesta de login.

### Shell — `shell/ui/src/store/useAegisStore.ts`

Reemplazar la lógica client-side de rol:

```typescript
// ANTES (eliminar):
isAdmin: tenantId.toLowerCase() === 'root' || tenantId.toLowerCase() === 'admin'

// DESPUÉS:
isAdmin: data.role === 'admin'
```

---

## Criterios de aceptación

- [ ] `/api/auth/login` retorna campo `role` en la respuesta JSON
- [ ] El store no determina `isAdmin` por nombre de tenant
- [ ] `isAdmin` se setea exclusivamente desde `data.role === 'admin'`
- [ ] Un tenant llamado `admin` sin rol admin en el enclave NO ve el AdminDashboard
- [ ] El tenant master/root SÍ ve el AdminDashboard tras el login
- [ ] `cargo build` pasa sin errores
- [ ] `npm run build` pasa sin errores TypeScript

---

## Dependencias

Ninguna. Verificar que `MasterEnclave::authenticate_tenant` retorna info de rol
o si hace falta un método separado `get_tenant_role`.
