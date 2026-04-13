# CORE-070 — Fix: WebSocket URL hardcodeada con puerto 8000

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

En `shell/ui/src/store/useAegisStore.ts`, las URLs de WebSocket para chat y Siren
están construidas con `window.location.hostname` + puerto hardcodeado `:8000`:

```typescript
// chat (línea ~218)
const wsUrl = `ws://${window.location.hostname}:8000/ws/chat/${encodeURIComponent(tenantId)}`;

// siren (línea ~290 aprox)
const wsUrl = `ws://${window.location.hostname}:8000/ws/siren/${encodeURIComponent(tenantId)}`;
```

Esto rompe en cualquier despliegue donde la UI no se acceda por el puerto 8000 directamente:
- Reverse proxy (nginx :80 / :443)
- Cualquier PORT remapeado
- Acceso HTTPS (debería ser `wss://`)

`window.location.host` incluye el puerto correcto del browser (`hostname:port` o solo `hostname` si es 80/443).
`window.location.protocol` permite derivar `ws://` vs `wss://` automáticamente.

---

## Cambios requeridos

**Archivo:** `shell/ui/src/store/useAegisStore.ts`

Reemplazar la construcción manual de URL WebSocket por una función helper que use
el origen actual del browser:

```typescript
function buildWsUrl(path: string): string {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${proto}//${window.location.host}${path}`;
}
```

Aplicar en:
1. `connect()` — `/ws/chat/{tenant_id}`
2. `startSirenStream()` — `/ws/siren/{tenant_id}`

---

## Criterios de aceptación

- [ ] No hay ninguna referencia a puerto `:8000` hardcodeado en `useAegisStore.ts`
- [ ] La función helper `buildWsUrl` (o equivalente inline) usa `window.location.host` y `window.location.protocol`
- [ ] `npm run build` pasa sin errores TypeScript
- [ ] `npm run lint` pasa sin warnings

---

## Dependencias

Ninguna.
