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
| CORE-219 | Habilitar GitHub Sponsors + redactar sponsor page | ✅ Done | `.github/FUNDING.yml` + `docs/SPONSOR_PAGE.md` presentes. Habilitación de la cuenta Sponsors confirmada por el Owner el 2026-06-12. |

---

### Área D — Release & Visibility

| ID | Título | Estado | Nota |
|---|---|---|---|
| CORE-220 | Crear release estable en GitHub con release notes | ✅ Done | Release **v0.2.0 — Public Beta** publicada el 2026-06-12 (decisión del Owner: versión honesta 0.x en lugar de v1.0.0). El tag `v*` dispara `publish-native`, que adjunta los binarios de todas las plataformas (incl. `ank-cli`) sin marca de prerelease; marcada `latest` para que `aegis update --stable` la resuelva. |
| CORE-221 | Agregar topics al repo de GitHub | ✅ Done | Verificado vía API el 2026-06-12: `cognitive-os, rust, ai-agents, axum, llm, multi-agent, opensource, self-hosted, tokio` (los 5 requeridos + 4 extra). |
| CORE-222 | Social preview image (og:image del repo) | ✅ Done | Subida por el Owner el 2026-06-12 (Settings → Social preview), confirmado el mismo día del lanzamiento v0.2.0. |

---

### Área E — Repo Hygiene

| ID | Título | Estado | Nota |
|---|---|---|---|
| CORE-223 | .github/CODEOWNERS — definir owner para cada área | ✅ Done | `.github/CODEOWNERS` presente |
| CORE-224 | Archivar rama scratch/ y limpiar target_verify_fix/ | ✅ Done | Verificado 2026-06-12: `scratch/` está en `.gitignore` con **0 archivos trackeados** (`git ls-files scratch` vacío) — no existe en el repo publicado; es solo un workspace local. |
| CORE-225 | Sincronizar licencia: README menciona MIT, verificar Cargo.toml | ✅ Done | Verificado 2026-06-12: `[workspace.package] license = "MIT"` en el Cargo.toml raíz (PR #333) y los 9 crates declaran `license = "MIT"` explícito. |

---

## Criterio de completitud del Epic

El Epic 46 está completo cuando:
- [x] `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CONTRIBUTING.md` presentes en raíz
- [x] Issue templates activos en `.github/ISSUE_TEMPLATE/`
- [x] GitHub Sponsors habilitado con descripción del proyecto *(habilitación confirmada por el Owner el 2026-06-12)*
- [x] Release pública estable publicada en GitHub Releases con release notes *(v0.2.0 — Public Beta, 2026-06-12; decisión de versión honesta 0.x en lugar de v1.0.0)*
- [x] Repo tiene topics relevantes (cognitive-os, rust, ai-agents, self-hosted, llm) *(verificado vía API)*
- [x] `license` field en todos los `Cargo.toml` dice `MIT` *(workspace.package + 9 crates explícitos)*
- [x] No hay directorios de trabajo temporales en el repo publicado *(`scratch/` gitignorado, 0 archivos trackeados)*
- [x] Social preview image (og:image) configurada *(subida por el Owner el 2026-06-12)*

**Estado real: ✅ 100% — EPIC 46 COMPLETO.** Aegis OS lanzado públicamente como v0.2.0 — Public Beta el 2026-06-12.

---

*Arquitecto IA — 2026-04-28 · Reconciliado 2026-05-31 · Lanzamiento v0.2.0 Public Beta 2026-06-12 · Epic cerrado 2026-06-12*
