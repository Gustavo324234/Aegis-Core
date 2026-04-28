# BRIEFING — Shell Engineer — CORE-211
**Fecha:** 2026-04-28  
**Rama:** `fix/core-211-agents-graceful-errors`  
**PR title:** `fix(shell): CORE-211 manejo graceful de errores en fetchActiveProjects y connectAgentStream`

---

## Contexto

El Dashboard crashea o muestra estado inconsistente cuando `/api/agents/projects` o `/ws/agents/{tenant_id}` no están disponibles. Este fix agrega manejo defensivo permanente en la UI.

---

## Fix 1 — `shell/ui/src/store/useAegisStore.ts`

Leer el archivo completo antes de modificar.

### 1a. Agregar campo `agentStreamError` a la interface `AegisState`

```typescript
agentStreamError: string | null;
```

Inicializar en el estado base:
```typescript
agentStreamError: null,
```

Agregarlo a la firma de la interface junto a los otros campos de agent stream (buscar `isAgentStreamConnected`).

### 1b. Reemplazar `fetchActiveProjects`

Buscar la implementación actual y reemplazarla:

```typescript
fetchActiveProjects: async () => {
    const { tenantId, sessionKey } = get();
    if (!tenantId || !sessionKey) return;
    try {
        const res = await fetch('/api/agents/projects', {
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        });
        if (res.ok) {
            const data = await res.json() as { projects: ProjectSummary[] };
            set({ activeProjects: data.projects ?? [], agentStreamError: null });
        } else {
            // 404 = endpoint no desplegado aún — silencioso
            // Otros errores — registrar pero no crashear
            const errMsg = res.status !== 404 ? `HTTP ${res.status}` : null;
            set({ activeProjects: [], agentStreamError: errMsg });
        }
    } catch (e) {
        console.error('[Projects] Fetch failed:', e);
        set({ activeProjects: [], agentStreamError: null });
    }
},
```

### 1c. Agregar retry en `connectAgentStream`

Buscar el `ws.onclose` dentro de `connectAgentStream` y reemplazarlo:

```typescript
ws.onclose = () => {
    set({ agentSocket: null, isAgentStreamConnected: false });
    // Retry automático en 5s si el tenant sigue autenticado
    setTimeout(() => {
        const { isAuthenticated, tenantId, sessionKey, isAgentStreamConnected } = get();
        if (isAuthenticated && tenantId && sessionKey && !isAgentStreamConnected) {
            get().connectAgentStream();
        }
    }, 5000);
};

ws.onerror = () => {
    // El onclose se dispara después del onerror — el retry vive ahí
    set({ isAgentStreamConnected: false });
};
```

---

## Fix 2 — `shell/ui/src/components/Dashboard.tsx`

Leer el archivo completo antes de modificar.

### 2a. Agregar `isAgentStreamConnected` al destructuring

Buscar la línea con `useAegisStore()` y agregar `isAgentStreamConnected`:

```typescript
const { setCurrentView, system_metrics, tenantId, sessionKey, isAgentStreamConnected } = useAegisStore();
```

### 2b. Agregar indicador de estado en la sección Projects

Buscar el bloque de "Projects & Agents — CORE-203" y agregar el indicador junto al título:

```tsx
<div className="flex items-center gap-3">
    <Bot className="w-5 h-5 text-aegis-purple" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Projects</h2>
    <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">
        — Cognitive Agent Architecture
    </span>
    {isAgentStreamConnected
        ? <span className="text-[9px] font-mono text-green-400/50 uppercase tracking-widest">● live</span>
        : <span className="text-[9px] font-mono text-white/20 uppercase tracking-widest">○ connecting</span>
    }
</div>
```

---

## Verificación

```
npm run build
```

Sin errores TypeScript.

---

## Branch y commit

```
git checkout -b fix/core-211-agents-graceful-errors

git commit -m "fix(shell): CORE-211 manejo graceful de errores en fetchActiveProjects y connectAgentStream"

git push origin fix/core-211-agents-graceful-errors
```

Tavo hace el PR y merge manualmente.

---

*Arquitecto IA — 2026-04-28*
