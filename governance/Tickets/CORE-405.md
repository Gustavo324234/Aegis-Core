# CORE-405: Modularización de God Files (`chal/mod.rs`, `useAegisStore.ts`, `ModulesAppHub.tsx`, `VideoStudioPanel.tsx`, scripts Python externos)

- **Tipo:** refactor
- **Estado:** 📥 Todo
- **Prioridad:** Media
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description

Dividir los archivos monolíticos más grandes en submódulos cohesivos, legibles y testeables:
- Rust: refactorizar `kernel/crates/ank-core/src/chal/mod.rs` (6,809 líneas) separando sanitizer, tool handlers, failover/recovery y tests en submódulos; modularizar `video_studio/runtime_deps.rs` extrayendo los scripts Python embebidos (`tts_edge.py`, `render_card.py`, `extract_ffmpeg.py`) a archivos reales bajo `resources/`/`tools/` validables con linter/tests; evaluar partición de `router/mod.rs` y `enclave/mod.rs`.
- Frontend: dividir `useAegisStore.ts` (2,340 líneas) en slices por dominio (`authSlice`, `chatSlice`, `agentsSlice`, `sirenSlice`, `adminSlice`) y descomponer `ModulesAppHub.tsx` y `VideoStudioPanel.tsx` en subcomponentes.

## Acceptance Criteria

- [ ] `chal/mod.rs`, `useAegisStore.ts`, `ModulesAppHub.tsx` y `VideoStudioPanel.tsx` divididos en módulos cohesivos sin regresiones funcionales.
- [ ] Scripts Python de Video Studio residen como archivos fuente reales testeados/embebidos con `include_str!`.
- [ ] `cargo test --workspace`, `cargo clippy` y `npm run build` pasan sin regresiones.
