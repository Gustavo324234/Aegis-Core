# CORE-347 — Clear Local Messages State on Tenant Delete and User Authentication

**Tipo:** fix
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Shell Engineer

---

## Problema

En `useAegisStore.ts`, al autenticar un nuevo usuario o eliminar un tenant desde la interfaz, el arreglo `messages` en memoria del cliente permanecía poblado temporalmente con los mensajes de la sesión anterior.

## Solución propuesta

1. Limpiar `messages: []` dentro de `useAegisStore` al completar `deleteTenant` y durante las transiciones de `authenticate`.

## Criterios de aceptación

- [x] `deleteTenant` y `authenticate` reinician `messages: []` inmediatamente en el store de Zustand.
