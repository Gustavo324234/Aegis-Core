# CORE-348 — Update Gemini Catalog with Official 3.5/3.6 Flash Production IDs

**Tipo:** fix
**Prioridad:** Alta
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

El catálogo de modelos (`models.yaml` y `syncer.rs`) necesitaba actualizarse con los identificadores de producción vigentes de la familia Gemini Flash y corregir advertencias de Clippy en fixtures de test.

## Solución propuesta

1. Actualizar `kernel/crates/ank-core/src/router/models.yaml` y `syncer.rs` con los IDs actualizados de Gemini.
2. Ajustar fixtures de pruebas en `enclave/mod.rs`, `plugins/mod.rs` y `syscalls/maker.rs` para mantener 0 warnings en CI.

## Criterios de aceptación

- [x] Catálogo `models.yaml` y `syncer.rs` actualizados y validados.
- [x] `cargo clippy` y tests pasando sin warnings.
