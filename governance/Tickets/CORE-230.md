# CORE-230 — fix(shell): Dashboard crashea al montar y dispara AegisErrorBoundary ("Session Error")

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Shell Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

Al hacer click en "Dashboard" desde el ChatTerminal, el usuario ve la pantalla:

> **Session Error**  
> State corruption detected. Session will be reset. Reset Session

Este mensaje es el `AegisErrorBoundary` de `App.tsx`. El boundary captura cualquier
excepción de React en el árbol de componentes y siempre muestra el mismo mensaje,
independientemente de la causa real. No es corrupción de sesión — es un crash de un
componente del Dashboard.

## Causa probable

`Dashboard.tsx` monta simultáneamente en el primer render:
- `AgentTreeWidget` (hace fetch a `/api/agents/...`)
- `WorkspaceSettings` (hace fetch a `/api/workspace/config`)
- `TerminalPanel`, `CodeViewer`, `GitTimeline`, `PRManagerPanel`

Si cualquiera de estos componentes accede a `tenantId`/`sessionKey` que aún son `null`
en el momento del mount (el store se está rehidratando desde localStorage), o si un
fetch falla y el componente no tiene error boundary propio, la excepción sube hasta
`AegisErrorBoundary` y borra toda la sesión.

## Cambios requeridos

### 1. Agregar error boundaries individuales a los widgets del Dashboard

**Archivo:** `shell/ui/src/components/Dashboard.tsx`

Envolver cada widget pesado en un `<ErrorBoundary fallback={<WidgetErrorState />}>`:
- `AgentTreeWidget`
- `WorkspaceSettings`
- `TerminalPanel`, `CodeViewer`, `GitTimeline`, `PRManagerPanel`

El `WidgetErrorState` debe ser un placeholder que muestre el nombre del widget y un
mensaje de error sin colapsar toda la pantalla. Un widget que crashea no debe tumbar
el Dashboard entero.

### 2. Guardar en `useAegisStore` si hay `sessionKey` antes de renderizar el Dashboard

**Archivo:** `shell/ui/src/App.tsx`

En el `useEffect` que llama a `connect()`, agregar una guarda explícita: no renderizar
`<Dashboard />` hasta que `sessionKey` esté disponible en memoria. Si `currentView === 'dashboard'`
pero `sessionKey` es null, forzar `setCurrentView('chat')` y logear el motivo.

### 3. Mejorar el mensaje del AegisErrorBoundary

**Archivo:** `shell/ui/src/App.tsx`

Cambiar el texto del boundary de "State corruption detected" a algo más descriptivo y
menos alarmante. Ej: "An error occurred loading this view. Your session is intact." y
ofrecer tanto "Reload view" (sin borrar localStorage) como "Reset session" (limpia todo).

El mensaje actual es técnicamente incorrecto (no es corrupción de estado) y confunde al
usuario haciéndole creer que perdió su sesión cuando en realidad no es así.

## Criterios de aceptación

- [x] Al entrar al Dashboard, si un widget falla su fetch, se muestra un error inline en ese widget sin afectar el resto
- [x] Si `sessionKey` es null al intentar navegar al Dashboard, la UI redirige al chat sin pantalla de error
- [x] El mensaje del `AegisErrorBoundary` no dice "State corruption" — dice algo más preciso
- [x] Ningún crash de widget borra la sesión del usuario

## Dependencias

Ninguna.
