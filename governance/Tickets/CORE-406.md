# CORE-406: Tests UI (Vitest), CI Extendido (`app/`, bridges, `update.ps1`, `sync_tickets_master.py`) y Actualización/Protección de OpenAPI (`/api/docs`)

- **Tipo:** chore
- **Estado:** 📥 Todo
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

- [ ] `shell/ui` ejecuta suite de tests en CI.
- [ ] `pr_check.yml` valida bridges, app mobile, `update.ps1` y gobernanza anti-drift.
- [ ] OpenAPI refleja todos los endpoints actuales y `/api/docs` queda protegido en producción.
