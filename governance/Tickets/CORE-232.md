# CORE-232 — fix(shell): IP vs Cloudflare producen chats separados — misma sesión, contexto diferente

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

Al acceder por IP directa (`http://192.168.1.6:8000`) después de haber usado Cloudflare
(`https://xxx.trycloudflare.com`), el usuario ve un chat diferente: sin historial de la
sesión anterior, como si fuera un usuario distinto.

## Causa raíz

Son **mismos datos en el backend** — el servidor es uno solo. El problema es el cliente:

1. El store persiste `messages` en `localStorage` bajo la key `aegis-storage`.
   `localStorage` es por **origen** (protocolo + dominio + puerto). `https://xxx.trycloudflare.com`
   y `http://192.168.1.6:8000` son **orígenes distintos** → dos `localStorage` distintos →
   dos historiales de mensajes distintos.

2. `sessionKey` no se persiste (correcto por seguridad). Al acceder por IP, el store
   se rehidrata con `isAuthenticated: true` pero `sessionKey: null`, y el `useEffect`
   en `App.tsx` fuerza `logout()`. El usuario re-loguea desde IP y empieza con historia
   limpia.

El historial en `localStorage` por Cloudflare no está disponible desde IP — son storages
aislados por diseño del navegador.

## Cambios requeridos

### 1. Informar al usuario que el historial local es por origen

**Archivo:** `shell/ui/src/components/ChatTerminal.tsx` (o donde esté el header del chat)

Agregar un indicador visible cuando el usuario está accediendo por HTTP/IP:

```
⚠️  Acceso local (HTTP). El historial de Cloudflare no está disponible aquí.
    Para historial unificado, usá siempre el link de Cloudflare.
```

Mostrar esto como un banner dismissable en la parte superior del chat, solo cuando
`window.location.protocol === 'http:'` y el host no es localhost.

### 2. Persistir el historial de mensajes en el servidor, no solo en localStorage

**Nota de arquitectura para el Kernel Engineer:**

El historial de conversación debería guardarse en la `TenantDB` del kernel (SQLCipher),
no solo en `localStorage` del cliente. Esto permitiría que cualquier origen (IP, Cloudflare,
app móvil) vea el mismo historial al autenticarse.

Este es un cambio de arquitectura mayor (requiere nuevo endpoint + schema). Se crea como
ticket separado (CORE-233) para no bloquear el fix de UX inmediato.

### 3. Fix del ciclo de logout en rehidratación

**Archivo:** `shell/ui/src/App.tsx`

El `useEffect` actual:
```typescript
if (_hydrated && isAuthenticated && !sessionKey) {
    logout();
}
```

Este logout forzado está bien por seguridad, pero borra `isAdmin` y `tenantId` del store.
Al llegar a LoginScreen desde esta IP, el usuario no tiene ningún hint de con qué tenant
loguearse.

Mejorar: al forzar logout por sessionKey null, guardar el `tenantId` en un estado local
del componente (`useState`) en `App.tsx` para pre-rellenar el campo de login con el
último tenant usado.

## Criterios de aceptación

- [x] Al acceder por HTTP/IP, el usuario ve un banner informativo (dismissable) que explica la situación
- [x] Al hacer re-login desde IP, el campo de tenant viene pre-rellenado con el último `tenantId` conocido
- [x] No hay pérdida de datos — el historial de Cloudflare sigue en su localStorage de Cloudflare

## Dependencias

CORE-233 (historial en servidor) es una mejora futura independiente. Este ticket es solo
el fix de UX inmediato.
