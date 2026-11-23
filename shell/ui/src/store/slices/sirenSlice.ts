import { ttsPlayer } from '../../audio/TTSPlayer';
import { loadVoicePrefs, stopSpeaking } from '../../audio/naturalVoice';
import type { AegisState, StoreGet, StoreSet } from '../types';

interface CustomWindow extends Window {
    _aegis_webrtc_peer?: RTCPeerConnection;
    _aegis_webrtc_dc?: RTCDataChannel;
    _aegis_audio_stream?: MediaStream;
    _aegis_audio_ctx?: AudioContext;
    _aegis_audio_node?: AudioNode;
    _aegis_audio_gain?: GainNode;
    _aegis_speech_recognition?: unknown;
}

interface AegisAudioRefs {
    _aegis_audio_stream?: MediaStream;
    _aegis_audio_ctx?: AudioContext;
    _aegis_audio_node?: ScriptProcessorNode;
    _aegis_audio_gain?: GainNode;
}

interface WindowWithWebkit extends Window {
    AudioContext?: typeof AudioContext;
    webkitAudioContext?: typeof AudioContext;
}

export type SirenSlice = Pick<AegisState, 'setInputMode' | 'setVoiceEnabled' | 'fetchSirenConfig' | 'startSirenStream' | 'fetchAgentName' | 'startWakewordListener' | 'stopWakewordListener' | 'stopSirenStream' | 'restartMicForConversation'>;

export const createSirenSlice = (set: StoreSet, get: StoreGet): SirenSlice => ({
    setInputMode: (mode) => {
        const prev = get().inputMode;
        if (prev === 'wakeword') get().stopWakewordListener();
        if (prev !== 'text') get().stopSirenStream();
        set({ inputMode: mode });
        if (mode === 'wakeword') {
            get().fetchAgentName().then(() => get().startWakewordListener());
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
                }
            });
            if (res.ok) {
                const data = await res.json() as { stt_available?: boolean; stt_provider?: string; stt_api_key?: string };
                const resolvedProvider = data.stt_provider ?? 'browser';
                set({
                    sttAvailable: resolvedProvider === 'browser' ? true : (data.stt_available ?? true),
                    sttProvider: resolvedProvider,
                    sttApiKey: data.stt_api_key ?? '',
                });
            }
        } catch (err) {
            console.error('Fetch siren config error:', err);
        }
    },
    startSirenStream: async () => {
        const { tenantId, sessionKey, isRecording, sttProvider } = get();
        if (isRecording || !tenantId || !sessionKey) return;

        // CORE-231: contexto inseguro (HTTP sobre IP no-localhost) bloquea getUserMedia
        const isInsecureContext =
            window.location.protocol === 'http:' &&
            !['localhost', '127.0.0.1'].includes(window.location.hostname);
        if (isInsecureContext) {
            throw new Error('INSECURE_CONTEXT: Microphone requires HTTPS');
        }

        // ── Opción A: Browser WebSpeech API ──────────────────────────────
        if (sttProvider === 'browser') {
            const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (!SpeechRecognitionCtor) throw new Error('WebSpeech API no disponible en este navegador. Usá Chrome o Edge.');
            await ttsPlayer.initialize();
            const recognition = new SpeechRecognitionCtor();
            recognition.lang = loadVoicePrefs().lang || 'es-AR';
            recognition.continuous = false;
            recognition.interimResults = false;
            set({ isRecording: true, status: 'listening' });
            recognition.onresult = (event: SpeechRecognitionEvent) => {
                const transcript = event.results[0][0].transcript.trim();
                if (transcript) {
                    set({ isRecording: false, status: 'thinking' });
                    get().sendMessage(transcript);
                } else {
                    set({ isRecording: false, status: 'idle' });
                    if (get().inputMode === 'conversation' || loadVoicePrefs().autoContinueMic) {
                        get().restartMicForConversation().catch(console.error);
                    }
                }
            };
            recognition.onerror = () => set({ isRecording: false, status: 'idle' });
            recognition.onend = () => set({ isRecording: false });
            const audioRefs = window as Window & AegisAudioRefs;
            (audioRefs as unknown as Record<string, unknown>)._aegis_speech_recognition = recognition;
            recognition.start();
            return;
        }

        // ── Opciones B y C: Groq/Local via WebSocket PCM ─────────────────
        try {
            await ttsPlayer.initialize();
            const stream = await navigator.mediaDevices.getUserMedia({ audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true } });
            const win = window as WindowWithWebkit;
            const AudioCtx = win.AudioContext || win.webkitAudioContext;
            if (!AudioCtx) throw new Error('Web Audio API not supported');
            const ctx = new AudioCtx({ sampleRate: 16000 });
            const source = ctx.createMediaStreamSource(stream);
            const analyser = ctx.createAnalyser();
            analyser.fftSize = 256;
            analyser.smoothingTimeConstant = 0.2;
            // CORE-302: GainNode para mute durante TTS — analyser va a source directo (VAD sin filtrar)
            const gainNode = ctx.createGain();
            gainNode.gain.value = 1;
            source.connect(analyser);
            source.connect(gainNode);

            // Inicializar WebRTC PeerConnection
            const peer = new RTCPeerConnection({
                iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
            });

            const audioRefs = window as unknown as CustomWindow;
            audioRefs._aegis_webrtc_peer = peer;

            stream.getTracks().forEach(track => {
                peer.addTrack(track, stream);
            });

            const dc = peer.createDataChannel("siren_events");
            audioRefs._aegis_webrtc_dc = dc;

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
                    if (Date.now() - silenceStart > MAX_SILENCE_MS) {
                        vadRequestedStop = true;
                        if (dc.readyState === 'open') {
                            dc.send("VAD_END_SIGNAL");
                        }
                        if (get().inputMode === 'conversation') {
                            set({ isRecording: false });
                            stream.getTracks().forEach(t => t.stop());
                            ctx.close();
                            delete audioRefs._aegis_audio_stream;
                            delete audioRefs._aegis_audio_ctx;
                            delete audioRefs._aegis_audio_gain;
                        } else {
                            get().stopSirenStream();
                        }
                        return;
                    }
                } else silenceStart = Date.now();
                requestAnimationFrame(checkSilence);
            };

            dc.onopen = () => {
                console.log('📡 Siren WebRTC: DataChannel abierto.');
                set({ isRecording: true, sirenSocket: dc });
                requestAnimationFrame(checkSilence);
            };

            dc.onmessage = (event) => {
                interface SirenEventPayload {
                    event_type: string;
                    message: string;
                }
                interface SirenMessage {
                    event: string;
                    data: SirenEventPayload;
                }
                const msg = JSON.parse(event.data) as SirenMessage;
                if (msg.event === 'siren_event') {
                    const sirenEvent = msg.data;
                    if (sirenEvent.event_type === 'VAD_START') set({ status: 'listening' });
                    else if (sirenEvent.event_type === 'STT_START') set({ status: 'transcribing' });
                    else if (sirenEvent.event_type === 'STT_DONE') {
                        try {
                            const payload = JSON.parse(sirenEvent.message) as { transcript: string; pid: string };
                            set((state) => ({ messages: [...state.messages, { id: `voice-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, role: 'user', content: payload.transcript, type: 'text', timestamp: Date.now() }], activePid: payload.pid, status: 'thinking' }));
                            const chatSocket = get().socket;
                            if (chatSocket?.readyState === WebSocket.OPEN) chatSocket.send(JSON.stringify({ action: 'watch', pid: payload.pid }));
                            if (get().inputMode === 'audio') {
                                get().stopSirenStream();
                            } else if (get().inputMode === 'conversation') {
                                set({ isRecording: false });
                                if (audioRefs._aegis_audio_stream) {
                                    audioRefs._aegis_audio_stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
                                    delete audioRefs._aegis_audio_stream;
                                }
                                if (audioRefs._aegis_audio_ctx) {
                                    audioRefs._aegis_audio_ctx.close();
                                    delete audioRefs._aegis_audio_ctx;
                                }
                                delete audioRefs._aegis_audio_gain;
                            }
                        } catch (e) { console.error('Failed to parse STT_DONE payload', e); set({ status: 'idle' }); }
                    } else if (sirenEvent.event_type === 'STT_EMPTY') {
                        set({ status: 'idle' });
                        if (get().inputMode === 'audio') {
                            get().stopSirenStream();
                        } else if (get().inputMode === 'conversation') {
                            set({ isRecording: false });
                            if (audioRefs._aegis_audio_stream) {
                                audioRefs._aegis_audio_stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
                                delete audioRefs._aegis_audio_stream;
                            }
                            if (audioRefs._aegis_audio_ctx) {
                                audioRefs._aegis_audio_ctx.close();
                                delete audioRefs._aegis_audio_ctx;
                            }
                            delete audioRefs._aegis_audio_gain;
                            get().restartMicForConversation().catch(console.error);
                        } else if (get().inputMode === 'wakeword') {
                            get().stopSirenStream();
                            get().startWakewordListener();
                        }
                    } else if (sirenEvent.event_type === 'TTS_DONE') {
                        const gainRef = audioRefs._aegis_audio_gain;
                        if (gainRef) gainRef.gain.setTargetAtTime(1, gainRef.context.currentTime, 0.01);
                        if (get().inputMode === 'conversation') {
                            get().restartMicForConversation().catch(console.error);
                        }
                    } else if (sirenEvent.event_type === 'STT_ERROR') {
                        const errMsg = sirenEvent.message || 'STT transcription failed';
                        set({ status: 'error', lastError: errMsg });
                        if (get().inputMode === 'audio') {
                            get().stopSirenStream();
                        } else if (get().inputMode === 'conversation') {
                            set({ isRecording: false });
                            if (audioRefs._aegis_audio_stream) {
                                audioRefs._aegis_audio_stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
                                delete audioRefs._aegis_audio_stream;
                            }
                            if (audioRefs._aegis_audio_ctx) {
                                audioRefs._aegis_audio_ctx.close();
                                delete audioRefs._aegis_audio_ctx;
                            }
                            delete audioRefs._aegis_audio_gain;
                        }
                    }
                }
            };

            peer.ontrack = (event) => {
                console.log('🔊 Siren WebRTC: Track de audio TTS recibido.');
                const gainRef = audioRefs._aegis_audio_gain;
                if (gainRef) gainRef.gain.setTargetAtTime(0, gainRef.context.currentTime, 0.01);

                const remoteStream = new MediaStream([event.track]);
                let audioEl = document.getElementById('siren-webrtc-audio') as HTMLAudioElement;
                if (!audioEl) {
                    audioEl = document.createElement('audio');
                    audioEl.id = 'siren-webrtc-audio';
                    audioEl.style.display = 'none';
                    document.body.appendChild(audioEl);
                }
                audioEl.srcObject = remoteStream;
                audioEl.play().catch(e => console.error("Playback error:", e));
            };

            // Intercambio SDP (Signaling) por REST
            const offer = await peer.createOffer();
            await peer.setLocalDescription(offer);

            await new Promise<void>((resolve) => {
                if (peer.iceGatheringState === 'complete') {
                    resolve();
                } else {
                    const checkState = () => {
                        if (peer.iceGatheringState === 'complete') {
                            peer.removeEventListener('icegatheringstatechange', checkState);
                            resolve();
                        }
                    };
                    peer.addEventListener('icegatheringstatechange', checkState);
                }
            });

            const res = await fetch(`/api/siren/webrtc/offer/${encodeURIComponent(tenantId)}`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({ sdp: peer.localDescription?.sdp })
            });

            if (!res.ok) {
                throw new Error(`Siren WebRTC signaling failed: ${res.statusText}`);
            }

            const data = await res.json() as { sdp: string };
            await peer.setRemoteDescription(new RTCSessionDescription({ type: 'answer', sdp: data.sdp }));

            audioRefs._aegis_audio_stream = stream;
            audioRefs._aegis_audio_ctx = ctx;
            audioRefs._aegis_audio_gain = gainNode;
        } catch (error) { console.error('🎤 Siren Capture Error:', error); set({ isRecording: false }); throw error; }
    },

    fetchAgentName: async () => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/persona/name', {
                headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey }
            });
            if (res.ok) {
                const data = await res.json() as { name?: string };
                set({ agentName: data.name || null });
            }
        } catch { /* silencio */ }
    },

    startWakewordListener: () => {
        const { agentName, inputMode, isRecording } = get();
        if (inputMode !== 'wakeword' || isRecording || !agentName) return;

        const SpeechRecognitionCtor =
            window.SpeechRecognition || window.webkitSpeechRecognition;
        if (!SpeechRecognitionCtor) return;

        const recognition = new SpeechRecognitionCtor();
        recognition.continuous = true;
        recognition.interimResults = true;
        recognition.maxAlternatives = 2;

        recognition.onresult = (event: SpeechRecognitionEvent) => {
            const { agentName: name, inputMode: mode, isRecording: rec } = get();
            if (mode !== 'wakeword' || rec || !name) return;

            for (let i = event.resultIndex; i < event.results.length; i++) {
                for (let j = 0; j < event.results[i].length; j++) {
                    const transcript = event.results[i][j].transcript.toLowerCase().trim();
                    const nameLower = name.toLowerCase();
                    const idx = transcript.indexOf(nameLower);
                    if (idx === -1) continue;

                    // Texto después del nombre → comando incluido
                    const command = transcript.slice(idx + nameLower.length).trim();
                    recognition.stop();
                    delete (window as unknown as Record<string, unknown>)._aegis_wakeword_recognition;
                    set({ isWakewordListening: false });

                    if (command.length > 2) {
                        // Comando en la misma utterance — enviar directo
                        get().sendMessage(command);
                    } else {
                        // Solo el nombre — abrir mic para el comando
                        get().startSirenStream().catch(console.error);
                    }
                    return;
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
            if (mode === 'wakeword' && !rec) {
                try { recognition.start(); }
                catch { get().startWakewordListener(); }
            }
        };

        recognition.start();
        set({ isWakewordListening: true });
        (window as unknown as Record<string, unknown>)._aegis_wakeword_recognition = recognition;
    },

    stopWakewordListener: () => {
        const rec = (window as unknown as Record<string, unknown>)
            ._aegis_wakeword_recognition as SpeechRecognition | undefined;
        if (rec) {
            try { rec.stop(); } catch { /* ya detenido */ }
            delete (window as unknown as Record<string, unknown>)._aegis_wakeword_recognition;
        }
        set({ isWakewordListening: false });
    },

    stopSirenStream: () => {
        // Limpiar WebSpeech si estaba activo
        const speechRec = (window as unknown as Record<string, unknown>)._aegis_speech_recognition as SpeechRecognition | undefined;
        if (speechRec) {
            try { speechRec.stop(); } catch { /* already stopped */ }
            delete (window as unknown as Record<string, unknown>)._aegis_speech_recognition;
        }

        const { sirenSocket } = get();
        if (sirenSocket && sirenSocket.readyState === 'open') {
            try {
                sirenSocket.send("VAD_END_SIGNAL");
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

        const audioRefs = window as unknown as CustomWindow;
        if (audioRefs._aegis_webrtc_peer) {
            try {
                const peer = audioRefs._aegis_webrtc_peer as RTCPeerConnection;
                peer.close();
            } catch (e) {
                console.error('Siren WebRTC: Failed to close PeerConnection', e);
            }
            delete audioRefs._aegis_webrtc_peer;
        }
        if (audioRefs._aegis_webrtc_dc) {
            delete audioRefs._aegis_webrtc_dc;
        }

        const audioEl = document.getElementById('siren-webrtc-audio');
        if (audioEl) {
            audioEl.parentNode?.removeChild(audioEl);
        }

        if (audioRefs._aegis_audio_stream) {
            audioRefs._aegis_audio_stream.getTracks().forEach((track: MediaStreamTrack) => track.stop());
            delete audioRefs._aegis_audio_stream;
        }
        if (audioRefs._aegis_audio_node) {
            audioRefs._aegis_audio_node.disconnect();
            delete audioRefs._aegis_audio_node;
        }
        if (audioRefs._aegis_audio_ctx) {
            audioRefs._aegis_audio_ctx.close();
            delete audioRefs._aegis_audio_ctx;
        }
        if (audioRefs._aegis_audio_gain) {
            audioRefs._aegis_audio_gain.disconnect();
            delete audioRefs._aegis_audio_gain;
        }
        set({ isRecording: false });
    },

    restartMicForConversation: async () => {
        const { tenantId, sessionKey, sirenSocket, isRecording, sttProvider } = get();
        if (isRecording || !tenantId || !sessionKey) return;

        // Modo browser (WebSpeech): iniciar nuevo recognition
        if (sttProvider === 'browser') {
            const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (!SpeechRecognitionCtor) return;
            await ttsPlayer.initialize();
            const recognition = new SpeechRecognitionCtor();
            recognition.lang = loadVoicePrefs().lang || 'es-AR';
            recognition.continuous = false;
            recognition.interimResults = false;
            set({ isRecording: true, status: 'listening' });
            recognition.onresult = (event: SpeechRecognitionEvent) => {
                const transcript = event.results[0][0].transcript.trim();
                if (transcript) {
                    set({ isRecording: false, status: 'thinking' });
                    get().sendMessage(transcript);
                } else {
                    set({ isRecording: false, status: 'idle' });
                    if (get().inputMode === 'conversation' || loadVoicePrefs().autoContinueMic) {
                        get().restartMicForConversation().catch(console.error);
                    }
                }
            };
            recognition.onerror = () => set({ isRecording: false, status: 'idle' });
            recognition.onend = () => set({ isRecording: false });
            (window as unknown as Record<string, unknown>)._aegis_speech_recognition = recognition;
            recognition.start();
            return;
        }

        // Modo Groq/Local (WebRTC): el sirenSocket (RTCDataChannel) ya está abierto — solo reabrir el mic
        if (!sirenSocket || sirenSocket.readyState !== 'open') {
            // sirenSocket se cerró inesperadamente — reiniciar todo
            await get().startSirenStream();
            return;
        }

        try {
            await ttsPlayer.initialize();
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true }
            });
            const win = window as WindowWithWebkit;
            const AudioCtx = win.AudioContext || win.webkitAudioContext;
            if (!AudioCtx) throw new Error('Web Audio API not supported');
            const ctx = new AudioCtx({ sampleRate: 16000 });
            const source = ctx.createMediaStreamSource(stream);
            const analyser = ctx.createAnalyser();
            analyser.fftSize = 256;
            analyser.smoothingTimeConstant = 0.2;
            // CORE-302: GainNode para mute durante TTS — analyser va a source directo (VAD sin filtrar)
            const gainNode = ctx.createGain();
            gainNode.gain.value = 1;
            source.connect(analyser);
            source.connect(gainNode);

            const audioRefs = window as unknown as CustomWindow;
            const peer = audioRefs._aegis_webrtc_peer as RTCPeerConnection | undefined;
            if (peer) {
                const newTrack = stream.getAudioTracks()[0];
                const sender = peer.getSenders().find(s => s.track?.kind === 'audio');
                if (sender && newTrack) {
                    await sender.replaceTrack(newTrack);
                } else if (newTrack) {
                    peer.addTrack(newTrack, stream);
                }
            }

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
                    if (Date.now() - silenceStart > MAX_SILENCE_MS) {
                        vadRequestedStop = true;
                        if (sirenSocket && sirenSocket.readyState === 'open') {
                            sirenSocket.send("VAD_END_SIGNAL");
                        }
                        if (get().inputMode === 'conversation') {
                            set({ isRecording: false });
                            stream.getTracks().forEach(t => t.stop());
                            ctx.close();
                            delete audioRefs._aegis_audio_stream;
                            delete audioRefs._aegis_audio_ctx;
                            delete audioRefs._aegis_audio_gain;
                        } else {
                            get().stopSirenStream();
                        }
                        return;
                    }
                } else {
                    silenceStart = Date.now();
                }
                requestAnimationFrame(checkSilence);
            };

            audioRefs._aegis_audio_stream = stream;
            audioRefs._aegis_audio_ctx = ctx;
            audioRefs._aegis_audio_gain = gainNode;

            set({ isRecording: true, status: 'listening' });
            requestAnimationFrame(checkSilence);
        } catch (error) {
            console.error('🎤 Mic restart error:', error);
            set({ isRecording: false });
        }
    },
});
