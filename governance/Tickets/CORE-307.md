# CORE-307 — Relay server WebSocket tunnel protocol

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling  
**Estado:✅ Done
**Asignado a:** DevOps / Infra  

---

## Contexto

Para Aegis Connect, se requiere un protocolo de túnel WebSocket inverso para conectar instancias locales con URLs fijas y seguras a través de Orion ID.

## Cambios Realizados

- Protocolo de transporte y tramas de control para el servidor de relay.
- Heartbeats de mantenimiento y reconexión activa.
- Aislamiento de tenants en el lado del relay.
