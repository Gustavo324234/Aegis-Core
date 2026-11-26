# CORE-352 — Optimize VCM Git & Governance Context Assembly with Native `git2` and TTL Cache

**Tipo:** perf
**Prioridad:** Alta
**Épica:** EPIC 49 — Cognitive Loop
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

En cada llamada a `VirtualContextManager::assemble_context`, `fetch_git_state` lanzaba 3 subprocesos de sistema operativo (`git rev-parse`, `git status --short`, `git log -1`) sobre el directorio global del proceso, y `fetch_governance_state` leía `governance/TICKETS_MASTER.md` desde disco sin caché. En bucles ReAct multi-turno o árboles de sub-agentes concurrentes, esto introducía latencia de I/O y creación de procesos en cada turno.

## Solución propuesta

1. Reemplazar los 3 subprocesos `git` en `VirtualContextManager::fetch_git_state` por consultas nativas en memoria usando la librería `git2` (`Repository::discover`), priorizando el workspace del tenant si contiene un repositorio Git y acotando la lista de archivos modificados para no saturar la ventana de contexto.
2. Incorporar una caché en memoria con TTL corto (10 segundos) tanto para el estado Git por workspace como para el estado de gobernanza (`TICKETS_MASTER.md`).

## Criterios de aceptación

- [x] `fetch_git_state` utiliza `git2::Repository` sin lanzar subprocesos `git` del sistema operativo.
- [x] Caché con TTL de 10s activa para estado Git y estado de gobernanza.
- [x] Tests unitarios de `vcm` pasando en verde.
