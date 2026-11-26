import { TAB_SESSION_KEY, type AegisState, type StoreGet, type StoreSet } from '../types';
import {
    clearPendingRetryTimer,
    storeRuntime,
} from '../shared';

export type AuthSlice = Pick<AegisState, 'setHydrated' | 'setCurrentView' | 'setActiveAgentId' | 'setNeedsPasswordReset' | 'setAdminActiveTab' | 'startTelemetryPolling' | 'fetchSystemState' | 'changeOwnPassword' | 'setAuth' | 'authenticate' | 'logout' | 'setEngineConfigured' | 'setTaskType' | 'setLastRoutingInfo' | 'configureEngine'>;

export const createAuthSlice = (set: StoreSet, get: StoreGet): AuthSlice => ({
    setHydrated: (val) => set({ _hydrated: val }),
    setCurrentView: (view) => set({ currentView: view }),
    setActiveAgentId: (agentId) => set({ activeAgentId: agentId }),
    setNeedsPasswordReset: (val) => set({ needsPasswordReset: val }),
    setAdminActiveTab: (tab) => set({ adminActiveTab: tab }),
    startTelemetryPolling: (tenantId: string) => {
        if (storeRuntime.telemetryInterval) clearInterval(storeRuntime.telemetryInterval);
        const poll = async () => {
            try {
                const sessionKey = get().sessionKey;
                if (!sessionKey || !tenantId) return;
                const response = await fetch('/api/status', {
                    headers: {
                        'x-citadel-tenant': tenantId,
                        'x-citadel-key': sessionKey,
                    }
                });
                if (response.ok) {
                    const data = await response.json();
                    set({ system_metrics: data, status: get().status === 'error' ? 'idle' : get().status, lastError: null });
                } else if (response.status === 401) {
                    set({ lastError: 'Citadel Session Expired/Unauthorized' });
                    get().logout();
                } else {
                    const data = await response.json();
                    set({ lastError: data.detail || 'Telemetry Stream Interrupted' });
                }
            } catch (error) { console.error('🛰️ Telemetry Polling Error:', error); }
        };
        poll();
        storeRuntime.telemetryInterval = window.setInterval(poll, 3000);
    },

    fetchSystemState: async () => {
        try {
            const response = await fetch('/api/system/state');
            if (response.ok) {
                const data = await response.json();
                set({ systemState: data.state });
            } else if (response.status === 401) {
                set({ systemState: 'STATE_OPERATIONAL' });
            }
        } catch (error) {
            console.error('Failed to fetch system state:', error);
            set({ systemState: 'UNKNOWN' });
        }
    },
    changeOwnPassword: async (newPass: string) => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const res = await fetch('/api/auth/change_password', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    tenant_id: tenantId,
                    current_password: sessionKey,
                    new_password: newPass,
                })
            });
            if (res.ok) {
                storeRuntime.chatAuthRejected = false;
                storeRuntime.chatReconnectAttempts = 0;
                try { sessionStorage.setItem(TAB_SESSION_KEY, newPass); } catch { /* ignore */ }
                set({ sessionKey: newPass, needsPasswordReset: false });
                return true;
            }
            return false;
        } catch (e) { console.error('Change own password error:', e); return false; }
    },
    setAuth: (tenantId, sessionKey) => {
        try { sessionStorage.setItem(TAB_SESSION_KEY, sessionKey); } catch { /* ignore */ }
        set({ tenantId, sessionKey, isAuthenticated: true });
    },

    authenticate: async (tenantId, passphrase) => {
        try {
            const response = await fetch('/api/auth/login', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ tenant_id: tenantId, session_key: passphrase })
            });
            if (response.ok) {
                storeRuntime.chatAuthRejected = false;
                storeRuntime.chatReconnectAttempts = 0;
                const data = await response.json() as {
                    status: string;
                    role?: string;
                    session_token?: string;
                };
                if (data.status === 'password_must_change') {
                    try { sessionStorage.setItem(TAB_SESSION_KEY, passphrase); } catch { /* ignore */ }
                    set({ tenantId, sessionKey: passphrase, isAuthenticated: true, isAdmin: false, needsPasswordReset: true, messages: [] });
                    return 'password_must_change';
                }
                const activeCredential = data.session_token || passphrase;
                try { sessionStorage.setItem(TAB_SESSION_KEY, activeCredential); } catch { /* ignore */ }
                set({ tenantId, sessionKey: activeCredential, isAuthenticated: true, isAdmin: data.role === 'admin', needsPasswordReset: false, messages: [] });
                return 'authenticated';
            }
            return 'failed';
        } catch (error) { console.error('🛡️ Citadel Auth Error:', error); return 'failed'; }
    },

    // CORE-369: Purge sensitive in-memory state on logout; server-side SQLCipher
    // history is reloaded automatically via loadChatHistory upon re-authentication.
    logout: () => {
        const { socket, sirenSocket, sessionKey: activeCredential } = get();
        if (activeCredential && activeCredential.startsWith('cst_')) {
            fetch('/api/auth/logout', {
                method: 'POST',
                headers: { 'x-citadel-token': activeCredential }
            }).catch(() => {});
        }
        try { sessionStorage.removeItem(TAB_SESSION_KEY); } catch { /* ignore */ }
        storeRuntime.chatManualDisconnect = true;
        storeRuntime.chatAuthRejected = false;
        storeRuntime.chatReconnectAttempts = 0;
        clearPendingRetryTimer();
        if (storeRuntime.chatReconnectTimer) {
            clearTimeout(storeRuntime.chatReconnectTimer);
            storeRuntime.chatReconnectTimer = null;
        }
        if (socket) {
            socket.onclose = null;
            socket.onerror = null;
            socket.close();
        }
        if (sirenSocket) sirenSocket.close();
        get().stopWakewordListener();
        get().stopSirenStream();
        get().disconnectAgentStream();
        get().disconnectTrainingStream();
        set({
            isAuthenticated: false,
            isAdmin: false,
            tenantId: null,
            sessionKey: null,
            socket: null,
            sirenSocket: null,
            messages: [],
            historyTotalCount: 0,
            nextHistoryBeforeIndex: null,
            hasMoreHistory: false,
            isLoadingHistory: false,
            activePid: null,
            lastRoutingInfo: null,
            lastError: null,
            lastSubmittedPrompt: null,
            pendingRetryPrompt: null,
            sttApiKey: '',
            agentTree: [],
            activeProjects: [],
            activeAgentId: null,
            currentView: 'chat',
            needsPasswordReset: false,
            adminActiveTab: 'users',
            status: 'disconnected',
            trainingProgress: null,
            trainingLogs: [],
        });
        if (storeRuntime.telemetryInterval) { clearInterval(storeRuntime.telemetryInterval); storeRuntime.telemetryInterval = null; }
    },
    setEngineConfigured: (configured: boolean) => set({ isEngineConfigured: configured }),
    setTaskType: (taskType) => set({ taskType }),
    setLastRoutingInfo: (info) => set({ lastRoutingInfo: info }),
    configureEngine: async (apiUrl, model, apiKey, provider = 'custom') => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const response = await fetch('/api/engine/configure', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ api_url: apiUrl, model_name: model, api_key: apiKey, provider })
            });
            if (response.ok) {
                set({ isEngineConfigured: true });
                if (get().pendingRetryPrompt && get().status !== 'thinking') {
                    get().retryPendingMessage(null);
                }
                return true;
            }
            return false;
        } catch (error) { console.error('Engine Setup Error:', error); return false; }
    }
});
