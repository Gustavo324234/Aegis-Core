# CORE-407: Higiene del Repo, Modularización Focalizada (`router/scoring.rs`, `chal/context_builder.rs`) y Mapa de Arquitectura

- **Tipo:** chore
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 62
- **Asignado a:** Kernel Engineer

---

## Description

Reducir la fricción estructural y el desperdicio de contexto en el repositorio:
- Eliminar copias huérfanas en `scratch/isolated_workspace/` y directorios residuales `kernel/crates/ank-core/users/test_*` / `vid_*`.
- Aislar los tests de `kernel/crates/ank-core/src/enclave/mod.rs` con `ScopedTestDataDir` para no ensuciar `./users/`.
- Extraer la lógica de puntuación del router a `kernel/crates/ank-core/src/router/scoring.rs` y el constructor de mensajes/contexto a `kernel/crates/ank-core/src/chal/context_builder.rs`.
- Documentar un mapa rápido de subsistemas en `CLAUDE.md` y refrescar el grafo `graphify`.

## Acceptance Criteria

- [x] `scratch/isolated_workspace/` y bases residuales de tests eliminados; tests de `enclave` aislados con `ScopedTestDataDir`.
- [x] `router/scoring.rs` y `chal/context_builder.rs` compilando sin cambios de firma pública en `CognitiveRouter` ni `CognitiveHAL`.
- [x] Mapa rápido actualizado en `CLAUDE.md`.
