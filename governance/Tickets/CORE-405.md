# CORE-405: Modularización de God Files (`chal/mod.rs`, `useAegisStore.ts`, `ModulesAppHub.tsx`, `VideoStudioPanel.tsx`, scripts Python externos)

- **Tipo:** refactor
- **Estado:** ✅ Done
- **Prioridad:** Media
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description

Dividir los archivos monolíticos más grandes en submódulos cohesivos, legibles y testeables:
- Rust: refactorizar `kernel/crates/ank-core/src/chal/mod.rs` (6,809 líneas) separando sanitizer, tool handlers, failover/recovery y tests en submódulos; modularizar `video_studio/runtime_deps.rs` extrayendo los scripts Python embebidos (`tts_edge.py`, `render_card.py`, `extract_ffmpeg.py`) a archivos reales bajo `resources/`/`tools/` validables con linter/tests; evaluar partición de `router/mod.rs` y `enclave/mod.rs`.
- Frontend: dividir `useAegisStore.ts` (2,340 líneas) en slices por dominio (`authSlice`, `chatSlice`, `agentsSlice`, `sirenSlice`, `adminSlice`) y descomponer `ModulesAppHub.tsx` y `VideoStudioPanel.tsx` en subcomponentes.

## Acceptance Criteria

- [x] `chal/mod.rs`, `useAegisStore.ts`, `ModulesAppHub.tsx` y `VideoStudioPanel.tsx` divididos en módulos cohesivos sin regresiones funcionales.
- [x] Scripts Python de Video Studio residen como archivos fuente reales testeados/embebidos con `include_str!`.
- [x] `cargo test --workspace`, `cargo clippy` y `npm run build` pasan sin regresiones.

## Verification

- **Scripts Video Studio**: `tts_edge.py`, `render_card.py` y `extract_ffmpeg.py` movidos a `kernel/crates/ank-core/resources/video_studio/` e inyectados con `include_str!` en `runtime_deps.rs` (`python -m py_compile` exit `0`).
- **Rust (`chal/`)**: `chal/mod.rs` (`7,462` → `2,970` líneas) dividido en `failover.rs` (`506`), `sanitizer.rs` (`171`), `prompt_helpers.rs` (`586`), `tool_handlers.rs` (`2,499`) y `tests.rs` (`751`).
- **Frontend Store (`shell/ui/src/store/`)**: `useAegisStore.ts` (`2,514` → `107` líneas) dividido en `types.ts`, `shared.ts`, `slices/authSlice.ts`, `slices/adminSlice.ts`, `slices/chatSlice.ts`, `slices/sirenSlice.ts` y `slices/agentsSlice.ts`.
- **Frontend UI**: `ModulesAppHub.tsx` (`2,388` → `686` líneas) en `modules/{types.ts, CreatorStudioTab.tsx, DevStudioTab.tsx, BusinessPosTab.tsx}` y `VideoStudioPanel.tsx` (`2,120` → `1,478` líneas) en `video_studio/{types.ts, useAuthenticatedBlobUrl.ts, MediaPreviews.tsx, ChatVideoResultCard.tsx}`.
- `cargo fmt --all -- --check`: OK (exit `0`).
- `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used -A clippy::double-must-use -A clippy::result-large-err`: OK (exit `0`).
- `cargo test --workspace`: **339 passed, 0 failed, 2 ignored** (exit `0`, incluyendo `ank-core` 269 passed / 2 ignored, `ank-http` 45 passed, `ank-mcp` 7 passed, `ank-server` 3 passed, `ank-cli` 4 passed, `aegis-supervisor` 2 passed, `aegis-connect-relay` 2 passed, plugins 7 passed).
- `npx tsc --noEmit`, `npm run lint` (`--max-warnings 0`) y `npm run build` (`vite build`): OK (exit `0`, 0 warnings, chunk inicial `68.22 kB`, chunk JS máximo `markdown-vendor` `153.42 kB`).
