# CORE-083 — Fix: `ProvidersTab` usa credenciales hardcodeadas y query params para el KeyPool

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

`ProvidersTab.tsx` tiene **tres** violaciones al Protocolo Citadel en sus
llamadas al KeyPool:

### Violación 1 — `handleSave`: credenciales hardcodeadas

```typescript
body: JSON.stringify({
    tenant_id: 'admin',     // ← HARDCODEADO
    session_key: 'session', // ← HARDCODEADO (ni siquiera es real)
    provider: selectedProvider,
    api_key: apiKey,
    ...
})
```

El endpoint `POST /api/router/keys/global` recibe credenciales de admin
hardcodeadas. El backend extrae auth de headers (`x-citadel-tenant` /
`x-citadel-key`), así que esta llamada falla con 401 silenciosamente o
pasa sin autenticarse dependiendo de la implementación del endpoint.

### Violación 2 — `fetchProviders`: session_key en query param

```typescript
fetch(`/api/router/keys/global?tenant_id=${tenantId}&session_key=${sessionKey}`)
```

Igual que los bugs CORE-071 y CORE-079 — la contraseña viaja en la URL.

### Violación 3 — `handleDelete`: session_key en query param

```typescript
fetch(`/api/router/keys/global/${keyId}?tenant_id=${tenantId}&session_key=${sessionKey}`, {
    method: 'DELETE'
})
```

Mismo problema.

---

## Cambios requeridos

**Archivo:** `shell/ui/src/components/ProvidersTab.tsx`

### 1. `handleSave` — usar credenciales reales del store con headers

```typescript
const handleSave = async () => {
    // tenantId y sessionKey vienen como props
    const res = await fetch('/api/router/keys/global', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'x-citadel-tenant': tenantId!,
            'x-citadel-key': sessionKey!,
        },
        body: JSON.stringify({
            provider: selectedProvider,
            api_key: apiKey,
            api_url: PROVIDER_PRESETS[selectedProvider].url,
            models: selectedModels
        })
    });
};
```

### 2. `fetchProviders` — migrar a headers

```typescript
const res = await fetch('/api/router/keys/global', {
    headers: {
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }
});
```

### 3. `handleDelete` — migrar a headers

```typescript
const res = await fetch(`/api/router/keys/global/${keyId}`, {
    method: 'DELETE',
    headers: {
        'x-citadel-tenant': tenantId!,
        'x-citadel-key': sessionKey!,
    }
});
```

---

## Criterios de aceptación

- [ ] Ninguna llamada de `ProvidersTab` incluye `session_key` en query params o body
- [ ] Ninguna llamada usa credenciales hardcodeadas (`'admin'`, `'session'`)
- [ ] Las tres operaciones (fetch, save, delete) usan headers `x-citadel-tenant` / `x-citadel-key`
- [ ] `npm run build` pasa sin errores TypeScript

---

## Dependencias

Verificar que el endpoint `POST /api/router/keys/global` en `router_api.rs`
lea las credenciales de headers (no de body). Si no, coordinar con Kernel
Engineer para alinear.
