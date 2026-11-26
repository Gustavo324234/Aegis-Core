import { ttsPlayer } from '../../audio/TTSPlayer';
import { loadVoicePrefs, stopSpeaking } from '../../audio/naturalVoice';
import {
    playWakeChime,
    startBreathAwareUtteranceCapture,
    startContinuousWakeMonitor,
    type CaptureHandle,
} from '../../audio/smartVadRecorder';
import type { AegisState, StoreGet, StoreSet } from '../types';

interface CustomWindow extends Window {
    _aegis_webrtc_peer?: RTCPeerConnection;
    _aegis_webrtc_dc?: RTCDataChannel;
    _aegis_audio_stream?: MediaStream;
    _aegis_audio_ctx?: AudioContext;
    _aegis_audio_node?: AudioNode;
    _aegis_audio_gain?: GainNode;
    _aegis_speech_recognition?: SpeechRecognition;
    _aegis_wakeword_recognition?: SpeechRecognition;
    _aegis_smart_vad_capture?: CaptureHandle;
    _aegis_wake_monitor?: CaptureHandle;
    _aegis_speech_flush_timer?: ReturnType<typeof setTimeout>;
}

export type SirenSlice = Pick<
    AegisState,
    | 'setInputMode'
    | 'setVoiceEnabled'
    | 'fetchSirenConfig'
    | 'startSirenStream'
    | 'fetchAgentName'
    | 'startWakewordListener'
    | 'stopWakewordListener'
    | 'armWakeFollowupWindow'
    | 'stopSirenStream'
    | 'restartMicForConversation'
>;

function stripWakeAndBuildVariants(agentName: string): string[] {
    const clean = (agentName || 'Aegis')
        .toLowerCase()
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .trim();
    const list = new Set<string>(['aegis', 'eyis', 'egis', 'ayis']);
    if (clean) list.add(clean);
    return Array.from(list);
}

export const createSirenSlice = (set: StoreSet, get: StoreGet): SirenSlice => {
    const submitUtteranceToKernel = async (pcmB64: string, mode: 'direct' | 'wakeword' | 'followup') => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey || !pcmB64) return;
        const lang = loadVoicePrefs().lang || 'es-AR';

        if (mode === 'direct') {
            set({ status: 'transcribing' });
        }

        try {
            const res = await fetch('/api/siren/utterance', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({
                    pcm_b64: pcmB64,
                    sample_rate: 16000,
                    mode,
                    lang,
                }),
            });
            const data = await res.json();
            if (!res.ok) {
                if (mode === 'direct') {
                    set({ status: 'error', lastError: data.error || 'Error transcribiendo audio' });
                }
                return;
            }

            if (data.agent_name && data.agent_name !== get().agentName) {
                set({ agentName: data.agent_name });
            }

            if (!data.accepted) {
                if (data.reason === 'speaker_mismatch') {
                    playWakeChime('rejected');
                    if (mode === 'direct') {
                        set({
                            status: 'error',
                            lastError: `Voz no reconocida (${Math.round((data.score ?? 0) * 100)}%)`,
                        });
                    }
                } else if (mode === 'direct') {
                    set({ status: 'idle' });
                    if (get().inputMode === 'conversation' || loadVoicePrefs().autoContinueMic) {
                        get().restartMicForConversation().catch(console.error);
                    }
                }
                return;
            }

            if (data.matched_speaker) {
                set({ lastWakeSpeaker: data.matched_speaker });
            }

            if (data.wake_only) {
                playWakeChime('ready');
                get().armWakeFollowupWindow(Math.max(10, loadVoicePrefs().followupWindowSecs || 8));
                return;
            }

            const command = String(data.command || '').trim();
            if (command) {
                playWakeChime('done');
                set({ status: 'thinking' });
                get().sendMessage(command);
            } else if (mode === 'direct') {
                set({ status: 'idle' });
            }
        } catch (err) {
            if (mode === 'direct') {
                set({
                    status: 'error',
                    lastError: err instanceof Error ? err.message : 'Error de voz',
                });
            }
        }
    };

    const startBrowserBreathAwareDictation = async () => {
        const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
        if (!SpeechRecognitionCtor) {
            throw new Error('WebSpeech API no disponible en este navegador. Configurá Groq en Motores o usá Chrome/Edge.');
        }

        await ttsPlayer.initialize();
        const recognition = new SpeechRecognitionCtor();
        const prefs = loadVoicePrefs();
        const hangoverMs = prefs.silenceHangoverMs ?? 2400;

        recognition.lang = prefs.lang || 'es-AR';
        recognition.continuous = true;
        recognition.interimResults = true;

        const win = window as unknown as CustomWindow;
        let finalizedText = '';
        let latestInterim = '';
        let dispatched = false;

        const clearFlushTimer = () => {
            if (win._aegis_speech_flush_timer) {
                clearTimeout(win._aegis_speech_flush_timer);
                delete win._aegis_speech_flush_timer;
            }
        };

        const flushAndSend = () => {
            if (dispatched) return;
            dispatched = true;
            clearFlushTimer();
            try {
                recognition.stop();
            } catch {
                // ignore
            }
            delete win._aegis_speech_recognition;
            const fullText = `${finalizedText} ${latestInterim}`.trim();
            if (fullText) {
                set({ isRecording: false, status: 'thinking' });
                get().sendMessage(fullText);
            } else {
                set({ isRecording: false, status: 'idle' });
                if (get().inputMode === 'conversation' || loadVoicePrefs().autoContinueMic) {
                    get().restartMicForConversation().catch(console.error);
                }
            }
        };

        const armPauseTimer = () => {
            clearFlushTimer();
            win._aegis_speech_flush_timer = setTimeout(flushAndSend, hangoverMs);
        };

        set({ isRecording: true, status: 'listening' });

        recognition.onresult = (event: SpeechRecognitionEvent) => {
            let finalSegment = '';
            let interimSegment = '';
            for (let i = 0; i < event.results.length; i++) {
                const res = event.results[i];
                const txt = res[0]?.transcript?.trim() ?? '';
                if (!txt) continue;
                if (res.isFinal) {
                    finalSegment += (finalSegment ? ' ' : '') + txt;
                } else {
                    interimSegment += (interimSegment ? ' ' : '') + txt;
                }
            }
            finalizedText = finalSegment;
            latestInterim = interimSegment;
            armPauseTimer();
        };

        recognition.onerror = () => {
            clearFlushTimer();
            if (!dispatched) {
                set({ isRecording: false, status: 'idle' });
            }
        };

        recognition.onend = () => {
            if (!dispatched) {
                const fullText = `${finalizedText} ${latestInterim}`.trim();
                if (fullText) {
                    flushAndSend();
                } else {
                    set({ isRecording: false, status: 'idle' });
                }
            }
        };

        win._aegis_speech_recognition = recognition;
        recognition.start();
    };

    const startSmartPcmDictation = async () => {
        await ttsPlayer.initialize();
        const prefs = loadVoicePrefs();
        const win = window as unknown as CustomWindow;
        set({ isRecording: true, status: 'listening', lastError: null });

        const handle = startBreathAwareUtteranceCapture({
            silenceHangoverMs: prefs.silenceHangoverMs ?? 2400,
            initialWaitMs: 10000,
            onSpeechStart: () => set({ status: 'listening' }),
            onComplete: async (result) => {
                delete win._aegis_smart_vad_capture;
                set({ isRecording: false });
                if (!result.hadSpeech || !result.pcmB64) {
                    set({ status: 'idle' });
                    if (get().inputMode === 'conversation' || loadVoicePrefs().autoContinueMic) {
                        get().restartMicForConversation().catch(console.error);
                    }
                    return;
                }
                await submitUtteranceToKernel(result.pcmB64, 'direct');
            },
            onError: (err) => {
                delete win._aegis_smart_vad_capture;
                set({ isRecording: false, status: 'error', lastError: err.message });
            },
        });

        win._aegis_smart_vad_capture = handle;
    };

    return {
        armWakeFollowupWindow: (secs) => {
            const windowSecs = secs ?? (loadVoicePrefs().followupWindowSecs || 8);
            set({ wakeFollowupUntil: Date.now() + windowSecs * 1000 });
        },

        setInputMode: (mode) => {
            const prev = get().inputMode;
            if (prev === 'wakeword') get().stopWakewordListener();
            if (prev !== 'text') get().stopSirenStream();
            set({ inputMode: mode });
            if (mode === 'wakeword') {
                get()
                    .fetchSirenConfig()
                    .then(() => get().fetchAgentName())
                    .then(() => get().startWakewordListener());
            }
        },

        setVoiceEnabled: (val) => {
            if (!val) stopSpeaking();
            set({ voiceEnabled: val });
        },

        fetchSirenConfig: async () => {
            const { tenantId, sessionKey } = get();
            if (!tenantId || !sessionKey) return;
            try {
                const res = await fetch('/api/siren/config', {
                    headers: {
                        'x-citadel-tenant': tenantId,
                        'x-citadel-key': sessionKey,
                    },
                });
                if (res.ok) {
                    const data = (await res.json()) as {
                        stt_available?: boolean;
                        stt_provider?: string;
                        stt_api_key?: string;
                        groq_key_detected?: boolean;
                        stt_ready?: boolean;
                    };
                    const resolvedProvider = data.stt_provider ?? 'browser';
                    const sttReady = Boolean(data.stt_ready || data.groq_key_detected || data.stt_available);
                    set({
                        sttAvailable: resolvedProvider === 'browser' ? true : sttReady,
                        sttProvider: resolvedProvider,
                        sttApiKey: data.stt_api_key ?? '',
                        groqKeyDetected: Boolean(data.groq_key_detected),
                        sttReady,
                    });
                }
            } catch (err) {
                console.error('Fetch siren config error:', err);
            }
        },

        startSirenStream: async () => {
            const { tenantId, sessionKey, isRecording, sttProvider, sttReady, groqKeyDetected } = get();
            if (isRecording || !tenantId || !sessionKey) return;

            const isInsecureContext =
                window.location.protocol === 'http:' &&
                !['localhost', '127.0.0.1'].includes(window.location.hostname);
            if (isInsecureContext) {
                throw new Error('INSECURE_CONTEXT: Microphone requires HTTPS');
            }

            // Prefer Breath-Aware Smart VAD + /api/siren/utterance whenever Groq/Local Whisper is ready.
            if (sttReady || groqKeyDetected || sttProvider !== 'browser') {
                await startSmartPcmDictation();
                return;
            }

            // Browser WebSpeech fallback (breath-tolerant continuous accumulator).
            await startBrowserBreathAwareDictation();
        },

        fetchAgentName: async () => {
            const { tenantId, sessionKey } = get();
            if (!tenantId || !sessionKey) {
                set({ agentName: 'Aegis' });
                return;
            }
            try {
                const res = await fetch('/api/persona/name', {
                    headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                });
                if (res.ok) {
                    const data = (await res.json()) as { name?: string };
                    set({ agentName: data.name?.trim() || 'Aegis' });
                    return;
                }
            } catch {
                // fallback below
            }
            set({ agentName: get().agentName || 'Aegis' });
        },

        startWakewordListener: () => {
            const { agentName, inputMode, isRecording, sttReady, groqKeyDetected, sttProvider, isWakewordListening } = get();
            if (inputMode !== 'wakeword' || isRecording || isWakewordListening) return;

            const effectiveName = agentName?.trim() || 'Aegis';
            if (!agentName) set({ agentName: effectiveName });

            const win = window as unknown as CustomWindow;
            const useServerWake = sttReady || groqKeyDetected || sttProvider !== 'browser';

            if (useServerWake) {
                const monitor = startContinuousWakeMonitor({
                    getHangoverMs: () => {
                        const inFollowup = Date.now() < get().wakeFollowupUntil;
                        return inFollowup ? loadVoicePrefs().silenceHangoverMs || 2400 : 1400;
                    },
                    isPaused: () => {
                        const st = get().status;
                        const ttsBusy = typeof window !== 'undefined' && Boolean(window.speechSynthesis?.speaking);
                        return st === 'thinking' || st === 'transcribing' || st === 'executing_syscall' || ttsBusy;
                    },
                    onSegmentReady: async (result) => {
                        if (get().inputMode !== 'wakeword') return;
                        const inFollowup = Date.now() < get().wakeFollowupUntil;
                        await submitUtteranceToKernel(result.pcmB64, inFollowup ? 'followup' : 'wakeword');
                    },
                    onError: (err) => {
                        console.error('Continuous wake monitor error:', err);
                        delete win._aegis_wake_monitor;
                        set({ isWakewordListening: false });
                    },
                });
                win._aegis_wake_monitor = monitor;
                set({ isWakewordListening: true });
                return;
            }

            // Fallback WebSpeech WakeWord when no server STT key is configured
            const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (!SpeechRecognitionCtor) return;

            const recognition = new SpeechRecognitionCtor();
            recognition.continuous = true;
            recognition.interimResults = true;
            recognition.lang = loadVoicePrefs().lang || 'es-AR';
            recognition.maxAlternatives = 2;

            recognition.onresult = (event: SpeechRecognitionEvent) => {
                const { agentName: currentName, inputMode: mode, isRecording: rec } = get();
                if (mode !== 'wakeword' || rec) return;

                const variants = stripWakeAndBuildVariants(currentName || 'Aegis');
                for (let i = event.resultIndex; i < event.results.length; i++) {
                    const res = event.results[i];
                    if (!res.isFinal) continue;
                    const raw = res[0]?.transcript?.trim() ?? '';
                    const folded = raw.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '');

                    for (const alias of variants) {
                        const re = new RegExp(`^(?:hola|oye|ok|hey|che)?\\s*(?:${alias}(?:\\s+escuchame)?|escuchame\\s+${alias})[,:\\s]*(.*)$`, 'i');
                        const match = folded.match(re);
                        if (match) {
                            const cmd = (match[1] || '').trim();
                            get().stopWakewordListener();
                            if (cmd.length > 1) {
                                get().sendMessage(cmd);
                            } else {
                                playWakeChime('ready');
                                get().startSirenStream().catch(console.error);
                            }
                            return;
                        }
                    }
                }
            };

            recognition.onerror = (event: SpeechRecognitionErrorEvent) => {
                if (event.error === 'aborted' || event.error === 'no-speech') return;
                setTimeout(() => {
                    if (get().inputMode === 'wakeword' && !get().isRecording) {
                        get().startWakewordListener();
                    }
                }, 2000);
            };

            recognition.onend = () => {
                const { inputMode: mode, isRecording: rec } = get();
                if (mode === 'wakeword' && !rec && get().isWakewordListening) {
                    try {
                        recognition.start();
                    } catch {
                        set({ isWakewordListening: false });
                        get().startWakewordListener();
                    }
                }
            };

            recognition.start();
            set({ isWakewordListening: true });
            win._aegis_wakeword_recognition = recognition;
        },

        stopWakewordListener: () => {
            const win = window as unknown as CustomWindow;
            if (win._aegis_wake_monitor) {
                win._aegis_wake_monitor.cancel();
                delete win._aegis_wake_monitor;
            }
            const rec = win._aegis_wakeword_recognition;
            if (rec) {
                try {
                    rec.stop();
                } catch {
                    // already stopped
                }
                delete win._aegis_wakeword_recognition;
            }
            set({ isWakewordListening: false });
        },

        stopSirenStream: () => {
            const win = window as unknown as CustomWindow;
            if (win._aegis_speech_flush_timer) {
                clearTimeout(win._aegis_speech_flush_timer);
                delete win._aegis_speech_flush_timer;
            }
            if (win._aegis_smart_vad_capture) {
                // Pressing stop sends what was captured so far without losing the utterance.
                win._aegis_smart_vad_capture.stopNow();
                delete win._aegis_smart_vad_capture;
            }
            if (win._aegis_speech_recognition) {
                try {
                    win._aegis_speech_recognition.stop();
                } catch {
                    // already stopped
                }
                delete win._aegis_speech_recognition;
            }

            const { sirenSocket } = get();
            if (sirenSocket && sirenSocket.readyState === 'open') {
                try {
                    sirenSocket.send('VAD_END_SIGNAL');
                } catch (e) {
                    console.error('Siren WebRTC: Failed to send VAD_END_SIGNAL', e);
                }
            }
            if (sirenSocket) {
                try {
                    sirenSocket.close();
                } catch (e) {
                    console.error('Siren WebRTC: Failed to close DataChannel', e);
                }
            }
            set({ sirenSocket: null });

            if (win._aegis_webrtc_peer) {
                try {
                    win._aegis_webrtc_peer.close();
                } catch (e) {
                    console.error('Siren WebRTC: Failed to close PeerConnection', e);
                }
                delete win._aegis_webrtc_peer;
            }
            if (win._aegis_webrtc_dc) delete win._aegis_webrtc_dc;

            const audioEl = document.getElementById('siren-webrtc-audio');
            if (audioEl) audioEl.parentNode?.removeChild(audioEl);

            if (win._aegis_audio_stream) {
                win._aegis_audio_stream.getTracks().forEach((track) => track.stop());
                delete win._aegis_audio_stream;
            }
            if (win._aegis_audio_node) {
                win._aegis_audio_node.disconnect();
                delete win._aegis_audio_node;
            }
            if (win._aegis_audio_ctx) {
                win._aegis_audio_ctx.close().catch(() => {});
                delete win._aegis_audio_ctx;
            }
            if (win._aegis_audio_gain) {
                win._aegis_audio_gain.disconnect();
                delete win._aegis_audio_gain;
            }
            set({ isRecording: false });
        },

        restartMicForConversation: async () => {
            const { isRecording } = get();
            if (isRecording) return;
            await get().startSirenStream();
        },
    };
};
