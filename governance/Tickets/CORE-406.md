# CORE-406: Tests UI (Vitest), CI Extendido (`app/`, bridges, `update.ps1`, `sync_tickets_master.py`) y Actualización/Protección de OpenAPI (`/api/docs`)

- **Tipo:** chore
- **Estado:** ✅ Done
- **Prioridad:** Media
- **Épica:** EPIC 61
- **Asignado a:** Shell Engineer + DevOps Engineer

---

## Description

Cerrar las brechas de cobertura de pruebas, validación en CI y documentación de API:
- Incorporar Vitest + React Testing Library en `shell/ui` con pruebas unitarias iniciales para `useAegisStore`, parsing de `[AEGIS_VIDEO_CARD]` y componentes críticos.
- Ampliar `.github/workflows/pr_check.yml` para validar `app/` (lint/tsc), `tools/whatsapp_bridge` (`node --check`), `tools/antigravity_bridge` (`unittest`), `update.ps1` (check BOM + PowerShell 5.1) y `python tools/sync_tickets_master.py --report`.
- Actualizar `ank-http/src/routes/openapi.rs` (versión actual, enlaces al repo correcto y registro completo de rutas activas) y restringir `/api/docs` / `/api-docs/openapi.json` en producción detrás de autenticación o flag explícito.

## Acceptance Criteria

- [x] `shell/ui` ejecuta suite de tests en CI.
- [x] `pr_check.yml` valida bridges, app mobile, `update.ps1` y gobernanza anti-drift.
- [x] OpenAPI refleja todos los endpoints actuales y `/api/docs` queda protegido en producción.

## Verification

- `shell/ui`: `npm test` (`vitest run`: `4` archivos, `14` pruebas passing), `npm run lint` (`0` warnings) y `npm run build` (`tsc && vite build`, exit `0`).
- Kernel Rust: `cargo fmt --all -- --check` (exit `0`), `cargo clippy --workspace` con flags CI (exit `0`) y `cargo test --workspace` (exit `0`: `ank-http` `47` passed incl. `openapi_docs_exposure_follows_debug_and_explicit_override` y `openapi_spec_contains_updated_routes_and_repo_url`; `ank-core` `269` passed / `2` ignored `test_espeak_synthesize` y `test_live_free_video_render_pipeline`; resto de crates passing).
- Bridges, Video Studio y PowerShell 5.1: `node --check tools/whatsapp_bridge/index.mjs`, `python -m py_compile` (bridges y `video_studio/*.py`), `python -m unittest discover -s tools/antigravity_bridge -p "test_*.py"` (`Ran 1 test ... OK`) y validación UTF-8 BOM + `Parser::ParseFile` en `installer/aegis.ps1`, `installer/install.ps1` y `update.ps1` (`Parse OK`).
- Nota CI: el job `mobile-app` agregado a `.github/workflows/pr_check.yml` (`cd app && npm ci`, `npx tsc --noEmit`, `npm run lint`) se validará en runner de GitHub Actions (`app/node_modules` no instalado localmente).
