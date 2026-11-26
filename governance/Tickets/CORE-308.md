# CORE-308 — Client agent integration & heartbeat

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling  
**Estado:✅ Done
**Asignado a:** Kernel Engineer  

---

## Contexto

El kernel local necesita actuar como cliente del túnel inverso WebSocket persistente, reconectándose automáticamente y respondiendo a los pings/heartbeats del relay.

## Cambios Realizados

- Integración del cliente WebSocket en el kernel (`ank-core`).
- Heartbeat loop con re-intentos exponenciales.
- Enrutamiento correcto de peticiones remotas entrantes a través del WebSocket.
