# CORE-126 — Fix: Panel de telemetría superior ilegible

**Status:** DONE — 2026-04-20

## Síntoma

El panel de telemetría superior era demasiado pequeño — texto de 8px,
padding mínimo, métricas ilegibles en pantallas normales.

## Fix en `TelemetryDashboard.tsx`

- `py-2` → `py-3` — más altura
- Texto de labels: `text-[8px]` → `text-[9px]`
- Texto de valores: `text-[10px]` → `text-[11px]`
- Título "Aegis Core": `text-[10px]` → `text-xs`
- Subtítulo estado: `text-[8px]` → `text-[10px]`
- Icono del punto de estado: `w-1 h-1` → `w-1.5 h-1.5`
- Border bottom: `border-white/5` → `border-white/10` (más visible)
- Background: `bg-black/40` → `bg-black/60` (más contraste)
- Indicador "Citadel Protocol": `text-[9px]` → `text-[10px]`
- Modelo activo ahora visible en panel derecho cuando hay routing info
