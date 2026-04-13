# CORE-071 — Fix: Credenciales admin en query params — migrar a headers Citadel

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

Los endpoints de administración de tenants tienen un patrón de autenticación
inconsistente. Algunos pasan las credenciales del admin en **query params de la URL**,
lo que las expone en logs del servidor, historial del browser y headers `Referer`.

### Violaciones actuales en `useAegisStore.ts`

**`fetchTenants`** — credenciales en query string:
```typescript
fetch(`/api/admin/tenants?admin_tenant_id=${encodeURIComponent(tenantId)}&admin_session_key=${encodeURIComponent(sessionKey)}`)
```

**`deleteTenant`** — usa `x-citadel-key` pero pasa `admin_tenant_id` como query param:
```typescript
fetch(`/api/admin/tenant/${id}?admin_tenant_id=${encodeURIComponent(tenantId)}`, {
    headers: { 'x-citadel-key': sessionKey }
})
```

El Protocolo Citadel documentado en `CLAUDE.md` define:
```
Headers HTTP: x-citadel-tenant + x-citadel-key
```

### Estado del backend (`admin.rs`)

El backend **sí acepta credenciales en headers** para `delete_tenant_path`:
```rust
Query(query): Query<AdminAuthQuery>  // admin_tenant_id + admin_session_key en query
```

El backend necesita ser actualizado para aceptar credenciales vía headers en los
endpoints que actualmente usan `Query<AdminAuthQuery>`.

---

## Cambios requeridos

### Shell — `shell/ui/src/store/useAegisStore.ts`

Unificar **todos** los fetch de admin para usar headers Citadel:

```typescript
// Patrón correcto — aplicar en fetchTenants, deleteTenant
headers: {
    'Content-Type': 'application/json',
    'x-citadel-tenant': tenantId,
    'x-citadel-key': sessionKey,
}
```

Funciones afectadas: `fetchTenants`, `deleteTenant`.  
`createTenant` y `resetPassword` ya usan body JSON — solo agregar headers.

### Kernel — `kernel/crates/ank-http/src/routes/admin.rs`

Actualizar `list_tenants` y `delete_tenant_path` para leer auth de headers
en lugar de (o además de) query params:

```rust
// Agregar extractor de headers
headers: HeaderMap,
// Leer x-citadel-tenant y x-citadel-key
// Eliminar AdminAuthQuery de list_tenants y delete_tenant_path
```

Agregar un helper privado `extract_admin_auth(headers: &HeaderMap)` que retorne
`(admin_tenant_id, admin_session_key)` o `AegisHttpError::Citadel(MissingKey)`.

---

## Criterios de aceptación

- [ ] Ningún endpoint admin pasa credenciales en query string
- [ ] `fetchTenants` usa `x-citadel-tenant` / `x-citadel-key` en headers
- [ ] `deleteTenant` usa `x-citadel-tenant` / `x-citadel-key` en headers
- [ ] `createTenant` y `resetPassword` incluyen headers Citadel
- [ ] Backend `list_tenants` y `delete_tenant_path` leen auth de headers
- [ ] `cargo build` pasa sin errores
- [ ] `npm run build` pasa sin errores TypeScript

---

## Dependencias

Ninguna. Puede implementarse en paralelo con CORE-070.
