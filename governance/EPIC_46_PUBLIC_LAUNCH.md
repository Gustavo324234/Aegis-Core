# EPIC 46 — Public Launch

**Objetivo:** Dejar Aegis-Core en condición de lanzamiento público open source.
El repo debe ser encontrable, comprensible, contribuible y con infraestructura de comunidad mínima lista.

**Precondición:** Epic Opción A (documentación) completada — ✅ Done (2026-04-28)

---

> **🔄 Reconciliación (2026-05-31 — Arquitecto IA):** Este doc estaba 100% en `📥 Todo` mientras `TICKETS_MASTER.md` lo marcaba "✅ Completa 100%". Se corrigieron los estados contra la realidad verificable (existencia de archivos en el repo + `Cargo.toml`). Resultado: la mayoría de los entregables existen, pero **el epic NO está 100%** — hay items sin verificar (release, topics, og:image, license) y uno sin hacer (`scratch/` sigue presente). Lo que requiere acción de GitHub (settings) lo marca el Owner manualmente.

---

## Áreas de trabajo

| Área | Tickets | Responsable |
|---|---|---|
| A — Community Health | CORE-214, CORE-215, CORE-216 | Arquitecto IA |
| B — GitHub Issue Templates | CORE-217, CORE-218 | Arquitecto IA |
| C — GitHub Sponsors | CORE-219 | Tavo (manual) |
| D — Release & Visibility | CORE-220, CORE-221, CORE-222 | DevOps / Arquitecto IA |
| E — Repo Hygiene | CORE-223, CORE-224, CORE-225 | Arquitecto IA |

---

## Tickets

### Área A — Community Health

| ID | Título | Estado | Evidencia |
|---|---|---|---|
| CORE-214 | CODE_OF_CONDUCT.md | ✅ Done | `CODE_OF_CONDUCT.md` presente en raíz |
| CORE-215 | SECURITY.md — política de reporte de vulnerabilidades | ✅ Done | `SECURITY.md` presente en raíz |
| CORE-216 | CHANGELOG.md — historial de versiones público | ✅ Done | `CHANGELOG.md` presente en raíz |

---

### Área B — GitHub Issue Templates

| ID | Título | Estado | Evidencia |
|---|---|---|---|
| CORE-217 | Issue template: Bug Report | ✅ Done | `.github/ISSUE_TEMPLATE/bug_report.yml` |
| CORE-218 | Issue template: Feature Request | ✅ Done | `.github/ISSUE_TEMPLATE/feature_request.yml` |

---

### Área C — GitHub Sponsors

| ID | Título | Estado | Evidencia |
|---|---|---|---|
| CORE-219 | Habilitar GitHub Sponsors + redactar sponsor page | ✅ Done (entregables) | `.github/FUNDING.yml` + `docs/SPONSOR_PAGE.md` presentes. Habilitación de la cuenta Sponsors = acción manual del Owner en GitHub. |

---

### Área D — Release & Visibility

| ID | Título | Estado | Nota |
|---|---|---|---|
| CORE-220 | Crear release v1.0.0 en GitHub con release notes | ⚠️ Verificar | No verificable desde el repo. El installer baja `nightly` por default → confirmar si existe una release `v1.0.0` estable en GitHub Releases. |
| CORE-221 | Agregar topics al repo de GitHub | ⚠️ Verificar | Setting de GitHub (manual) — no verificable desde archivos. |
| CORE-222 | Social preview image (og:image del repo) | ⚠️ Verificar | Setting/asset de GitHub (manual) — no verificable desde archivos. |

---

### Área E — Repo Hygiene

| ID | Título | Estado | Nota |
|---|---|---|---|
| CORE-223 | .github/CODEOWNERS — definir owner para cada área | ✅ Done | `.github/CODEOWNERS` presente |
| CORE-224 | Archivar rama scratch/ y limpiar target_verify_fix/ | ❌ No hecho | `scratch/` **sigue presente** en el repo. Revisar y cerrar. |
| CORE-225 | Sincronizar licencia: README menciona MIT, verificar Cargo.toml | ⚠️ Verificar | El `Cargo.toml` raíz **no tiene** campo `license` ni `[workspace.package]`. Confirmar si cada crate declara `license = "MIT"` o agregarlo a nivel workspace con `license.workspace = true`. |

---

## Criterio de completitud del Epic

El Epic 46 está completo cuando:
- [x] `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CONTRIBUTING.md` presentes en raíz
- [x] Issue templates activos en `.github/ISSUE_TEMPLATE/`
- [~] GitHub Sponsors habilitado con descripción del proyecto *(entregables listos; habilitación manual del Owner)*
- [ ] Release v1.0.0 publicada en GitHub Releases con release notes *(⚠️ verificar)*
- [ ] Repo tiene topics relevantes (cognitive-os, rust, ai-agents, self-hosted, llm) *(⚠️ verificar — manual)*
- [ ] `license` field en todos los `Cargo.toml` dice `MIT` *(⚠️ verificar — falta a nivel workspace)*
- [ ] No hay directorios de trabajo temporales en raíz del repo *(❌ `scratch/` presente)*

**Estado real:** ~70% verificado. Items abiertos: CORE-220, CORE-221, CORE-222, CORE-224, CORE-225.

---

*Arquitecto IA — 2026-04-28 · Reconciliado 2026-05-31*
