# BRIEFING — Shell Engineer: UX Zero-Friction, Fix de F5 y Panel de Vida Personal

**Fecha:** 2026-11-09  
**Para:** Shell Engineer  
**Épica:** EPIC 58 — Personal Assistant Experience, Zero-Friction UX & Community Onboarding  
**Tickets:** `CORE-378`, `CORE-379`, `CORE-377` (UI)  
**Branch sugerido:** `feat/epic58-shell-personal-ux`

---

## Prerequisito

Leer los tres tickets antes de editar código:
1. `governance/Tickets/CORE-378.md` (Fix recarga F5 — 10 min)
2. `governance/Tickets/CORE-379.md` (Humanizar textos + Drawer de Memoria y Gastos — 35 min)
3. `governance/Tickets/CORE-377.md` (Wizard de Onboarding Personal en 1 paso — 30 min)

---

## Orden de Implementación

### 1. `CORE-378` — Persistencia de Sesión al apretar `F5` (10 min)

**Archivos:**
- `shell/ui/src/store/useAegisStore.ts`
- `shell/ui/src/App.tsx`

**Qué hacer:**
1. En `shell/ui/src/store/useAegisStore.ts`:
   - Crear una constante `const TAB_SESSION_KEY = 'aegis-tab-session-key';`.
   - Donde se guarda `sessionKey` al hacer login exitoso (y en `setSession`), agregar:
     ```ts
     try { sessionStorage.setItem(TAB_SESSION_KEY, sessionKey); } catch { /* ignore */ }
     ```
   - En `logout()`, agregar:
     ```ts
     try { sessionStorage.removeItem(TAB_SESSION_KEY); } catch { /* ignore */ }
     ```
   - En `onRehydrateStorage` (línea 2108):
     ```ts
     onRehydrateStorage: () => (state) => {
         if (state) {
             try {
                 const tabKey = sessionStorage.getItem('aegis-tab-session-key');
                 if (state.isAuthenticated && !state.sessionKey && tabKey) {
                     state.sessionKey = tabKey;
                 }
             } catch { /* ignore */ }
             state.setHydrated(true);
         }
     },
     ```
2. Verificar en `shell/ui/src/App.tsx` (líneas 105-110) que al recargar con `F5`, `sessionKey` ya viene restaurado desde `sessionStorage` cuando `_hydrated` pasa a `true`, evitando el `logout()` forzado.

---

### 2. `CORE-379` — Humanizar el Chat y Crear Panel Lateral de Memoria y Gastos (35 min)

**Archivos:**
- `shell/ui/src/components/ChatTerminal.tsx`
- `shell/ui/src/App.tsx`
- `shell/ui/src/components/PersonalAssistantDrawer.tsx` (nuevo)

**Qué hacer:**
1. En `shell/ui/src/components/ChatTerminal.tsx`:
   - En `StatusBadge` (líneas 33-42), reemplazar etiquetas técnicas:
     - `idle` $\rightarrow$ `'En línea'`
     - `thinking` $\rightarrow$ `'Pensando...'`
     - `executing_syscall` $\rightarrow$ `'Ejecutando acción...'`
     - `disconnected` $\rightarrow$ `'Desconectado'`
     - `connecting` $\rightarrow$ `'Conectando...'`
     - `error` $\rightarrow$ `lastError || 'Error de conexión'`
   - En `MessageItem` (líneas 67-69), cambiar `"Operator"` por `"Vos"` y `"ANK Kernel"` por `"Aegis"`.
2. En `shell/ui/src/App.tsx` (línea 201):
   - Cambiar `"ESTABLISHING RING 0 NEURAL LINK..."` por `"CONECTANDO CON TU ASISTENTE PERSONAL..."`.
3. Crear `shell/ui/src/components/PersonalAssistantDrawer.tsx`:
   - Recibe `isOpen: boolean` y `onClose: () => void`.
   - Al abrirse, consulta `GET /api/personal/summary` pasando headers `'x-citadel-tenant': tenantId` y `'x-citadel-key': sessionKey` (con manejo graceful si el endpoint aún devuelve 404).
   - Muestra 3 pestañas limpias con estilo glassmorphism de Aegis:
     1. **💰 Gastos:** Total acumulado, desglose por categoría (`comida`, `transporte`, `hogar`, etc.) y últimos movimientos.
     2. **🧠 Memoria:** Lista de hechos aprendidos sobre el usuario y recordatorios pendientes.
     3. **⚡ Segundo Plano:** Lista de tareas/agentes en ejecución leyendo `agentTree` desde `useAegisStore`.
   - Agregar un botón en el header de `ChatTerminal.tsx` (junto al botón de Dashboard/Settings) para abrir este panel con un clic.

---

### 3. `CORE-377` — Onboarding Personal en 1 Solo Paso (30 min)

**Archivos:**
- `shell/ui/src/components/PersonalQuickSetup.tsx` (nuevo)
- `shell/ui/src/App.tsx` (líneas 165-176)

**Qué hacer:**
1. Crear `shell/ui/src/components/PersonalQuickSetup.tsx`:
   - Un formulario limpio de 1 solo paso para cuando el usuario abre Aegis por primera vez (`systemState === 'STATE_INITIALIZING'`):
     - Campo **Tu Nombre / Usuario** (ej. `tavo`)
     - Campo **Tu Contraseña** (mínimo 8 caracteres)
     - Selector de **Proveedor de IA** (`OpenRouter`, `Google Gemini`, `Ollama Local (Gratis sin API Key)`, `OpenAI`, `Anthropic`) y campo de **API Key** (oculto si elige `Ollama Local`).
   - Envía `POST /api/auth/quick-personal-setup` (incluyendo `setup_token` si estaba presente en la URL).
   - Al recibir éxito, autentica automáticamente en `useAegisStore` y lleva directo al chat sin pasar por pantallas intermedias.
2. En `shell/ui/src/App.tsx` (línea 165):
   - Reemplazar el cartel rojo estático `"Ring 0 Identity Required"` por `<PersonalQuickSetup setupToken={setupToken} />`.

---

## Verificación Obligatoria antes de finalizar

```bash
cd shell/ui
npm run build
```

El build de TypeScript (`tsc && vite build`) debe finalizar con código `0` sin errores ni variables sin usar.
