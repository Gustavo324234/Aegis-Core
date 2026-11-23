import type { AegisState, AgentNodeSummary, ProjectSummary, StoreGet, StoreSet, TrainingProgress } from '../types';
import { buildWsUrl, formatAgentRole, normalizeAgentEvent } from '../shared';

export type AgentsSlice = Pick<AegisState, 'connectAgentStream' | 'disconnectAgentStream' | 'fetchActiveProjects' | 'fetchTrainingStatus' | 'startTraining' | 'cancelTraining' | 'connectTrainingStream' | 'disconnectTrainingStream'>;

export const createAgentsSlice = (set: StoreSet, get: StoreGet): AgentsSlice => ({
    // CORE-204 — agent stream methods
    connectAgentStream: () => {
        const { tenantId, sessionKey, agentSocket } = get();
        if (!tenantId || !sessionKey) return;
        if (agentSocket?.readyState === WebSocket.OPEN) return;

        const wsUrl = buildWsUrl(`/ws/agents/${encodeURIComponent(tenantId)}`);
        const ws = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);

        ws.onopen = () => {
            set({ agentSocket: ws, isAgentStreamConnected: true });
        };

        ws.onmessage = (event) => {
            try {
                const raw = JSON.parse(event.data as string) as Record<string, unknown>;
                const payload = normalizeAgentEvent(raw);
                if (!payload) return;
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
                            console.info(`[AgentStream] Project restored: ${payload.project_id} (${payload.node_count} nodes)`);
                            return {};

                        default:
                            return {};
                    }
                });

                // Generate system message from the event to keep the user informed in real time
                let systemMsg: string | null = null;
                const currentTree = get().agentTree;

                switch (payload.type) {
                    case 'Spawned': {
                        const roleStr = formatAgentRole(payload.role);
                        systemMsg = `🤖 [${roleStr}] Creado para ejecutar tareas usando el modelo ${payload.model}. (ID: ${payload.agent_id.substring(0, 8)})`;
                        break;
                    }
                    case 'StateChanged': {
                        const agent = currentTree.find(n => n.id === payload.agent_id);
                        const roleStr = agent ? formatAgentRole(agent.role) : 'Agente';
                        if (typeof payload.state === 'object' && payload.state && 'Failed' in payload.state) {
                            systemMsg = `⚠️ [${roleStr}] Error en ejecución: ${payload.state.Failed.reason}`;
                        } else if (payload.state === 'Running') {
                            systemMsg = `⚡ [${roleStr}] Ha comenzado a trabajar...`;
                        } else if (payload.state === 'WaitingQuery') {
                            systemMsg = `❓ [${roleStr}] Requiere confirmación o datos adicionales del usuario.`;
                        } else if (payload.state === 'Complete') {
                            systemMsg = `✅ [${roleStr}] Tarea completada con éxito.`;
                        }
                        break;
                    }
                    case 'Activity': {
                        const agent = currentTree.find(n => n.id === payload.agent_id);
                        const roleStr = agent ? formatAgentRole(agent.role) : 'Agente';
                        systemMsg = `⚙️ [${roleStr}] Actividad: ${payload.description}`;
                        break;
                    }
                    case 'Reported': {
                        const agent = currentTree.find(n => n.id === payload.agent_id);
                        const roleStr = agent ? formatAgentRole(agent.role) : 'Agente';
                        systemMsg = `📋 [${roleStr}] Reporte: ${payload.summary}`;
                        break;
                    }
                    default:
                        break;
                }

                if (systemMsg) {
                    get().addSystemMessage(systemMsg);
                }
            } catch (e) {
                console.error('[AgentStream] Failed to parse event:', e);
            }
        };

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

        set({ agentSocket: ws });
    },

    disconnectAgentStream: () => {
        const { agentSocket } = get();
        if (agentSocket) {
            agentSocket.onclose = null;
            agentSocket.onerror = null;
            agentSocket.close();
            set({ agentSocket: null, isAgentStreamConnected: false, agentTree: [] });
        }
    },

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
    // CORE-331 — native training engine methods
    fetchTrainingStatus: async () => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/train/status', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                const data = await res.json() as TrainingProgress;
                set({ trainingProgress: data });
            }
        } catch (err) {
            console.error('Fetch training status error:', err);
        }
    },

    startTraining: async (config) => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const res = await fetch('/api/train/start', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify(config)
            });
            if (res.ok) {
                set({ trainingLogs: ['[Aegis] Solicitud de entrenamiento enviada...'] });
                get().connectTrainingStream();
                return true;
            } else {
                const err = await res.json().catch(() => ({ detail: 'Fallo al iniciar el entrenamiento' }));
                set({ trainingLogs: [`[Error] ${err.detail || 'Fallo al iniciar el entrenamiento'}`] });
                return false;
            }
        } catch (err) {
            console.error('Start training error:', err);
            set({ trainingLogs: ['[Error] Fallo en la conexión de red al arrancar el entrenamiento'] });
            return false;
        }
    },

    cancelTraining: async () => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const res = await fetch('/api/train/cancel', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                set(state => ({
                    trainingLogs: [...state.trainingLogs, '[Aegis] Enviada señal de cancelación...']
                }));
                return true;
            }
            return false;
        } catch (err) {
            console.error('Cancel training error:', err);
            return false;
        }
    },

    connectTrainingStream: () => {
        const { trainingSocket, tenantId, sessionKey } = get();
        if (trainingSocket) {
            trainingSocket.onclose = null;
            trainingSocket.onerror = null;
            trainingSocket.close();
        }
        if (!tenantId || !sessionKey) return;

        const wsUrl = buildWsUrl(`/api/train/progress/${encodeURIComponent(tenantId)}`);
        const socket = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);

        socket.onopen = () => {
            console.log('[TrainWS] Connected to progress stream');
            set({ trainingSocket: socket });
        };

        socket.onmessage = (event) => {
            try {
                const progress = JSON.parse(event.data as string) as TrainingProgress;
                set({ trainingProgress: progress });
                if (progress.log_snippet && progress.log_snippet.trim()) {
                    set(state => {
                        const logs = state.trainingLogs;
                        if (logs[logs.length - 1] === progress.log_snippet) {
                            return {};
                        }
                        return { trainingLogs: [...logs, progress.log_snippet] };
                    });
                }
            } catch (err) {
                console.error('[TrainWS] Error parsing progress payload:', err);
            }
        };

        socket.onclose = () => {
            console.log('[TrainWS] Progress stream closed');
            set({ trainingSocket: null });
        };

        socket.onerror = (err) => {
            console.error('[TrainWS] error:', err);
        };
    },

    disconnectTrainingStream: () => {
        const { trainingSocket } = get();
        if (trainingSocket) {
            trainingSocket.onclose = null;
            trainingSocket.onerror = null;
            trainingSocket.close();
        }
        set({ trainingSocket: null });
    },
});
