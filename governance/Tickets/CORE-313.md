# CORE-313 — Reconciliación completa de TICKETS_MASTER + script anti-drift

**Tipo:** chore
**Estado:** ✅ Done  
**Prioridad:** Media
**Épica:** — (Governance / Tooling)
**Asignado a:DevOps + Arquitecto IA
**Componentes:**
- `governance/TICKETS_MASTER.md`
- `governance/Tickets/`
- `tools/sync_tickets_master.py` (nuevo)
**Estado:** ✅ Done

---

## Contexto

`TICKETS_MASTER.md` estaba desincronizado del inventario real de `governance/Tickets/`. En la pasada del 2026-05-31 se cerró lo urgente (registrar CORE-264..267, corregir estados stale de Epic 53 y Epic 46, deprecar la tail huérfana) y el índice quedó **honesto** (declara sus huecos) pero **no completo**.

Causa raíz: el master se mantiene a mano, así que cada vez que un agente cierra un ticket sin actualizar el índice, deriva. Este ticket cierra los huecos restantes **y** elimina la causa raíz con un script de regeneración.

Huecos conocidos (de la nota de sincronización del master):
- Bloque **CORE-259..289**: no itemizado en el master (excepto los ya registrados 262, 263, 264, 265, 266, 267).
- **CORE-307..312** (Epic 54/55): figuran en el master pero **no tienen ticket file** en `governance/Tickets/`.
- `TICKETS_MASTER_tail.md`: deprecado, pendiente de borrar.

## Cambios requeridos

1. **Itemizar/verificar el bloque CORE-259..289** respetando el diseño del master (los epics completos se **resumen**; sólo se itemiza lo activo/reciente). Foco real: **detectar tickets abiertos escondidos** bajo epics marcados `✅ 100%` (como pasó con CORE-266). Cualquier ticket no-Done que aparezca, reflejarlo en el master y en el progreso del epic.
2. **Backfill de los ticket files faltantes** para CORE-307..312, o documentarlos explícitamente como N/A si la decisión es no crearlos.
3. **Script `tools/sync_tickets_master.py`**: parsea los headers de cada `governance/Tickets/CORE-*.md` (campos `Tipo`, título, `Estado`, `Épica`) y reporta discrepancias contra `TICKETS_MASTER.md` (tickets en disco sin entrada en el master, entradas del master sin ticket file, estados divergentes). Modo `--report` (solo diff) y modo `--write` (regenera las tablas itemizadas). Sin dependencias pesadas; stdlib + parsing de markdown.
4. **Borrar `governance/TICKETS_MASTER_tail.md`** (ya deprecado).
5. Cuando el índice quede consistente, **retirar la nota de sincronización** del encabezado de `TICKETS_MASTER.md`.

## Criterios de aceptación

- [ ] `tools/sync_tickets_master.py --report` lista todas las discrepancias entre `governance/Tickets/` y `TICKETS_MASTER.md` sin falsos positivos.
- [ ] No queda ningún ticket en estado `🚧 / 📥 / ❌` bajo un epic marcado `✅ Completa 100%` sin estar reflejado explícitamente en el master.
- [ ] CORE-307..312 tienen ticket file, o están documentados como N/A con justificación.
- [ ] `governance/TICKETS_MASTER_tail.md` eliminado del repo.
- [ ] La nota de sincronización temporal del master fue retirada (índice consistente).

## Dependencias

- Ninguna dura. Puede ejecutarse en paralelo al Epic 56 (no es bloqueante del launch — el master ya es honesto).

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Auditoría completa ejecutada el 2026-06-12: 23 tickets verificados como ya implementados y reconciliados con evidencia archivo:línea; único pendiente genuino del backlog: CORE-276. Script anti-drift en verde (cero discrepancias).
