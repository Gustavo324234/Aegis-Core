# CORE-124 — Fix: Historial del chat se borra al recargar

**Status:** DONE — 2026-04-20

## Síntoma

Al recargar la página (F5) o al cerrar y abrir el browser, el historial
del chat desaparecía completamente.

## Causa raíz

`logout()` en el store incluía `messages: []`, lo que borraba el historial.
Como `sessionKey` no se persiste (CORE-073, correcto por seguridad),
al recargar el store detectaba `isAuthenticated=true` pero `sessionKey=null`
y ejecutaba `logout()` — borrando los mensajes.

## Fix

1. `logout()` ya no incluye `messages: []` — el historial se preserva.
   Solo se limpia la sesión (socket, credenciales, estado de auth).

2. Los mensajes tipo `syslog` ("Aegis Shell established secure bridge...") ya
   no se agregan al historial persistido — evita que se acumulen con cada
   reconexión.

3. `STORE_VERSION` bumpeado de 3 → 4 para migrar stores viejos que pudieran
   tener `sessionKey` en localStorage.

4. `clearHistory()` sigue disponible como acción explícita del usuario.

## Flujo post-fix

```
F5 / recargar
  → store hidrata: isAuthenticated=true, tenantId="Tavo", sessionKey=null
  → App.tsx detecta sessionKey=null → redirige al login (NO borra mensajes)
  → usuario ingresa password → sessionKey se restaura en memoria
  → WebSocket reconecta → historial sigue visible
```
