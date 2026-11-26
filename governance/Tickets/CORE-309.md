# CORE-309 — Shell: Connect status widget & Orion ID linking

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling  
**Estado:✅ Done
**Asignado a:** Shell Engineer  

---

## Contexto

La interfaz web (React) debe mostrar el estado actual del túnel Aegis Connect y proveer un mecanismo intuitivo para vincular la instancia con una cuenta de Orion ID.

## Cambios Realizados

- Componente visual `ConnectStatusWidget` en la barra del sistema.
- Formulario de vinculación con flujo de autenticación contra el servidor Orion ID.
- Gestión de estados de conexión (Conectando, Conectado, Error, Desconectado) en Zustand store.
