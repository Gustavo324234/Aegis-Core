# CORE-032 — shell/ui: componentes core

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/src/components/`
**Asignado a:** Shell Engineer
**Prioridad:** 🔴 Alta
**Estado:** DONE
**Depende de:** CORE-031

---

## Contexto

Los componentes principales de la interfaz: el terminal de chat, la telemetría,
el dashboard de admin y el orb. Se portan desde el legacy sin cambios funcionales.

**Referencia:** `Aegis-Shell/ui/src/components/`

---

## Componentes a portar

### ChatTerminal — `src/components/ChatTerminal.tsx`
Terminal principal de chat con:
- Lista virtualizada de mensajes (`react-window` + `react-virtualized-auto-sizer`)
- Renderizado Markdown (`react-markdown` + `remark-gfm`)
- Input con envío por Enter, soporte multiline con Shift+Enter
- Selector de `TaskType` (chat/coding/planning/analysis/summarization)
- Botón de voz (VoiceButton) que llama a `startSirenStream()`
- Cursor de streaming animado (▍) mientras el kernel procesa
- Indicador de `RoutingInfo` en el mensaje del assistant

### TelemetrySidebar — `src/components/TelemetrySidebar.tsx`
Sidebar derecho con métricas en tiempo real:
- CPU load, VRAM allocated/total
- Active processes, active workers
- Estado del kernel (OPERATIONAL / INITIALIZING)
- `RoutingInfo` del último request (model_id, provider, latency)

### TheOrb — `src/components/TheOrb.tsx`
Indicador visual animado del estado del sistema (idle/thinking/error/listening).

### AdminDashboard — `src/components/AdminDashboard.tsx`
Dashboard de administración con tabs:
- **Users** — listar, crear, eliminar tenants; reset password
- **Providers** — ProvidersTab (ver CORE-034)
- **Router** — RouterConfig (ver CORE-034)
- **Voice** — SirenConfigTab

### TelemetryDashboard — `src/components/TelemetryDashboard.tsx`
Vista expandida de telemetría (usada dentro del AdminDashboard).

### App.tsx — `src/App.tsx`
Lógica de routing principal basada en estado Zustand:
- `systemState == 'STATE_INITIALIZING'` → `AdminSetupScreen`
- `?setup_token=...` en URL → `BootstrapSetup`
- `!isAuthenticated` → `LoginScreen`
- `needsPasswordReset` → `ForcePasswordChangeScreen`
- `!isEngineConfigured && !isAdmin` → `EngineSetupWizard`
- Default → `ChatTerminal` + `TelemetrySidebar`

### `src/main.tsx` y `src/styles/index.css`
Portar desde legacy sin cambios.

---

## Criterios de aceptación

- [x] `ChatTerminal` renderiza mensajes y envía prompts correctamente
- [x] `ChatTerminal` virtualiza la lista (no renderiza todos los mensajes a la vez)
- [x] `TelemetrySidebar` muestra métricas actualizadas del store
- [x] `AdminDashboard` muestra las tabs correctas según `isAdmin`
- [x] `App.tsx` enruta correctamente según el estado del store
- [x] `npm run build` → 0 errores TypeScript

## Referencia

`Aegis-Shell/ui/src/components/` — todos los componentes a portar
`Aegis-Shell/ui/src/App.tsx` — routing principal
