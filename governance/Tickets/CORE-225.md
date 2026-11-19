# CORE-225 — License field en Cargo.toml → MIT

**Tipo:** chore  
**Prioridad:** Alta  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer  

---

## Contexto

El `Cargo.toml` de la raíz del monorepo no declara explícitamente el campo `license` en un bloque `[workspace.package]`. Es necesario verificar que todos los crates del workspace declaren la licencia `MIT` o consolidarlo a nivel de workspace para mayor higiene de código abierto.

## Tareas

- Verificar que los Cargo.toml de todos los crates secundarios tengan el campo `license = "MIT"`.
- Incorporar `license = "MIT"` a nivel de `workspace` si es soportado.
