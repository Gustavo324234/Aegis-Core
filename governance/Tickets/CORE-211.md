# CORE-211 — fix(shell): manejo graceful de errores en fetchActiveProjects y connectAgentStream

**Epic:** 45 — Cognitive Agent Architecture  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** ninguna — es defensivo ante 404/conexión fallida

---

## Contexto

Cuando el backend no tiene `/api/agents/projects` o `/ws/agents/{tenant_id}` disponibles
(por ejemplo, si CORE-209 no está desplegado aún, o en un reinicio parcial del servidor),
la UI puede entrar en un estado de error que afecta al Dashboard completo.

También aplica como capa de defensa permanente: la UI no debe depender de que estas rutas
estén siempre disponibles para mostrar algo útil.

**Comportamiento actual:**
- `fetchActiveProjects()` — el `catch` existe pero no setea ningún estado de error visible.
  Si la respuesta es 404, el método falla silenciosamente y `activeProjects` queda vacío,
  pero la UI no informa al usuario.
- `connectAgentStream()` — si el WebSocket falla (`ws.onerror`), solo setea
  `isAgentStreamConnected: false`. No hay retry ni indicador visible en el Dashboard.
- El `Dashboard.tsx` asume que si `activeProjects.length === 0` todo está bien, sin distinguir
  entre "no hay proyectos" y "el endpoint falló".

---

## Cambios requeridos

### 1. `shell/ui/src/store/useAegisStore.ts`

**`fetchActiveProjects`** — distinguir entre "vacío" y "error":

Agregar campo de estado en la interface `AegisState`:

```typescript
agentStreamError: string | null;
```

Inicializar en `null`. En `fetchActiveProjects`, si el fetch falla o retorna non-200:

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
            // 404 = backend no implementado aún, no es un error de usuario
            set({ activeProjects: [], agentStreamError: res.status === 404 ? null : `HTTP ${res.status}` });
        }
    } catch (e) {
        console.error('[Projects] Fetch failed:', e);
        set({ activeProjects: [], agentStreamError: null }); // red error → silencioso
    }
},
```

**`connectAgentStream`** — retry con backoff y log claro:

Agregar backoff simple en `ws.onclose` cuando la conexión nunca llegó a abrirse:

```typescript
ws.onclose = () => {
    set({ agentSocket: null, isAgentStreamConnected: false });
    // Retry en 5s si se desconectó inesperadamente y el tenant sigue autenticado
    const { tenantId, sessionKey, isAuthenticated } = get();
    if (isAuthenticated && tenantId && sessionKey) {
        setTimeout(() => {
            if (!get().isAgentStreamConnected) {
                get().connectAgentStream();
            }
        }, 5000);
    }
};

ws.onerror = (e) => {
    console.warn('[AgentStream] WebSocket error — will retry on close.', e);
    set({ isAgentStreamConnected: false });
};
```

### 2. `shell/ui/src/components/Dashboard.tsx`

En la sección "Projects & Agents" del Dashboard, mostrar estado apropiado cuando
no hay proyectos y el stream no está conectado:

```tsx
{/* Projects & Agents — CORE-203 */}
<div className="col-span-12 space-y-6">
    <div className="flex items-center gap-3">
        <Bot className="w-5 h-5 text-aegis-purple" />
        <h2 className="text-xl font-bold uppercase tracking-widest">Projects</h2>
        <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">
            — Cognitive Agent Architecture
        </span>
        {/* Indicador de conexión del stream */}
        {isAgentStreamConnected
            ? <span className="text-[9px] font-mono text-green-400/50 uppercase">● live</span>
            : <span className="text-[9px] font-mono text-white/20 uppercase">○ connecting...</span>
        }
    </div>
    <div className="glass rounded-2xl border border-white/10 p-6">
        <div className="grid grid-cols-2 gap-4">
            <div>
                <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-2">
                    Projects
                </p>
                <ProjectList
                    onSelectProject={setSelectedProjectId}
                    selectedProjectId={selectedProjectId}
                />
            </div>
            <div>
                {selectedProjectId
                    ? <AgentTreeView projectId={selectedProjectId} />
                    : <p className="text-[10px] font-mono text-white/20 pt-8 text-center">
                        Select a project
                      </p>
                }
            </div>
        </div>
    </div>
</div>
```

Agregar `isAgentStreamConnected` al destructuring del `useAegisStore()` en `Dashboard.tsx`.

---

## Criterios de aceptación

- [ ] Si `/api/agents/projects` retorna 404, el Dashboard muestra la sección Projects
  con la lista vacía (sin crash, sin pantalla de error)
- [ ] Si el WebSocket `/ws/agents` falla, la UI muestra "○ connecting..." y reintenta
  automáticamente cada 5 segundos
- [ ] Cuando la conexión es exitosa, el indicador cambia a "● live"
- [ ] Ninguno de estos errores propaga al ErrorBoundary global
- [ ] `npm run build` sin errores TypeScript

---

## Commit message

```
fix(shell): CORE-211 manejo graceful de errores en fetchActiveProjects y connectAgentStream
```

---

*Arquitecto IA — 2026-04-28*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: fetchActiveProjects con try/catch, 404 silencioso y agentStreamError (useAegisStore.ts:1518-1541); connectAgentStream registra errores sin crashear.
