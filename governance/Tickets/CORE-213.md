# CORE-213 — Kernel: loguear error en key_pool.load()

**Tipo:** fix  
**Prioridad:** Media  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:✅ Done
**Asignado a:** Kernel Engineer  

---

## Contexto

Cuando el `KeyPool` fallaba al cargar las API keys cifradas del tenant desde la base de datos (por ejemplo, por una contraseña incorrecta o base de datos corrupta), fallaba silenciosamente sin registrar el error específico en los logs del SRE.

## Cambios Realizados

- Agregados logs detallados en `ank-core/src/security/key_pool.rs` con nivel `ERROR` cuando `key_pool.load()` falla.
- Preservada la privacidad de la clave en los logs de error (solo se loguea la causa raíz del fallo de descifrado, no el valor de la clave).
