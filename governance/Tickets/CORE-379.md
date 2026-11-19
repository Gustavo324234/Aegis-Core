# CORE-379: Humanización de Interfaz Web y Panel de Vida Personal (Memoria, Gastos y Tareas en Segundo Plano) en `shell/ui`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 58
- **Asignado a:** Shell Engineer

---

## Description
> **Nota de trazabilidad (`CORE-396`):** Este ticket pertenece formalmente a `EPIC 58`. La especificación histórica de Fase 5 (UI/UX y Generative Artifacts) queda canonizada en [`CORE-379-p5.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-379-p5.md).

La interfaz principal (`ChatTerminal.tsx` y `App.tsx`) utiliza terminología interna de sistemas operativos (*"Operator"*, *"ANK Kernel"*, *"Executing Syscall"*, *"Establishing Ring 0 Neural Link"*) que resulta críptica para usuarios nuevos, y carece de un panel accesible desde el chat para ver qué hechos recuerda Aegis, la tabla de gastos personales y las tareas ejecutándose en segundo plano.

## Archivos a Modificar
1. **`shell/ui/src/components/ChatTerminal.tsx`** (líneas 31-95) y **`shell/ui/src/App.tsx`** (líneas 199-202):
   - En `StatusBadge` (`ChatTerminal.tsx` líneas 33-42), cambiar etiquetas técnicas por estados amigables:
     - `idle` -> `"En línea"` / `"Online"`
     - `thinking` -> `"Pensando..."` / `"Thinking..."`
     - `executing_syscall` -> `"Ejecutando acción..."` / `"Running tool..."`
     - `connecting` -> `"Conectando..."` / `"Connecting..."`
     - `error` -> `lastError || "Error de conexión"`
   - En `MessageItem` (`ChatTerminal.tsx` líneas 67-69), reemplazar `"Operator"` por `tenantId || "Vos"` y `"ANK Kernel"` por `"Aegis"`.
   - En `App.tsx` (línea 201), cambiar `"ESTABLISHING RING 0 NEURAL LINK..."` por `"CONECTANDO CON TU ASISTENTE PERSONAL..."`.

2. **Nuevo Componente `shell/ui/src/components/PersonalAssistantDrawer.tsx`:**
   - Panel lateral deslizable (abierto desde un botón **"🧠 Mi Memoria y Gastos"** en el header de `ChatTerminal.tsx`) que consume `GET /api/personal/summary` (`CORE-375`) y el estado `agentTree` de `useAegisStore`:
     - **Tab 1 — 💰 Gastos y Finanzas:** Tarjeta con el Total Acumulado, barras por categoría (`comida`, `transporte`, `hogar`, etc.) y lista de últimos movimientos registrados por chat.
     - **Tab 2 — 🧠 Memoria a Largo Plazo:** Lista de hechos que Aegis aprendió sobre el usuario (con badge si fue aprendido por `chat` o consolidado por `chronos`) + botón **"Consolidar Ahora (Chronos)"** (`POST /api/personal/chronos/consolidate`).
     - **Tab 3 — ⚡ Segundo Plano:** Lista de subagentes/workers activos o completados (`agentTree`) y recordatorios pendientes.

## Acceptance Criteria
- [x] El chat principal muestra nombres y estados amigables (*"Aegis"*, *"Pensando..."*, *"Ejecutando acción..."*) sin jerga de kernel (*"Syscall"*, *"Ring 0"*, *"Operator"*).
- [x] Desde el header del chat se puede abrir y cerrar el panel lateral **Mi Memoria y Gastos** viendo en tiempo real los gastos por categoría, los hechos recordados y las tareas en segundo plano.
- [x] `npm run build` en `shell/ui` pasa sin errores ni warnings de TypeScript.
