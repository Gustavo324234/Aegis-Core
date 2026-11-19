# CORE-204 — feat: useAegisStore — agentTree state + connectAgentStream

**Epic:** 45 — Cognitive Agent Architecture  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-200 (WebSocket ws/agents/{tenant_id} — implementado en kernel)

---

## Contexto

El kernel ya expone el WebSocket `ws/agents/{tenant_id}` que emite `AgentEvent`
en tiempo real. El store necesita conectarse a ese stream y mantener el árbol de
agentes en estado para que los componentes CORE-202 y CORE-203 puedan renderizarlo.

---

## Cambios Requeridos

### `shell/ui/src/store/useAegisStore.ts`

#### 1. Nuevos tipos

Agregar al inicio del archivo junto a los tipos existentes:

```typescript
export type AgentRole =
    | { type: 'ChatAgent' }
    | { type: 'ProjectSupervisor'; project_id: string }
    | { type: 'Supervisor'; name: string; depth: number }
    | { type: 'Specialist'; scope: string };

export type AgentNodeState =
    | 'Idle'
    | 'Running'
    | 'WaitingReport'
    | 'WaitingQuery'
    | 'Complete'
    | { Failed: { reason: string } };

export interface AgentNodeSummary {
    id: string;
    role: AgentRole;
    state: AgentNodeState;
    parent_id: string | null;
    children: string[];
    model: string | null;       // modelo asignado por el CMR
    activity: string | null;    // última descripción de actividad
    last_report: string | null;
}

export interface ProjectSummary {
    project_id: string;
    name: string;
    description: string | null;
    status: 'active' | 'archived';
    root_agent_id: string | null;
}

export type AgentEventPayload =
    | { type: 'Spawned'; agent_id: string; role: AgentRole; parent_id: string | null; model: string }
    | { type: 'StateChanged'; agent_id: string; state: AgentNodeState }
    | { type: 'Activity'; agent_id: string; description: string }
    | { type: 'Reported'; agent_id: string; summary: string }
    | { type: 'Pruned'; agent_id: string }
    | { type: 'Restored'; project_id: string; node_count: number }
    | { type: 'TreeSnapshot'; tree: AgentNodeSummary[] };
```

#### 2. Nuevos campos en `AegisState`

```typescript
// En la interfaz AegisState
agentTree: AgentNodeSummary[];
activeProjects: ProjectSummary[];
agentSocket: WebSocket | null;
isAgentStreamConnected: boolean;

connectAgentStream: () => void;
disconnectAgentStream: () => void;
fetchActiveProjects: () => Promise<void>;
```

#### 3. Valores iniciales

```typescript
agentTree: [],
activeProjects: [],
agentSocket: null,
isAgentStreamConnected: false,
```

#### 4. Implementación de `connectAgentStream`

```typescript
connectAgentStream: () => {
    const { tenantId, sessionKey, agentSocket } = get();
    if (!tenantId || !sessionKey) return;
    if (agentSocket?.readyState === WebSocket.OPEN) return; // ya conectado

    const wsUrl = buildWsUrl(`/ws/agents/${encodeURIComponent(tenantId)}`);
    const ws = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);

    ws.onopen = () => {
        set({ agentSocket: ws, isAgentStreamConnected: true });
    };

    ws.onmessage = (event) => {
        try {
            const payload = JSON.parse(event.data as string) as AgentEventPayload;
            set((state) => {
                switch (payload.type) {
                    case 'TreeSnapshot':
                        return { agentTree: payload.tree };

                    case 'Spawned': {
                        const newNode: AgentNodeSummary = {
                            id: payload.agent_id,
                            role: payload.role,
                            state: 'Idle',
                            parent_id: payload.parent_id,
                            children: [],
                            model: payload.model,
                            activity: null,
                            last_report: null,
                        };
                        // Agregar nodo + actualizar children del padre
                        const updatedTree = [
                            ...state.agentTree.map(n =>
                                n.id === payload.parent_id
                                    ? { ...n, children: [...n.children, payload.agent_id] }
                                    : n
                            ),
                            newNode,
                        ];
                        return { agentTree: updatedTree };
                    }

                    case 'StateChanged':
                        return {
                            agentTree: state.agentTree.map(n =>
                                n.id === payload.agent_id
                                    ? { ...n, state: payload.state }
                                    : n
                            ),
                        };

                    case 'Activity':
                        return {
                            agentTree: state.agentTree.map(n =>
                                n.id === payload.agent_id
                                    ? { ...n, activity: payload.description }
                                    : n
                            ),
                        };

                    case 'Reported':
                        return {
                            agentTree: state.agentTree.map(n =>
                                n.id === payload.agent_id
                                    ? { ...n, last_report: payload.summary, state: 'Complete' }
                                    : n
                            ),
                        };

                    case 'Pruned':
                        return {
                            agentTree: state.agentTree
                                .filter(n => n.id !== payload.agent_id)
                                .map(n => ({
                                    ...n,
                                    children: n.children.filter(c => c !== payload.agent_id),
                                })),
                        };

                    case 'Restored':
                        // El kernel mandará un TreeSnapshot inmediatamente después
                        // Solo loguear por ahora
                        console.info(`[AgentStream] Project restored: ${payload.project_id} (${payload.node_count} nodes)`);
                        return {};

                    default:
                        return {};
                }
            });
        } catch (e) {
            console.error('[AgentStream] Failed to parse event:', e);
        }
    };

    ws.onclose = () => {
        set({ agentSocket: null, isAgentStreamConnected: false });
    };

    ws.onerror = () => {
        set({ isAgentStreamConnected: false });
    };

    set({ agentSocket: ws });
},
```

#### 5. Implementación de `disconnectAgentStream`

```typescript
disconnectAgentStream: () => {
    const { agentSocket } = get();
    if (agentSocket) {
        agentSocket.close();
        set({ agentSocket: null, isAgentStreamConnected: false, agentTree: [] });
    }
},
```

#### 6. Implementación de `fetchActiveProjects`

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
            set({ activeProjects: data.projects ?? [] });
        }
    } catch (e) {
        console.error('[Projects] Fetch failed:', e);
    }
},
```

#### 7. Integrar `connectAgentStream` en `connect()`

Al final del `socket.onopen` existente, agregar:

```typescript
socket.onopen = () => {
    set({ socket, status: 'idle' });
    get().startTelemetryPolling(tenantId);
    get().connectAgentStream(); // ← AGREGAR
    get().fetchActiveProjects(); // ← AGREGAR
};
```

#### 8. Integrar `disconnectAgentStream` en `logout()`

Al final del `logout()` existente, agregar:

```typescript
get().disconnectAgentStream(); // ← AGREGAR antes del set()
```

#### 9. NO persistir en `partialize`

`agentTree` y `activeProjects` son estado de sesión en tiempo real.
No agregar a `partialize` — se reconstituyen al conectar.

---

## Criterios de Aceptación

- [ ] Al hacer login, `connectAgentStream` se llama automáticamente
- [ ] `agentTree` se actualiza en tiempo real al recibir `AgentEvent` del kernel
- [ ] `Spawned` agrega el nodo y actualiza `children` del padre
- [ ] `Pruned` elimina el nodo y lo remueve de `children` del padre
- [ ] `TreeSnapshot` reemplaza el árbol completo
- [ ] Al hacer logout, `agentSocket` se cierra y `agentTree` se vacía
- [ ] `fetchActiveProjects` popula `activeProjects` al conectar
- [ ] `npm run build` sin errores TypeScript

---

## Commit message

```
feat(shell): CORE-204 useAegisStore — agentTree state + connectAgentStream
```

---

*Arquitecto IA — 2026-04-26*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: useAegisStore tiene agentTree state + connectAgentStream + agentStreamError.
