# CORE-153 — Feat: Dashboard Dinámico & Kanban UI

**Epic:** 42 — Vision Realignment & Autonomy
**Repo:** Aegis-Core (Shell)
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** UI Engineer
**Estado:** ✅ Done

---

## Problema

La interfaz actual es solo un chat. No hay una forma visual de ver el progreso de las tareas, los gastos registrados o el estado general de los sistemas de un vistazo.

## Solución Propuesta

Implementar una vista de **Dashboard** en el Aegis Shell (React).

### Requisitos Técnicos:
1.  **Kanban Board:** Un componente visual que consuma datos de `governance/TICKETS_MASTER.md` o un archivo de tareas sincronizado.
2.  **Widgets de Datos:** Mini-gráficos o paneles para mostrar los gastos (Ledger) y próximos recordatorios (Chronos).
3.  **Comunicación WebSocket:** El kernel debe poder enviar eventos tipo `ui_update` para refrescar el dashboard en tiempo real cuando se añade un gasto o se mueve un ticket.

---

## Criterios de Aceptación

- [x] Existe una pestaña o ruta `/dashboard` en la Shell.
- [x] Se visualizan los tickets en columnas (Todo, In Progress, Done).
- [x] Se visualiza un widget con el gasto total del mes.

---

*Arquitecto IA — 2026-04-23*
