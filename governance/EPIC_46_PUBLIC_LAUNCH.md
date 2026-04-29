# EPIC 46 — Public Launch

**Objetivo:** Dejar Aegis-Core en condición de lanzamiento público open source.
El repo debe ser encontrable, comprensible, contribuible y con infraestructura de comunidad mínima lista.

**Precondición:** Epic Opción A (documentación) completada — ✅ Done (2026-04-28)

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

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| CORE-214 | CODE_OF_CONDUCT.md | 📥 Todo | Alta |
| CORE-215 | SECURITY.md — política de reporte de vulnerabilidades | 📥 Todo | Alta |
| CORE-216 | CHANGELOG.md — historial de versiones público | 📥 Todo | Media |

---

### Área B — GitHub Issue Templates

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| CORE-217 | Issue template: Bug Report | 📥 Todo | Alta |
| CORE-218 | Issue template: Feature Request | 📥 Todo | Media |

---

### Área C — GitHub Sponsors

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| CORE-219 | Habilitar GitHub Sponsors + redactar sponsor page | 📥 Todo | Alta |

---

### Área D — Release & Visibility

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| CORE-220 | Crear release v1.0.0 en GitHub con release notes | 📥 Todo | Crítica |
| CORE-221 | Agregar topics al repo de GitHub | 📥 Todo | Media |
| CORE-222 | Social preview image (og:image del repo) | 📥 Todo | Media |

---

### Área E — Repo Hygiene

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| CORE-223 | .github/CODEOWNERS — definir owner para cada área | 📥 Todo | Media |
| CORE-224 | Archivar rama scratch/ y limpiar target_verify_fix/ | 📥 Todo | Baja |
| CORE-225 | Sincronizar licencia: README menciona MIT, verificar Cargo.toml | 📥 Todo | Alta |

---

## Criterio de completitud del Epic

El Epic 46 está completo cuando:
- [ ] `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CONTRIBUTING.md` presentes en raíz
- [ ] Issue templates activos en `.github/ISSUE_TEMPLATE/`
- [ ] GitHub Sponsors habilitado con descripción del proyecto
- [ ] Release v1.0.0 publicada en GitHub Releases con release notes
- [ ] Repo tiene topics relevantes (cognitive-os, rust, ai-agents, self-hosted, llm)
- [ ] `license` field en todos los `Cargo.toml` dice `MIT`
- [ ] No hay directorios de trabajo temporales en raíz del repo

---

*Arquitecto IA — 2026-04-28*
