# CORE-079 — Fix: `SystemTab` pasa `session_key` en query param de `/api/status`

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

El componente `SystemTab` en `AdminDashboard.tsx` hace polling de telemetría
con las credenciales en la URL:

```typescript
const response = await fetch(
    `/api/status?tenant_id=${encodeURIComponent(tenantId)}&session_key=${encodeURIComponent(sessionKey)}`
);
```

Dos problemas:

1. **`session_key` en query param** — igual que el bug BUG-5 de `fetchTenants`.
   La contraseña aparece en logs del servidor Axum, historial del browser y
   headers `Referer`.

2. **El endpoint `/api/status` espera `x-citadel-key` en headers** (ver
   `routes/status.rs`), no `session_key` en query string. Esto significa que
   el polling del `SystemTab` del AdminDashboard **falla silenciosamente** —
   devuelve 401 y las métricas de sistema nunca se muestran al admin.

El store `useAegisStore.startTelemetryPolling` ya usa el patrón correcto:
```typescript
headers: { 'x-citadel-key': sessionKey }
```
El `SystemTab` tiene su propio fetch duplicado que no lo aplica.

---

## Cambios requeridos

**Archivo:** `shell/ui/src/components/AdminDashboard.tsx`

Corregir el fetch en `SystemTab`:

```typescript
const response = await fetch(
    `/api/status?tenant_id=${encodeURIComponent(tenantId)}`,
    { headers: { 'x-citadel-key': sessionKey } }
);
```

Eliminar `session_key` del query string completamente.

---

## Criterios de aceptación

- [ ] El fetch de `SystemTab` no incluye `session_key` en la URL
- [ ] La request usa header `x-citadel-key` para la autenticación
- [ ] Las métricas de sistema se muestran correctamente en el AdminDashboard
- [ ] `npm run build` pasa sin errores TypeScript

---

## Dependencias

Ninguna. Cambio de 3 líneas.
