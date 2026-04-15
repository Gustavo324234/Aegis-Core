import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { ttsPlayer } from '../audio/TTSPlayer';

export type MessageType = 'text' | 'thought' | 'system' | 'error';
export type SystemStatus = 'disconnected' | 'connecting' | 'idle' | 'thinking' | 'executing_syscall' | 'error' | 'listening' | 'transcribing';
export type TaskTypeValue = 'chat' | 'coding' | 'planning' | 'analysis' | 'summarization';

export interface RoutingInfo {
    model_id: string;
    provider: string;
    task_type: string;
    latency_ms: number;
}

export interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    type: MessageType;
    timestamp: number;
}

export interface SystemMetrics {
    cpu_load: number;
    vram_allocated_mb: number;
    vram_total_mb: number;
    total_processes: number;
    active_workers: number;
}

interface AegisState {
    messages: Message[];
    status: SystemStatus;
    system_metrics: SystemMetrics;
    tenants: string[];
    socket: WebSocket | null;
    activePid: string | null;
    isAuthenticated: boolean;
    isAdmin: boolean;
    systemState: 'STATE_INITIALIZING' | 'STATE_OPERATIONAL' | 'UNKNOWN';
    tenantId: string | null;
    sessionKey: string | null;
    isRecording: boolean;
    sirenSocket: WebSocket | null;
    isEngineConfigured: boolean;
    taskType: TaskTypeValue;
    lastRoutingInfo: RoutingInfo | null;
    lastError: string | null;
    _hydrated: boolean;
    needsPasswordReset: boolean;
    adminActiveTab: string;
    isFetchingTenants: boolean;
    lastTenantsUpdate: string | null;
    tenantsError: string | null;

    setHydrated: (val: boolean) => void;
    setNeedsPasswordReset: (val: boolean) => void;
    setAdminActiveTab: (tab: string) => void;
    connect: (tenantId: string, sessionKey: string) => void;
    disconnect: () => void;
    sendMessage: (prompt: string) => void;
    appendToken: (msgId: string, token: string, type: MessageType) => void;
    setStatus: (status: SystemStatus) => void;
    clearHistory: () => void;
    addSystemMessage: (content: string) => void;
    startTelemetryPolling: (tenantId: string) => void;
    fetchSystemState: () => Promise<void>;
    setAuth: (tenantId: string, sessionKey: string) => void;
    authenticate: (tenantId: string, passphrase: string) => Promise<'authenticated' | 'password_must_change' | 'failed'>;
    logout: () => void;
    fetchTenants: () => Promise<void>;
    createTenant: (targetUsername: string) => Promise<{ success: boolean; message?: string; temporary_passphrase?: string }>;
    deleteTenant: (targetId: string) => Promise<boolean>;
    resetPassword: (targetId: string, newPass: string) => Promise<boolean>;
    startSirenStream: () => Promise<void>;
    stopSirenStream: () => void;
    configureEngine: (apiUrl: string, model: string, apiKey: string, provider?: string) => Promise<boolean>;
    setEngineConfigured: (configured: boolean) => void;
    setTaskType: (taskType: TaskTypeValue) => void;
    setLastRoutingInfo: (info: RoutingInfo) => void;
}

interface AegisAudioRefs {
    _aegis_audio_stream?: MediaStream;
    _aegis_audio_ctx?: AudioContext;
    _aegis_audio_node?: ScriptProcessorNode;
}

interface WindowWithWebkit extends Window {
    AudioContext?: typeof AudioContext;
    webkitAudioContext?: typeof AudioContext;
}

const buildWsUrl = (path: string) => {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${proto}//${window.location.host}${path}`;
};

let telemetryInterval: number | null = null;

// Incrementar cuando cambie el schema de partialize para descartar localStorage viejo.
const STORE_VERSION = 3;

export const useAegisStore = create<AegisState>()(
    persist(
        (set, get) => ({
            messages: [],
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
            _hydrated: false,
            needsPasswordReset: false,
            adminActiveTab: 'users',
            isFetchingTenants: false,
            lastTenantsUpdate: null,
            tenantsError: null,

            setHydrated: (val) => set({ _hydrated: val }),
            setNeedsPasswordReset: (val) => set({ needsPasswordReset: val }),
            setAdminActiveTab: (tab) => set({ adminActiveTab: tab }),

            startTelemetryPolling: (tenantId: string) => {
                if (telemetryInterval) clearInterval(telemetryInterval);
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
                telemetryInterval = window.setInterval(poll, 3000);
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

            fetchTenants: async () => {
                const { tenantId, sessionKey } = get();
                if (!tenantId || !sessionKey) return;
                set({ isFetchingTenants: true, tenantsError: null });
                try {
                    const res = await fetch('/api/admin/tenants', {
                        headers: {
                            'Content-Type': 'application/json',
                            'x-citadel-tenant': tenantId,
                            'x-citadel-key': sessionKey,
                        }
                    });
                    if (res.ok) {
                        const data = await res.json();
                        const rawTenants = data.tenants || [];
                        const ids = rawTenants
                            .map((t: string | { tenant_id: string }) => typeof t === 'string' ? t : t.tenant_id)
                            .filter((id: string) => !!id && id !== 'root');
                        set({ tenants: ids, lastTenantsUpdate: new Date().toLocaleTimeString() });
                    } else {
                        const err = await res.json().catch(() => ({ detail: 'Servidor no respondió con JSON válido' }));
                        set({ tenantsError: err.detail || 'Error al listar enclaves' });
                    }
                } catch (e) {
                    console.error('🛡️ Citadel Fetch Error:', e);
                    set({ tenantsError: 'Fallo de conexión (Network Error)' });
                } finally {
                    set({ isFetchingTenants: false });
                }
            },

            createTenant: async (targetUsername: string) => {
                const { tenantId, sessionKey } = get();
                if (!tenantId || !sessionKey) return { success: false, message: 'No admin session' };
                try {
                    const res = await fetch('/api/admin/tenant', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                            'x-citadel-tenant': tenantId,
                            'x-citadel-key': sessionKey,
                        },
                        body: JSON.stringify({ username: targetUsername })
                    });
                    // Safely parse: Axum 422 may return plain text, not JSON
                    const contentType = res.headers.get('content-type') || '';
                    const data = contentType.includes('application/json')
                        ? await res.json()
                        : { detail: await res.text() };
                    if (res.ok) {
                        get().fetchTenants();
                        return { success: true, temporary_passphrase: data.temporary_passphrase };
                    } else {
                        let errMsg = data.detail || data.error || 'Error desconocido al crear Tenant';
                        if (typeof errMsg === 'string' && (errMsg.includes('already exists') || errMsg.includes('Duplicate')))
                            errMsg = `El Tenant "${targetUsername}" ya existe en el Ring 0.`;
                        return { success: false, message: String(errMsg) };
                    }
                } catch (e) {
                    console.error('Create tenant error:', e);
                    return { success: false, message: 'Fallo crítico en la comunicación con el Citadel' };
                }
            },

            deleteTenant: async (targetId: string) => {
                const { tenantId, sessionKey } = get();
                if (!tenantId || !sessionKey) return false;
                try {
                    const res = await fetch(`/api/admin/tenant/${encodeURIComponent(targetId)}`, {
                        method: 'DELETE',
                        headers: {
                            'Content-Type': 'application/json',
                            'x-citadel-tenant': tenantId,
                            'x-citadel-key': sessionKey,
                        }
                    });
                    if (res.ok) { get().fetchTenants(); return true; }
                    return false;
                } catch (e) { console.error('Delete tenant error:', e); return false; }
            },

            resetPassword: async (targetId: string, newPass: string) => {
                const { tenantId, sessionKey } = get();
                if (!tenantId || !sessionKey) return false;
                try {
                    const res = await fetch('/api/admin/reset_password', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                            'x-citadel-tenant': tenantId,
                            'x-citadel-key': sessionKey,
                        },
                        body: JSON.stringify({ tenant_id: targetId, new_passphrase: newPass })
                    });
                    if (res.ok) {
                        if (tenantId === targetId) set({ sessionKey: newPass });
                        return true;
                    }
                    return false;
                } catch (e) { console.error('Reset password error:', e); return false; }
            },

            connect: (tenantId, sessionKey) => {
                const wsUrl = buildWsUrl(`/ws/chat/${encodeURIComponent(tenantId)}`);
                const currentSocket = get().socket;
                if (currentSocket) currentSocket.close();
                set({ status: 'connecting' });
                const socket = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);
                socket.onopen = () => { set({ socket, status: 'idle' }); get().startTelemetryPolling(tenantId); };
                socket.onmessage = (event) => {
                    const msg = JSON.parse(event.data as string);
                    const { event: type, data, pid } = msg as { event: string; data: unknown; pid?: string };
                    switch (type) {
                        case 'syslog':
                            set((state) => ({ messages: [...state.messages, { id: `sys-${Date.now()}`, role: 'system', content: data as string, type: 'system', timestamp: Date.now() }] }));
                            break;
                        case 'status':
                            set({ status: 'thinking' });
                            if (pid) set({ activePid: pid });
                            break;
                        case 'kernel_event': {
                            const payload = data as Record<string, unknown>;
                            if (payload.routing_info) get().setLastRoutingInfo(payload.routing_info as RoutingInfo);
                            if (payload.thought) { get().appendToken(payload.pid as string, payload.thought as string, 'thought'); set({ status: 'thinking' }); }
                            else if (payload.output) { get().appendToken(payload.pid as string, payload.output as string, 'text'); set({ status: 'thinking' }); }
                            else if (payload.error) { get().appendToken(payload.pid as string, payload.error as string, 'error'); set({ status: 'error' }); }
                            else if (payload.status_update) {
                                const su = payload.status_update as { state: string };
                                if (su.state === 'STATE_COMPLETED') set({ status: 'idle', activePid: null });
                            }
                            break;
                        }
                        case 'error': {
                            const errData = data as string;
                            set({ status: 'error', lastError: errData || 'Unknown Kernel Panic' });
                            if (errData === 'PASSWORD_MUST_CHANGE') {
                                set({ needsPasswordReset: true });
                            } else if (errData?.includes('AUTH_FAILURE: Access Denied')) {
                                const sock = get().socket;
                                if (sock) sock.close();
                                set({ socket: null, status: 'error' });
                            }
                            break;
                        }
                    }
                };
                socket.onclose = () => {
                    set(state => ({ socket: null, status: state.status === 'error' ? 'error' : 'disconnected' }));
                };
                socket.onerror = () => set({ status: 'error' });
            },

            disconnect: () => get().socket?.close(),

            sendMessage: (prompt) => {
                const { socket, messages } = get();
                if (!socket || socket.readyState !== WebSocket.OPEN) return;
                ttsPlayer.initialize();
                set({ messages: [...messages, { id: `user-${Date.now()}`, role: 'user', content: prompt, type: 'text', timestamp: Date.now() }] });
                socket.send(JSON.stringify({ prompt, task_type: get().taskType }));
            },

            appendToken: (pid, token, type) => {
                set((state) => {
                    const lastMessage = state.messages[state.messages.length - 1];
                    if (lastMessage?.role === 'assistant' && lastMessage.type === type && lastMessage.id === pid) {
                        const updatedMessages = [...state.messages];
                        updatedMessages[updatedMessages.length - 1] = { ...lastMessage, content: lastMessage.content + token };
                        return { messages: updatedMessages };
                    }
                    return { messages: [...state.messages, { id: pid, role: 'assistant', content: token, type, timestamp: Date.now() }] };
                });
            },

            setStatus: (status) => set({ status }),
            clearHistory: () => set({ messages: [] }),
            addSystemMessage: (content: string) => {
                set({ messages: [...get().messages, { id: `sys-${Date.now()}`, role: 'system', content, type: 'system', timestamp: Date.now() }] });
            },

            setAuth: (tenantId, sessionKey) => set({ tenantId, sessionKey, isAuthenticated: true }),

            authenticate: async (tenantId, passphrase) => {
                try {
                    const response = await fetch('/api/auth/login', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ tenant_id: tenantId, session_key: passphrase })
                    });
                    if (response.ok) {
                        const data = await response.json() as { status: string; role?: string };
                        if (data.status === 'password_must_change') {
                            set({ tenantId, sessionKey: passphrase, isAuthenticated: true, isAdmin: false, needsPasswordReset: true });
                            return 'password_must_change';
                        }
                        set({ tenantId, sessionKey: passphrase, isAuthenticated: true, isAdmin: data.role === 'admin', needsPasswordReset: false });
                        return 'authenticated';
                    }
                    return 'failed';
                } catch (error) { console.error('🛡️ Citadel Auth Error:', error); return 'failed'; }
            },

            logout: () => {
                const { socket, sirenSocket } = get();
                if (socket) socket.close();
                if (sirenSocket) sirenSocket.close();
                set({
                    isAuthenticated: false,
                    isAdmin: false,
                    tenantId: null,
                    sessionKey: null,
                    socket: null,
                    sirenSocket: null,
                    messages: [],
                    needsPasswordReset: false,
                    adminActiveTab: 'users', // resetear tab al hacer logout
                });
                if (telemetryInterval) { clearInterval(telemetryInterval); telemetryInterval = null; }
            },

            startSirenStream: async () => {
                const { tenantId, sessionKey, isRecording } = get();
                if (isRecording || !tenantId || !sessionKey) return;
                try {
                    await ttsPlayer.initialize();
                    const stream = await navigator.mediaDevices.getUserMedia({ audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true } });
                    const win = window as WindowWithWebkit;
                    const AudioCtx = win.AudioContext || win.webkitAudioContext;
                    if (!AudioCtx) throw new Error('Web Audio API not supported');
                    const ctx = new AudioCtx({ sampleRate: 16000 });
                    const source = ctx.createMediaStreamSource(stream);
                    const scriptNode = ctx.createScriptProcessor(4096, 1, 1);
                    const analyser = ctx.createAnalyser();
                    analyser.fftSize = 256;
                    analyser.smoothingTimeConstant = 0.2;
                    source.connect(analyser);
                    let silenceStart = Date.now();
                    const SILENCE_THRESHOLD = 5;
                    const MAX_SILENCE_MS = 1500;
                    let vadRequestedStop = false;
                    const checkSilence = () => {
                        if (!get().isRecording || vadRequestedStop) return;
                        const dataArray = new Uint8Array(analyser.frequencyBinCount);
                        analyser.getByteFrequencyData(dataArray);
                        let sum = 0;
                        for (let i = 0; i < dataArray.length; i++) sum += dataArray[i];
                        if (sum / dataArray.length < SILENCE_THRESHOLD) {
                            if (Date.now() - silenceStart > MAX_SILENCE_MS) { vadRequestedStop = true; get().stopSirenStream(); return; }
                        } else silenceStart = Date.now();
                        requestAnimationFrame(checkSilence);
                    };
                    const wsUrl = buildWsUrl(`/ws/siren/${encodeURIComponent(tenantId)}`);
                    const sirenWs = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);
                    sirenWs.binaryType = 'arraybuffer';
                    sirenWs.onopen = () => { set({ isRecording: true, sirenSocket: sirenWs }); requestAnimationFrame(checkSilence); };
                    sirenWs.onmessage = (event) => {
                        const msg = JSON.parse(event.data as string) as Record<string, unknown>;
                        if (msg.event === 'siren_event') {
                            const sirenEvent = msg.data as Record<string, unknown>;
                            if (sirenEvent.tts_audio_chunk) {
                                try { ttsPlayer.playChunk(sirenEvent.tts_audio_chunk as string, (sirenEvent.sample_rate as number) || 22050); }
                                catch (e) { console.error('TTS Playback error:', e); }
                            }
                            if (sirenEvent.event_type === 'VAD_START') set({ status: 'listening' });
                            else if (sirenEvent.event_type === 'STT_START') set({ status: 'transcribing' });
                            else if (sirenEvent.event_type === 'STT_DONE') {
                                try {
                                    const payload = JSON.parse(sirenEvent.message as string) as { transcript: string; pid: string };
                                    set((state) => ({ messages: [...state.messages, { id: `voice-${Date.now()}`, role: 'user', content: payload.transcript, type: 'text', timestamp: Date.now() }], activePid: payload.pid, status: 'thinking' }));
                                    const chatSocket = get().socket;
                                    if (chatSocket?.readyState === WebSocket.OPEN) chatSocket.send(JSON.stringify({ action: 'watch', pid: payload.pid }));
                                } catch (e) { console.error('Failed to parse STT_DONE payload', e); set({ status: 'idle' }); }
                            } else if (sirenEvent.event_type === 'STT_ERROR') set({ status: 'error' });
                        } else if (msg.error) { console.error('❌ Siren Kernel Error:', msg.error); get().stopSirenStream(); }
                    };
                    sirenWs.onclose = () => get().stopSirenStream();
                    scriptNode.onaudioprocess = (audioEvent: AudioProcessingEvent) => {
                        if (sirenWs.readyState !== WebSocket.OPEN) return;
                        const inputData = audioEvent.inputBuffer.getChannelData(0);
                        const pcmBuffer = new Int16Array(inputData.length);
                        for (let i = 0; i < inputData.length; i++) {
                            const s = Math.max(-1, Math.min(1, inputData[i]));
                            pcmBuffer[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
                        }
                        sirenWs.send(pcmBuffer.buffer);
                    };
                    source.connect(scriptNode);
                    scriptNode.connect(ctx.destination);
                    const audioRefs = window as Window & AegisAudioRefs;
                    audioRefs._aegis_audio_stream = stream;
                    audioRefs._aegis_audio_ctx = ctx;
                    audioRefs._aegis_audio_node = scriptNode;
                } catch (error) { console.error('🎤 Siren Capture Error:', error); set({ isRecording: false }); throw error; }
            },

            stopSirenStream: () => {
                const { sirenSocket } = get();
                if (sirenSocket?.readyState === WebSocket.OPEN) {
                    sirenSocket.send(JSON.stringify({ sequence_number: 9999, data: '', format: 'VAD_END_SIGNAL', sample_rate: 16000 }));
                    setTimeout(() => { if (sirenSocket.readyState === WebSocket.OPEN) sirenSocket.close(); }, 100);
                } else if (sirenSocket) sirenSocket.close();
                set({ sirenSocket: null });
                const audioRefs = window as Window & AegisAudioRefs;
                if (audioRefs._aegis_audio_stream) audioRefs._aegis_audio_stream.getTracks().forEach(track => track.stop());
                if (audioRefs._aegis_audio_node) audioRefs._aegis_audio_node.disconnect();
                if (audioRefs._aegis_audio_ctx) audioRefs._aegis_audio_ctx.close();
                set({ isRecording: false });
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
                    if (response.ok) { set({ isEngineConfigured: true }); return true; }
                    return false;
                } catch (error) { console.error('Engine Setup Error:', error); return false; }
            }
        }),
        {
            name: 'aegis-storage',
            version: STORE_VERSION,
            onRehydrateStorage: () => (state) => {
                if (state) state.setHydrated(true);
            },
            partialize: (state) => ({
                isAuthenticated: state.isAuthenticated,
                isAdmin: state.isAdmin,
                tenantId: state.tenantId,
                // sessionKey NO se persiste — seguridad (CORE-073)
                // adminActiveTab NO se persiste — evita crash al recargar en tabs que requieren sessionKey
                isEngineConfigured: state.isEngineConfigured,
                taskType: state.taskType,
                messages: state.messages,
                lastError: state.lastError,
                needsPasswordReset: state.needsPasswordReset,
                lastTenantsUpdate: state.lastTenantsUpdate,
            }),
        }
    )
);
