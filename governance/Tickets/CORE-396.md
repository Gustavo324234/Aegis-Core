# CORE-396: Higiene Git y Gobernanza — Limpieza de Worktrees Huérfanos, Reconciliación de IDs `CORE-375..379` y Rama Limpia hacia `main`

- **Tipo:** chore
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 61
- **Asignado a:** DevOps Engineer

---

## Description

Ordenar el estado de Git y la trazabilidad de tickets antes de consolidar hacia `main`:
- Inspeccionar y limpiar los 9 worktrees huérfanos `subagent-*` en `git worktree list`.
- Resolver la colisión de numeración en `CORE-375..379` entre `docs/TICKETS_CUTTING_EDGE_ROADMAP.md` (commits `ffb4e62..7efd3a0`) y los tickets de `EPIC 58` en `governance/Tickets/`.
- Organizar los cambios actualmente sin commitear del working tree en commits Conventional Commits coherentes sobre una rama de integración limpia desde `main`.

## Acceptance Criteria

- [x] `git worktree list` solo contiene el working tree principal limpio.
- [x] Sin colisión de IDs `CORE-*` entre roadmap y archivos en `governance/Tickets/`.
- [x] Working tree limpio y sincronizado con `python tools/sync_tickets_master.py --report` en `exit 0`.
