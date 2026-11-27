import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { STORE_VERSION } from './shared';
import { TAB_SESSION_KEY, type AegisState } from './types';
import { createAuthSlice } from './slices/authSlice';
import { createAdminSlice } from './slices/adminSlice';
import { createChatSlice } from './slices/chatSlice';
import { createSirenSlice } from './slices/sirenSlice';
import { createAgentsSlice } from './slices/agentsSlice';

export * from './types';

export const useAegisStore = create<AegisState>()(
    persist(
        (set, get) => ({
            messages: [],
            historyTotalCount: 0,
            nextHistoryBeforeIndex: null,
            hasMoreHistory: false,
            isLoadingHistory: false,
            cleanOnOpen: true,
            historyPrependedSeq: 0,
            status: 'disconnected',
            system_metrics: { cpu_load: 0, vram_allocated_mb: 0, vram_total_mb: 0, total_processes: 0, active_workers: 0 },
            tenants: [],
            socket: null,
            activePid: null,
            isAuthenticated: false,
            isAdmin: false,
            systemState: 'UNKNOWN',
            tenantId: null,
            sessionKey: null,
            isRecording: false,
            sirenSocket: null,
            isEngineConfigured: false,
            taskType: 'chat',
            lastRoutingInfo: null,
            lastError: null,
            lastSubmittedPrompt: null,
            pendingRetryPrompt: null,
            _hydrated: false,
            needsPasswordReset: false,
            adminActiveTab: 'users',
            isFetchingTenants: false,
            lastTenantsUpdate: null,
            tenantsError: null,
            sttAvailable: true,
            sttProvider: 'browser',
            sttApiKey: '',
            groqKeyDetected: false,
            sttReady: false,
            wakeFollowupUntil: 0,
            lastWakeSpeaker: null,
            currentView: 'chat',
            activeAgentId: null,
            inputMode: 'text',
            voiceEnabled: false,
            agentName: null,
            isWakewordListening: false,
            liveToolProgress: null,

            // CORE-204 — initial agent stream values
            agentTree: [],
            activeProjects: [],
            agentSocket: null,
            isAgentStreamConnected: false,
            agentStreamError: null,

            // CORE-331 — initial training values
            trainingProgress: null,
            trainingLogs: [],
            trainingSocket: null,
            ...createAuthSlice(set, get),
            ...createAdminSlice(set, get),
            ...createChatSlice(set, get),
            ...createSirenSlice(set, get),
            ...createAgentsSlice(set, get),
        }),
        {
            name: 'aegis-storage',
            version: STORE_VERSION,
            onRehydrateStorage: () => (state) => {
                if (state) {
                    try {
                        const tabKey = sessionStorage.getItem(TAB_SESSION_KEY);
                        if (state.isAuthenticated && !state.sessionKey && tabKey) {
                            state.sessionKey = tabKey;
                        }
                    } catch { /* ignore */ }
                    state.setHydrated(true);
                }
            },
            partialize: (state) => ({
                isAuthenticated: state.isAuthenticated,
                isAdmin: state.isAdmin,
                tenantId: state.tenantId,
                // sessionKey y messages NO se persisten en localStorage — seguridad (CORE-073 / CORE-369)
                isEngineConfigured: state.isEngineConfigured,
                taskType: state.taskType,
                needsPasswordReset: state.needsPasswordReset,
                lastTenantsUpdate: state.lastTenantsUpdate,
                // No persistir vistas de agente — el store es en memoria y se perdería al recargar
                currentView: (state.currentView === 'chat' || state.currentView === 'dashboard') ? state.currentView : 'chat',
                inputMode: state.inputMode,
                sttProvider: state.sttProvider,  // CORE-231: persistir para que sobreviva recarga
                voiceEnabled: state.voiceEnabled,
                cleanOnOpen: state.cleanOnOpen,
            }),
        }
    )
);
