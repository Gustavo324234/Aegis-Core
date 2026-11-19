# CORE-378: Persistencia de Sesión tras Recarga (`F5`) en `shell/ui/src/App.tsx`

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 58
- **Asignado a:** Shell Engineer

---

## Description
> **Nota de trazabilidad (`CORE-396`):** Este ticket pertenece formalmente a `EPIC 58`. La especificación histórica de Fase 5 (Memoria Semántica y RAG Local, commit `7efd3a0`) queda canonizada en [`CORE-378-p5.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-378-p5.md).

En `shell/ui/src/App.tsx` (líneas 105-110), cada vez que el usuario refresca la página (`F5`), `sessionKey` es `null` porque `useAegisStore.ts` (línea 2115) no lo guarda en `localStorage` por seguridad (`CORE-073`). Como consecuencia, `App.tsx` ejecuta `logout()` en cada `F5`, obligando al usuario a escribir su contraseña cada vez que recarga la pestaña.

## Archivos a Modificar
1. **`shell/ui/src/store/useAegisStore.ts`** (líneas 2105-2126 y acciones `login` / `logout`):
   - Usar `window.sessionStorage` (que está aislado a la pestaña activa y se destruye automáticamente al cerrar la pestaña/navegador, manteniendo la seguridad de `CORE-073` de no dejar la clave en `localStorage` permanente):
     - Al autenticar exitosamente en `login` (cuando se setea `sessionKey`), guardar `sessionStorage.setItem('aegis-tab-session-key', sessionKey)`.
     - En `logout()`, ejecutar `sessionStorage.removeItem('aegis-tab-session-key')`.
     - En `onRehydrateStorage` (línea 2108), antes de llamar a `state.setHydrated(true)`, verificar:
       ```ts
       const savedSessionKey = sessionStorage.getItem('aegis-tab-session-key');
       if (state && state.isAuthenticated && !state.sessionKey && savedSessionKey) {
           state.sessionKey = savedSessionKey;
       }
       ```

2. **`shell/ui/src/App.tsx`** (líneas 105-110):
   - El `useEffect` de seguridad solo forzará `logout()` si tras la hidratación (`_hydrated`) `sessionKey` sigue siendo `null` (por ejemplo, cuando se abre una pestaña nueva después de haber cerrado el navegador).

## Acceptance Criteria
- [x] Presionar `F5` (recargar página) en `http://localhost:8000` mantiene la sesión activa y reconecta el WebSocket del chat automáticamente sin enviar al usuario al `LoginScreen`.
- [x] Hacer clic en el botón de Cerrar Sesión (`logout`) limpia `sessionStorage` y `localStorage` correctamente.
- [x] `npm run build` en `shell/ui` compila sin errores de TypeScript.
