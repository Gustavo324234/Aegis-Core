# CORE-073 — Fix: `sessionKey` (contraseña) persistida en localStorage

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

El store Zustand usa `persist` middleware para sobrevivir recargas de página.
El `partialize` actual incluye `sessionKey` en la serialización a `localStorage`:

```typescript
partialize: (state) => ({
    isAuthenticated: state.isAuthenticated,
    isAdmin: state.isAdmin,
    tenantId: state.tenantId,
    sessionKey: state.sessionKey,       // ← contraseña en texto plano en localStorage
    isEngineConfigured: state.isEngineConfigured,
    // ...
})
```

`sessionKey` es la contraseña del usuario en texto plano (el hash SHA-256 se
aplica en el backend, no antes de almacenar). Persistirla en `localStorage`
la expone a:
- Cualquier XSS en la app
- Extensiones de browser con acceso a `localStorage`
- DevTools de cualquier persona con acceso físico al browser

### Análisis de impacto

La razón por la que `sessionKey` se persiste es para mantener la sesión activa
tras un refresh de página — se usa en:
- `startTelemetryPolling` — header `x-citadel-key`
- `connect` / WebSocket — subprotocol `session-key.<key>`
- Operaciones admin — body / headers

La sesión de Aegis **no tiene tokens de sesión del lado servidor** (no hay JWT,
no hay session store). El `sessionKey` es la contraseña misma, usada como
credencial directa en cada request.

### Decisión de diseño

Dos opciones:

**Opción A (mínima — este ticket):** Eliminar `sessionKey` del `partialize`.
Al refrescar, el usuario ve la UI como desconectado y debe re-autenticarse.
`isAuthenticated: true` persiste pero sin `sessionKey` la conexión WS falla
limpiamente → App.tsx detecta `status === 'disconnected'` y muestra LoginScreen.

**Opción B (ideal — post-launch):** Implementar tokens de sesión del lado servidor
con TTL, almacenados en `sessionStorage` (no `localStorage`). Requiere cambios
en el Kernel (nuevo endpoint + tabla en SQLCipher). Ticket separado post-launch.

**Este ticket implementa Opción A.**

---

## Cambios requeridos

### `shell/ui/src/store/useAegisStore.ts`

1. Eliminar `sessionKey` del objeto `partialize`:

```typescript
partialize: (state) => ({
    isAuthenticated: state.isAuthenticated,
    isAdmin: state.isAdmin,
    tenantId: state.tenantId,
    // sessionKey: state.sessionKey,   ← ELIMINAR
    isEngineConfigured: state.isEngineConfigured,
    taskType: state.taskType,
    messages: state.messages,
    lastError: state.lastError,
    needsPasswordReset: state.needsPasswordReset,
    adminActiveTab: state.adminActiveTab,
    lastTenantsUpdate: state.lastTenantsUpdate,
})
```

2. En el efecto de reconexión en `App.tsx`, cuando `isAuthenticated === true`
   pero `sessionKey === null` tras hydration, limpiar el estado de auth y
   redirigir a login:

```typescript
// En App.tsx — useEffect tras hydration
useEffect(() => {
    if (_hydrated && isAuthenticated && !sessionKey) {
        logout();  // limpia estado, muestra LoginScreen
    }
}, [_hydrated, isAuthenticated, sessionKey]);
```

---

## Criterios de aceptación

- [ ] `sessionKey` no aparece en `localStorage` en ninguna circunstancia
- [ ] Al refrescar la página con sesión activa, el usuario es redirigido al login
- [ ] El login funciona normalmente después de la redirección
- [ ] No hay regresión en el flujo de autenticación normal
- [ ] `npm run build` pasa sin errores TypeScript

---

## Nota para el agente

NO eliminar `sessionKey` del estado en memoria (solo del `partialize`).
El estado en memoria durante la sesión activa es aceptable.
Solo el almacenamiento persistente entre sesiones es el problema.

---

## Dependencias

Ninguna. Independiente del resto de tickets.
