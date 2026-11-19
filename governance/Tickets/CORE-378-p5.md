# CORE-378-p5: Motor de Memoria Semántica y RAG Local Embebido (Fase 5)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Media
- **Épica:** Fase 5 — Cutting-Edge (Otras Características Consolidadas)
- **Asignado a:** Kernel Engineer

---

> **Nota de reconciliación (`CORE-396`):** Definido originalmente como `CORE-378` en [`docs/TICKETS_CUTTING_EDGE_ROADMAP.md`](file:///e:/Aegis/Aegis-Core/docs/TICKETS_CUTTING_EDGE_ROADMAP.md) e implementado en el commit `7efd3a0` (*"feat(memory): CORE-378 local semantic embedding driver, auto episodic indexing and search api"*). Se canoniza como `CORE-378-p5` para evitar colisión con [`CORE-378.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-378.md) (EPIC 58 — F5 Session Persistence).

## Description

Driver de embeddings locales en `ank-core::chal::drivers::embeddings`, indexación episódica por tenant en `vcm` / L3 swap cifrado y endpoints de búsqueda semántica en `ank-http::routes::memory_api`.

## Acceptance Criteria

- [x] Driver local de embeddings y almacenamiento vectorial particionado por tenant (`7efd3a0`).
- [x] Endpoint `/api/memory/search` e inyección contextual en preparación del PCB.
