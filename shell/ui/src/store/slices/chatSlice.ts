import { ttsPlayer } from '../../audio/TTSPlayer';
import { speakNaturalText, stopSpeaking, loadVoicePrefs } from '../../audio/naturalVoice';
import { useMusicStore } from '../musicStore';
import { useTerminalStore } from '../terminalStore';
import { usePrStore } from '../prStore';
import { useAgentInboxStore } from '../agentInboxStore';
import type { AegisState, RoutingInfo, StoreGet, StoreSet, SupervisorEventPayload } from '../types';
import {
    buildWsUrl,
    clearPendingRetryTimer,
    mapHistoricalMessages,
    resolveAutoRetryDelaySecs,
    storeRuntime,
} from '../shared';

export type ChatSlice = Pick<AegisState, 'loadChatHistory' | 'loadOlderHistory' | 'clearChatView' | 'setCleanOnOpen' | 'connect' | 'disconnect' | 'sendMessage' | 'retryPendingMessage' | 'dismissPendingRetry' | 'appendToken' | 'setStatus' | 'clearHistory' | 'clearAgentTraces' | 'addSystemMessage'>;

export const createChatSlice = (set: StoreSet, get: StoreGet): ChatSlice => ({
    loadChatHistory: async (tenantId: string, sessionKey: string) => {
        try {
            const res = await fetch('/api/chat/history?respect_clean=true&limit=25', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (!res.ok) return;
            const data = await res.json();
            const total = typeof data.total === 'number' ? data.total : (data.messages?.length ?? 0);
            const startIndex = typeof data.start_index === 'number' ? data.start_index : 0;
            const hasMore = typeof data.has_more === 'boolean' ? data.has_more : startIndex > 0;
            const cleanOnOpen = typeof data.clean_on_open === 'boolean' ? data.clean_on_open : get().cleanOnOpen;
            const historicalMessages = mapHistoricalMessages(data.messages || [], startIndex);
            // Setear mensajes históricos ANTES de que el WebSocket empiece a recibir
            set({
                messages: historicalMessages,
                historyTotalCount: total,
                nextHistoryBeforeIndex: startIndex,
                hasMoreHistory: hasMore,
                cleanOnOpen,
            });
        } catch {
            // Silencioso — si falla, el chat arranca vacío (comportamiento actual)
        }
    },

    loadOlderHistory: async (limit = 25) => {
        const { tenantId, sessionKey, isLoadingHistory, hasMoreHistory, nextHistoryBeforeIndex, historyTotalCount } = get();
        const before = nextHistoryBeforeIndex ?? historyTotalCount;
        if (!tenantId || !sessionKey || isLoadingHistory || !hasMoreHistory || before <= 0) {
            return 0;
        }
        set({ isLoadingHistory: true });
        try {
            const res = await fetch(`/api/chat/history?before=${before}&limit=${limit}`, {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (!res.ok) return 0;
            const data = await res.json();
            const total = typeof data.total === 'number' ? data.total : historyTotalCount;
            const startIndex = typeof data.start_index === 'number' ? data.start_index : 0;
            const hasMore = typeof data.has_more === 'boolean' ? data.has_more : startIndex > 0;
            const older = mapHistoricalMessages(data.messages || [], startIndex);
            let prependedCount = 0;
            set((state) => {
                const existingIds = new Set(state.messages.map((m) => m.id));
                const uniqueOlder = older.filter((m) => !existingIds.has(m.id));
                prependedCount = uniqueOlder.length;
                return {
                    messages: [...uniqueOlder, ...state.messages],
                    historyTotalCount: total,
                    nextHistoryBeforeIndex: startIndex,
                    hasMoreHistory: hasMore,
                    historyPrependedSeq: uniqueOlder.length > 0 ? state.historyPrependedSeq + 1 : state.historyPrependedSeq,
                };
            });
            return prependedCount;
        } catch {
            return 0;
        } finally {
            set({ isLoadingHistory: false });
        }
    },

    clearChatView: async () => {
        clearPendingRetryTimer();
        const { tenantId, sessionKey, historyTotalCount, nextHistoryBeforeIndex } = get();
        const fallbackTotal = nextHistoryBeforeIndex ?? historyTotalCount;
        set({ messages: [], lastSubmittedPrompt: null, pendingRetryPrompt: null });
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/chat/visual-clear', {
                method: 'POST',
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                const data = await res.json();
                const total = typeof data.total === 'number' ? data.total : fallbackTotal;
                set({
                    historyTotalCount: total,
                    nextHistoryBeforeIndex: total,
                    hasMoreHistory: typeof data.has_more === 'boolean' ? data.has_more : total > 0,
                    cleanOnOpen: typeof data.clean_on_open === 'boolean' ? data.clean_on_open : get().cleanOnOpen,
                });
            }
        } catch (e) {
            console.error('Failed to persist visual clear:', e);
        }
    },

    setCleanOnOpen: async (val: boolean) => {
        set({ cleanOnOpen: val });
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return;
        try {
            await fetch('/api/chat/ui-state', {
                method: 'PATCH',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ clean_on_open: val }),
            });
        } catch (e) {
            console.error('Failed to save clean_on_open preference:', e);
        }
    },
    connect: async (tenantId, sessionKey) => {
        if (storeRuntime.chatReconnectTimer) {
            clearTimeout(storeRuntime.chatReconnectTimer);
            storeRuntime.chatReconnectTimer = null;
        }
        storeRuntime.chatManualDisconnect = false;

        const wsUrl = buildWsUrl(`/ws/chat/${encodeURIComponent(tenantId)}`);
        const currentSocket = get().socket;
        if (currentSocket) {
            currentSocket.onclose = null;
            currentSocket.onerror = null;
            currentSocket.close();
        }
        set({ socket: null, status: 'connecting' });

        // Cargar historial persistido ANTES de abrir el WebSocket (CORE-247)
        if (storeRuntime.chatReconnectAttempts === 0 || get().messages.length === 0) {
            await get().loadChatHistory(tenantId, sessionKey);
        }

        const socket = new WebSocket(wsUrl, [`session-key.${sessionKey}`]);
        socket.onopen = () => {
            storeRuntime.chatReconnectAttempts = 0;
            storeRuntime.chatAuthRejected = false;
            set({ socket, status: 'idle', lastError: null });
            get().startTelemetryPolling(tenantId);
            get().connectAgentStream();
            void get().fetchActiveProjects();
            void get().fetchAgentName();
            if (get().pendingRetryPrompt) {
                setTimeout(() => {
                    if (get().pendingRetryPrompt && get().status !== 'thinking') {
                        get().retryPendingMessage();
                    }
                }, 400);
            }
        };
        const handleFailedTurnRetry = (errText: string, explicitSecs?: number) => {
            const lastPrompt = get().lastSubmittedPrompt;
            if (!lastPrompt) {
                get().addSystemMessage(errText);
                return;
            }
            const retryAfterSecs = resolveAutoRetryDelaySecs(errText, explicitSecs);
            clearPendingRetryTimer();
            set({
                pendingRetryPrompt: {
                    ...lastPrompt,
                    retryAfterSecs,
                    errorMessage: errText,
                },
            });
            if (retryAfterSecs && retryAfterSecs > 0 && retryAfterSecs <= 90 && lastPrompt.autoRetryCount < 2) {
                storeRuntime.pendingRetryTimer = setTimeout(() => {
                    storeRuntime.pendingRetryTimer = null;
                    if (get().pendingRetryPrompt && get().status !== 'thinking') {
                        get().retryPendingMessage();
                    }
                }, retryAfterSecs * 1000);
                get().addSystemMessage(
                    `${errText}\n⏳ Se reintentará automáticamente en ${retryAfterSecs}s (o cambiá de modelo / configurá una API key para responder al instante).`
                );
            } else {
                get().addSystemMessage(errText);
            }
        };
        socket.onmessage = (event) => {
            const msg = JSON.parse(event.data as string);
            const { event: type, data, pid } = msg as { event: string; data: unknown; pid?: string };
            switch (type) {
                case 'syslog':
                    // CORE-124: no agregar syslog de reconexión al historial persistido
                    break;
                case 'status':
                    set({ status: 'thinking', liveToolProgress: null });
                    if (pid) set({ activePid: pid });
                    break;
                case 'kernel_event': {
                    const payload = data as Record<string, unknown>;
                    if (payload.routing_info) get().setLastRoutingInfo(payload.routing_info as RoutingInfo);
                    if (payload.thought) { get().appendToken(payload.pid as string, payload.thought as string, 'thought'); set({ status: 'thinking' }); }
                    else if (payload.output) {
                        if (get().pendingRetryPrompt || get().lastSubmittedPrompt) {
                            clearPendingRetryTimer();
                            set({ pendingRetryPrompt: null, lastSubmittedPrompt: null });
                        }
                        get().appendToken(payload.pid as string, payload.output as string, 'text');
                        set({ status: 'thinking' });
                    }
                    else if (payload.error) {
                        // CORE-FIX (D): surface the kernel's actual error text
                        // (e.g. "modelo saturado, reintentá en 15s") instead of a
                        // generic fallback, so the user sees the real reason.
                        const errText = typeof payload.error === 'string' && payload.error.trim()
                            ? (payload.error as string)
                            : 'No pude procesar tu mensaje. Intentá de nuevo en unos segundos.';
                        set({ status: 'error', liveToolProgress: null });
                        handleFailedTurnRetry(errText);
                    }
                    else if (payload.status_update) {
                        const su = payload.status_update as { state: string };
                        if (su.state === 'STATE_COMPLETED') {
                            set({ status: 'idle', activePid: null, liveToolProgress: null });
                            if (get().messages.length <= 10) {
                                void get().fetchAgentName();
                            }

                            // CORE-FIX: feedback loop fix. Previously, when voiceEnabled was on,
                            // we (a) spoke the response via browser TTS (window.speechSynthesis),
                            // and (b) reactivated the mic after a fixed-ish delay computed from
                            // ttsPlayer.getNextStartTime() — but that is the AEGIS cloud TTS,
                            // NOT the browser TTS. So the mic came back ON while the browser was
                            // still mid-sentence, the STT transcribed the assistant's own words,
                            // and the kernel got self-replies like "ya soy Hola Te llamaré aegis".
                            //
                            // Now: gate the mic reactivation on the actual `onend` event of the
                            // browser utterance. If voice is OFF (or there's nothing to speak),
                            // the old delay-based path runs as before. We also `cancel()` the
                            // synthesis if the next user message arrives so we don't pile up.
                            const reactivateMic = () => {
                                const mode = get().inputMode;
                                const autoContinue = loadVoicePrefs().autoContinueMic;
                                if ((mode === 'conversation' || autoContinue) && !get().isRecording) {
                                    get().restartMicForConversation().catch(console.error);
                                } else if (mode === 'wakeword' && !get().isRecording) {
                                    get().armWakeFollowupWindow();
                                    get().startWakewordListener();
                                }
                            };

                            const messages = get().messages;
                            const lastAssistant = [...messages].reverse().find(m => m.role === 'assistant');
                            const speakingViaBrowserTts =
                                get().voiceEnabled
                                && typeof window !== 'undefined'
                                && !!window.speechSynthesis
                                && !!lastAssistant?.content;

                            if (speakingViaBrowserTts && lastAssistant?.content) {
                                speakNaturalText(lastAssistant.content, () => {
                                    const mode = get().inputMode;
                                    const autoContinue = loadVoicePrefs().autoContinueMic;
                                    if (mode === 'conversation' || mode === 'wakeword' || autoContinue) {
                                        setTimeout(reactivateMic, 250);
                                    }
                                });
                            } else if (get().inputMode === 'conversation' || get().inputMode === 'wakeword' || loadVoicePrefs().autoContinueMic) {
                                // CORE-184 legacy path: no browser TTS in play, fall back to the
                                // ttsPlayer-based delay (server-side TTS).
                                const scheduleReactivation = () => {
                                    const ctx = ttsPlayer.getAudioContext();
                                    const remaining = ctx
                                        ? Math.max(0, (ttsPlayer.getNextStartTime() - ctx.currentTime) * 1000)
                                        : 0;
                                    const delay = Math.max(300, remaining + 200);
                                    setTimeout(reactivateMic, delay);
                                };
                                scheduleReactivation();
                            }
                        }
                    }
                    break;
                }
                case 'error': {
                    // CORE-FIX: backend now emits one of two shapes for `error`:
                    //   legacy: data = "AUTH_FAILURE: Access Denied" (string)
                    //   new:    data = { code, message, detail, retry_after_secs } (object)
                    // Normalise here so we accept both without losing the specific
                    // user-facing message for categorised errors (auth_failed,
                    // scheduler_unavailable, models_unavailable, etc.).
                    const structured = (typeof data === 'object' && data !== null)
                        ? data as { code?: string; message?: string; detail?: string; retry_after_secs?: number }
                        : null;
                    const legacy = (typeof data === 'string') ? data : null;
                    const code = structured?.code ?? '';
                    const message = structured?.message ?? legacy ?? 'Unknown Kernel Panic';

                    set({ status: 'error', lastError: message });

                    const isAuthFailure =
                        code === 'auth_failed' ||
                        (legacy?.includes('AUTH_FAILURE: Access Denied') ?? false);
                    const isPasswordReset =
                        code === 'password_must_change' ||
                        legacy === 'PASSWORD_MUST_CHANGE';

                    if (isPasswordReset) {
                        storeRuntime.chatAuthRejected = true;
                        clearPendingRetryTimer();
                        set({ needsPasswordReset: true, pendingRetryPrompt: null, lastSubmittedPrompt: null });
                    } else if (isAuthFailure) {
                        storeRuntime.chatAuthRejected = true;
                        clearPendingRetryTimer();
                        const sock = get().socket;
                        if (sock) {
                            sock.onclose = null;
                            sock.onerror = null;
                            sock.close();
                        }
                        set({ socket: null, status: 'error', pendingRetryPrompt: null, lastSubmittedPrompt: null });
                    } else {
                        handleFailedTurnRetry(
                            message !== 'Unknown Kernel Panic'
                                ? message
                                : 'No pude procesar tu mensaje. Intentá de nuevo en unos segundos.',
                            structured?.retry_after_secs
                        );
                    }
                    break;
                }

                case 'provider_keys_updated': {
                    if (get().pendingRetryPrompt && get().status !== 'thinking') {
                        get().retryPendingMessage(null);
                    }
                    break;
                }

                case 'modules_updated': {
                    if (typeof window !== 'undefined') {
                        window.dispatchEvent(new CustomEvent('aegis-modules-updated', { detail: data }));
                    }
                    break;
                }

                case 'video_studio_progress': {
                    if (typeof window !== 'undefined') {
                        window.dispatchEvent(new CustomEvent('aegis-video-progress', { detail: data }));
                    }
                    break;
                }

                case 'tool_progress': {
                    const tp = data as {
                        stage?: string;
                        message?: string;
                        progress?: number;
                        done?: boolean;
                    };
                    set({
                        liveToolProgress: tp?.done ? null : tp,
                    });
                    if (typeof window !== 'undefined') {
                        window.dispatchEvent(new CustomEvent('aegis-video-progress', { detail: data }));
                    }
                    break;
                }

                case 'model_selected': {
                    // CORE-FIX: backend announces which model is about to answer
                    // so the UI can show a badge. We piggy-back on lastRoutingInfo
                    // (the existing routing display) and synthesise the missing
                    // task_type / latency_ms fields — they get overwritten by
                    // the real routing_info event when the model finishes.
                    const meta = data as { model_id?: string; provider?: string };
                    if (meta?.model_id) {
                        get().setLastRoutingInfo({
                            model_id: meta.model_id,
                            provider: meta.provider ?? 'unknown',
                            task_type: get().lastRoutingInfo?.task_type ?? 'chat',
                            latency_ms: 0,
                        });
                    }
                    break;
                }

                case 'warning': {
                    // CORE-FIX: non-fatal warnings from the kernel (e.g. VCM context
                    // assembly failure, fallback model swap). Show them as system
                    // messages so the user knows the response is degraded.
                    const w = data as { category?: string; message?: string };
                    if (w?.message) {
                        get().addSystemMessage(`⚠ ${w.message}`);
                    }
                    break;
                }
                // FIX CORE-BUG: music_play y music_control leían data.payload.xxx
                // El kernel envía: { event: "music_play", data: { provider, video_id, ... } }
                // No existe data.payload — data ES el objeto directamente.
                case 'music_play': {
                    const musicData = data as {
                        provider?: string;
                        video_id?: string;
                        track_uri?: string;
                        track_id?: string;
                        title?: string;
                        channel?: string;
                        thumbnail?: string;
                    };
                    const { playTrack } = useMusicStore.getState();
                    if (musicData.video_id) {
                        playTrack({
                            videoId: musicData.video_id,
                            title: musicData.title || 'Unknown',
                            channel: musicData.channel || 'Unknown',
                            thumbnail: musicData.thumbnail || '',
                            provider: 'youtube',
                        });
                    } else if (musicData.track_id || musicData.track_uri) {
                        playTrack({
                            videoId: musicData.track_id || musicData.track_uri || '',
                            title: musicData.title || 'Unknown',
                            channel: musicData.channel || 'Unknown',
                            thumbnail: musicData.thumbnail || '',
                            provider: 'spotify',
                        });
                    }
                    break;
                }
                // CORE-175: Developer Workspace events
                case 'terminal_output': {
                    const line = data as { kind: string; content: string; timestamp: string };
                    useTerminalStore.getState().appendLine({
                        kind: (line.kind || 'Stdout') as import('../terminalStore').LineKind,
                        content: line.content || '',
                        timestamp: line.timestamp || new Date().toISOString(),
                    });
                    break;
                }
                case 'pr_update': {
                    const d = data as { pr_number: number; status: string };
                    if (d.pr_number) {
                        usePrStore.getState().updatePr(d.pr_number, {
                            status: d.status?.toLowerCase().replace(/([A-Z])/g, '_$1').replace(/^_/, '') as import('../prStore').PrStatus,
                        });
                    }
                    break;
                }
                case 'pr_merged': {
                    const d = data as { pr_number: number };
                    if (d.pr_number) usePrStore.getState().updatePr(d.pr_number, { status: 'merged' });
                    break;
                }
                case 'git_push': {
                    const d = data as { branch: string };
                    useTerminalStore.getState().addSystemLine(`Git push completed: ${d.branch ?? ''}`);
                    break;
                }
                case 'ci_fix_attempt': {
                    const d = data as { pr_number: number; attempt: number; max_attempts: number; error_summary: string };
                    useTerminalStore.getState().addSystemLine(
                        `[PR #${d.pr_number}] Auto-fix attempt ${d.attempt}/${d.max_attempts}: ${d.error_summary}`,
                    );
                    break;
                }
                case 'chat_notification': {
                    const d = data as { message: string };
                    get().addSystemMessage(d.message || '');
                    break;
                }
                case 'reminder_due': {
                    const d = data as {
                        id?: number;
                        description?: string;
                        remind_at?: string;
                        next_remind_at?: string | null;
                        recurrence?: string;
                    };
                    if (d.description) {
                        const timeTag = d.remind_at ? ` (${d.remind_at})` : '';
                        const nextTag = d.next_remind_at ? ` · próximo: ${d.next_remind_at}` : '';
                        get().addSystemMessage(`⏰ Recordatorio: ${d.description}${timeTag}${nextTag}`);
                        if (get().voiceEnabled) {
                            speakNaturalText(`Recordatorio: ${d.description}`);
                        }
                        if (typeof Notification !== 'undefined' && Notification.permission === 'granted') {
                            try {
                                new Notification('Aegis — Recordatorio', { body: d.description });
                            } catch {
                                // Ignore Notification restrictions in unsupported contexts
                            }
                        }
                    }
                    break;
                }
                // CORE-274: eventos de supervisores desde el kernel
                case 'agent_event': {
                    const payload = data as SupervisorEventPayload;
                    const inbox = useAgentInboxStore.getState();
                    switch (payload.type) {
                        case 'supervisor_question':
                            inbox.addMessage({
                                agentId: payload.agent_id,
                                projectName: payload.project_name,
                                question: payload.question,
                                context: payload.context,
                                timestamp: payload.timestamp,
                                status: 'pending',
                            });
                            if (get().voiceEnabled) {
                                speakNaturalText(
                                    `Pregunta de ${payload.project_name}: ${payload.question}`,
                                );
                            }
                            break;
                        case 'supervisor_completed': {
                            inbox.markAnswered(payload.agent_id);
                            const isFailure = payload.summary.trim().startsWith('❌');
                            get().addSystemMessage(
                                isFailure
                                    ? `${payload.project_name}: ${payload.summary}`
                                    : `✅ ${payload.project_name}: ${payload.summary}`,
                            );
                            if (get().voiceEnabled) {
                                speakNaturalText(
                                    `${payload.project_name}: ${payload.summary}`,
                                );
                            }
                            break;
                        }
                        case 'spawned':
                            get().addSystemMessage(
                                `🤖 Supervisor '${payload.project_id}' iniciado en segundo plano (ID: ${payload.agent_id.slice(0, 8)}).`,
                            );
                            break;
                        case 'state_changed':
                            if (typeof payload.state === 'string' && payload.state.startsWith('Failed:')) {
                                get().addSystemMessage(
                                    `⚠️ Agente ${payload.agent_id.slice(0, 8)} falló: ${payload.state.slice('Failed:'.length).trim()}`,
                                );
                            }
                            break;
                        case 'supervisor_timed_out':
                            inbox.markTimedOut(payload.agent_id);
                            get().addSystemMessage(
                                `⚠️ El supervisor de ${payload.project_name} se pausó: no recibió respuesta a tiempo. Volvé a escribirle para retomar.`,
                            );
                            break;
                        case 'supervisor_resumed':
                            break;
                    }
                    break;
                }
                case 'music_control': {
                    const ctrl = data as { action?: string; value?: string };
                    if (!ctrl.action) break;
                    const { setPlaying, setVolume, closePlayer } = useMusicStore.getState();
                    switch (ctrl.action) {
                        case 'pause': setPlaying(false); break;
                        case 'resume': setPlaying(true); break;
                        case 'stop': closePlayer(); break;
                        case 'volume': {
                            const vol = parseInt(ctrl.value ?? '70', 10);
                            if (!isNaN(vol)) setVolume(Math.min(100, Math.max(0, vol)));
                            break;
                        }
                    }
                    break;
                }
            }
        };
        socket.onclose = () => {
            const shouldReconnect =
                !storeRuntime.chatManualDisconnect &&
                !storeRuntime.chatAuthRejected &&
                get().isAuthenticated &&
                !get().isAdmin &&
                !get().needsPasswordReset &&
                get().isEngineConfigured &&
                Boolean(get().tenantId && get().sessionKey);

            set((state) => ({
                socket: state.socket === socket ? null : state.socket,
                status: storeRuntime.chatAuthRejected
                    ? 'error'
                    : shouldReconnect
                    ? 'connecting'
                    : 'disconnected',
            }));

            if (shouldReconnect) {
                const delayMs = Math.min(30_000, 1000 * Math.pow(2, storeRuntime.chatReconnectAttempts));
                storeRuntime.chatReconnectAttempts = Math.min(storeRuntime.chatReconnectAttempts + 1, 6);
                if (storeRuntime.chatReconnectTimer) clearTimeout(storeRuntime.chatReconnectTimer);
                storeRuntime.chatReconnectTimer = setTimeout(() => {
                    storeRuntime.chatReconnectTimer = null;
                    const cur = get();
                    if (
                        !storeRuntime.chatManualDisconnect &&
                        !storeRuntime.chatAuthRejected &&
                        cur.isAuthenticated &&
                        !cur.isAdmin &&
                        !cur.needsPasswordReset &&
                        cur.tenantId &&
                        cur.sessionKey &&
                        !cur.socket
                    ) {
                        void cur.connect(cur.tenantId, cur.sessionKey);
                    }
                }, delayMs);
            }
        };
        socket.onerror = () => {
            // Transient network errors trigger onclose right after onerror; let onclose manage backoff
            if (storeRuntime.chatAuthRejected) {
                set({ status: 'error' });
            }
        };
    },

    disconnect: () => {
        storeRuntime.chatManualDisconnect = true;
        if (storeRuntime.chatReconnectTimer) {
            clearTimeout(storeRuntime.chatReconnectTimer);
            storeRuntime.chatReconnectTimer = null;
        }
        const sock = get().socket;
        if (sock) {
            sock.onclose = null;
            sock.onerror = null;
            sock.close();
        }
        set({ socket: null, status: 'disconnected' });
        get().disconnectAgentStream();
        get().disconnectTrainingStream();
    },

    sendMessage: (prompt, modelOverride, isRetry = false) => {
        clearPendingRetryTimer();
        const { socket, messages } = get();
        const displayPrompt = prompt.replace(
            /\b(gsk_|AIza|sk-or-|sk-ant-|xai-|sk-)[A-Za-z0-9_-]{16,}\b/g,
            (match) => `${match.slice(0, 4)}••••${match.slice(-4)}`,
        );
        const prevAutoCount = isRetry
            ? (get().lastSubmittedPrompt?.autoRetryCount ?? get().pendingRetryPrompt?.autoRetryCount ?? 0) + 1
            : 0;
        const nextSubmitted = { prompt, displayPrompt, modelOverride, autoRetryCount: prevAutoCount };

        if (!socket || socket.readyState !== WebSocket.OPEN) {
            if (!isRetry) {
                set({
                    messages: [...messages, { id: `user-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, role: 'user', content: displayPrompt, type: 'text', timestamp: Date.now() }],
                    lastSubmittedPrompt: nextSubmitted,
                    pendingRetryPrompt: { ...nextSubmitted, errorMessage: 'Esperando reconexión del chat...' },
                });
            }
            return;
        }

        stopSpeaking();
        ttsPlayer.initialize();
        const musicState = useMusicStore.getState();
        const musicContext = musicState.isPlayerVisible && musicState.currentTrack
            ? `[CONTEXT: Currently playing "${musicState.currentTrack.title}"]\n`
            : '';
        const finalPrompt = musicContext + prompt;

        if (isRetry) {
            const cleanedMessages =
                messages.length > 0 && messages[messages.length - 1]?.role === 'system'
                    ? messages.slice(0, -1)
                    : messages;
            set({
                messages: cleanedMessages,
                status: 'thinking',
                lastSubmittedPrompt: nextSubmitted,
                pendingRetryPrompt: null,
            });
        } else {
            set({
                messages: [...messages, { id: `user-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, role: 'user', content: displayPrompt, type: 'text', timestamp: Date.now() }],
                lastSubmittedPrompt: nextSubmitted,
                pendingRetryPrompt: null,
            });
        }

        const payload: Record<string, unknown> = { prompt: finalPrompt, task_type: get().taskType };
        if (modelOverride) payload.model_override = modelOverride;
        socket.send(JSON.stringify(payload));
    },

    retryPendingMessage: (newModelOverride) => {
        const pending = get().pendingRetryPrompt;
        if (!pending) return;
        const effectiveOverride = newModelOverride !== undefined ? newModelOverride : pending.modelOverride;
        get().sendMessage(pending.prompt, effectiveOverride, true);
    },

    dismissPendingRetry: () => {
        clearPendingRetryTimer();
        set({ pendingRetryPrompt: null, lastSubmittedPrompt: null });
    },

    appendToken: (pid, token, type) => {
        set((state) => {
            const lastMessage = state.messages[state.messages.length - 1];
            const matchesStream = lastMessage?.role === 'assistant'
                && lastMessage.type === type
                && (lastMessage.id === pid || lastMessage.id.startsWith(`${pid}:${type}:`));
            if (matchesStream) {
                const updatedMessages = [...state.messages];
                updatedMessages[updatedMessages.length - 1] = { ...lastMessage, content: lastMessage.content + token };
                return { messages: updatedMessages };
            }
            const nextId = type === 'text' && !state.messages.some((m) => m.id === pid)
                ? pid
                : `${pid}:${type}:${state.messages.length}-${Date.now()}`;
            return { messages: [...state.messages, { id: nextId, role: 'assistant', content: token, type, timestamp: Date.now() }] };
        });
    },

    setStatus: (status) => set({ status }),

    // CORE-124: clearHistory es acción explícita del usuario — borra mensajes intencionalmente
    clearHistory: async () => {
        clearPendingRetryTimer();
        const { tenantId, sessionKey, socket } = get();
        if (tenantId && sessionKey) {
            try {
                await fetch('/api/chat/history', {
                    method: 'DELETE',
                    headers: {
                        'x-citadel-tenant': tenantId,
                        'x-citadel-key': sessionKey,
                    }
                });
            } catch (e) {
                console.error('Failed to clear backend chat history:', e);
            }
        }
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(JSON.stringify({ action: 'clear_history' }));
        }
        set({ messages: [], historyTotalCount: 0, nextHistoryBeforeIndex: 0, hasMoreHistory: false, lastSubmittedPrompt: null, pendingRetryPrompt: null });
    },

    clearAgentTraces: async () => {
        const { tenantId, sessionKey } = get();
        if (tenantId && sessionKey) {
            try {
                await fetch('/api/chat/traces', {
                    method: 'DELETE',
                    headers: {
                        'x-citadel-tenant': tenantId,
                        'x-citadel-key': sessionKey,
                    }
                });
            } catch (e) {
                console.error('Failed to clear backend agent traces:', e);
            }
        }
    },

    addSystemMessage: (content: string) => {
        set({ messages: [...get().messages, { id: `sys-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, role: 'system', content, type: 'system', timestamp: Date.now() }] });
    },
});
