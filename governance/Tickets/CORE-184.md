# CORE-184 — fix: modo conversación — sirenWs persistente + TTS loop sin botón

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-183 (implementado)

---

## Contexto

CORE-183 implementó el selector de modos pero el modo `conversation` tiene dos bugs
que lo hacen inutilizable en producción:

1. **Sin voz en respuesta**: el kernel manda los `tts_audio_chunk` por el `sirenWs`,
   pero ese WebSocket se cierra cuando el VAD detecta silencio (fin de la grabación).
   Cuando el kernel termina de procesar y quiere mandar audio, el canal ya no existe.

2. **Hay que presionar el botón cada vez**: en modo `browser` (WebSpeech API),
   `recognition.continuous = false` hace que el reconocimiento termine solo. El
   `STATE_COMPLETED` intenta reactivar el mic con un `setTimeout`, pero como no hay
   `sirenWs` activo, `ttsPlayer.getAudioContext()` devuelve `null` y el delay es 0ms —
   compitiendo con el procesamiento del kernel y causando condiciones de carrera.

---

## Causa raíz

El modelo mental de CORE-183 era incorrecto: asumió que el `sirenWs` era solo para
STT (entrada de audio). En realidad el `sirenWs` es el canal **bidireccional** de
Siren — STT de entrada Y TTS de salida viajan por el mismo WebSocket.

**El `sirenWs` debe permanecer abierto durante toda la conversación en modo `conversation`.**

---

## Arquitectura correcta

```
Modo TEXT:       textarea → sendMessage → socket → kernel → appendToken
Modo AUDIO:      mic → sirenWs → STT_DONE → sendMessage → socket → kernel → appendToken
                 [sirenWs cierra después de STT_DONE]
Modo CONV:       mic → sirenWs → STT_DONE → watch pid → kernel → tts_audio_chunk → sirenWs → ttsPlayer
                 [sirenWs permanece abierto — NO cierra hasta que el usuario cambia de modo]
                 [mic se reactiva automáticamente después de que termina el TTS]
```

---

## Cambios Requeridos

### 1. `useAegisStore.ts` — separar ciclo de vida del sirenWs por modo

**El problema central está en `startSirenStream` y en el handler `sirenWs.onclose`.**

#### A. `sirenWs.onclose` — no llamar `stopSirenStream` en modo conversación

```typescript
// ANTES
sirenWs.onclose = () => get().stopSirenStream();

// DESPUÉS
sirenWs.onclose = () => {
    // En modo conversación el sirenWs cierra cuando el usuario cambia de modo
    // o hace logout — no al terminar la grabación
    if (get().inputMode !== 'conversation') {
        get().stopSirenStream();
    } else {
        // Solo limpiar el estado de grabación, NO cerrar el socket ni el audio ctx
        set({ isRecording: false });
    }
};
```

#### B. `STT_DONE` — en modo conversación NO cerrar sirenWs, solo detener el mic

```typescript
else if (sirenEvent.event_type === 'STT_DONE') {
    try {
        const payload = JSON.parse(sirenEvent.message as string) as { transcript: string; pid: string };
        set((state) => ({
            messages: [...state.messages, {
                id: `voice-${Date.now()}`,
                role: 'user',
                content: payload.transcript,
                type: 'text',
                timestamp: Date.now()
            }],
            activePid: payload.pid,
            status: 'thinking'
        }));

        const chatSocket = get().socket;
        if (chatSocket?.readyState === WebSocket.OPEN) {
            chatSocket.send(JSON.stringify({ action: 'watch', pid: payload.pid }));
        }

        if (get().inputMode === 'audio') {
            // Modo audio: cerrar sirenWs completamente
            get().stopSirenStream();
        } else if (get().inputMode === 'conversation') {
            // Modo conversación: solo detener captura de mic, mantener sirenWs abierto
            // El sirenWs sigue escuchando tts_audio_chunk del kernel
            set({ isRecording: false });
            // Detener el stream de audio del mic pero NO cerrar el WebSocket
            const audioRefs = window as Window & AegisAudioRefs;
            if (audioRefs._aegis_audio_stream) {
                audioRefs._aegis_audio_stream.getTracks().forEach(track => track.stop());
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
        }
    } catch (e) {
        console.error('Failed to parse STT_DONE payload', e);
        set({ status: 'idle' });
    }
}
```

#### C. `tts_audio_chunk` — reproducir chunks que llegan DESPUÉS de cerrada la grabación

El handler de `tts_audio_chunk` ya existe y es correcto. No cambia. Lo que cambia es
que ahora el `sirenWs` está abierto para recibirlos.

#### D. `STATE_COMPLETED` — reactivar mic después del TTS en modo conversación

```typescript
if (su.state === 'STATE_COMPLETED') {
    set({ status: 'idle', activePid: null });

    if (get().inputMode === 'conversation') {
        // Esperar a que termine el audio del TTSPlayer y reactivar el mic
        const scheduleReactivation = () => {
            const ctx = ttsPlayer.getAudioContext();
            if (ctx) {
                const remaining = (ttsPlayer.getNextStartTime() - ctx.currentTime) * 1000;
                const delay = Math.max(300, remaining + 200); // mínimo 300ms
                setTimeout(() => {
                    if (get().inputMode === 'conversation' && !get().isRecording) {
                        // Reactivar solo el mic — el sirenWs ya está abierto
                        get().restartMicForConversation().catch(console.error);
                    }
                }, delay);
            } else {
                // Sin TTS activo, reactivar después de un delay fijo
                setTimeout(() => {
                    if (get().inputMode === 'conversation' && !get().isRecording) {
                        get().restartMicForConversation().catch(console.error);
                    }
                }, 500);
            }
        };
        scheduleReactivation();
    }
}
```

#### E. Nueva acción `restartMicForConversation` — solo reinicia el mic, no el sirenWs

Agregar a la interfaz `AegisState`:

```typescript
restartMicForConversation: () => Promise<void>;
```

Implementación:

```typescript
restartMicForConversation: async () => {
    const { tenantId, sessionKey, sirenSocket, isRecording, sttProvider } = get();
    if (isRecording || !tenantId || !sessionKey) return;

    // Modo browser (WebSpeech): iniciar nuevo recognition
    if (sttProvider === 'browser') {
        const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
        if (!SpeechRecognitionCtor) return;
        await ttsPlayer.initialize();
        const recognition = new SpeechRecognitionCtor();
        recognition.continuous = false;
        recognition.interimResults = false;
        set({ isRecording: true, status: 'listening' });
        recognition.onresult = (event: SpeechRecognitionEvent) => {
            const transcript = event.results[0][0].transcript.trim();
            set({ isRecording: false, status: 'thinking' });
            if (transcript) get().sendMessage(transcript);
        };
        recognition.onerror = () => set({ isRecording: false, status: 'idle' });
        recognition.onend = () => set({ isRecording: false });
        (window as unknown as Record<string, unknown>)._aegis_speech_recognition = recognition;
        recognition.start();
        return;
    }

    // Modo Groq/Local: el sirenWs ya está abierto — solo reabrir el mic
    if (!sirenSocket || sirenSocket.readyState !== WebSocket.OPEN) {
        // sirenWs se cerró inesperadamente — reiniciar todo
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
                if (Date.now() - silenceStart > MAX_SILENCE_MS) {
                    vadRequestedStop = true;
                    // Solo detener mic, NO cerrar sirenWs
                    set({ isRecording: false });
                    stream.getTracks().forEach(t => t.stop());
                    scriptNode.disconnect();
                    ctx.close();
                    return;
                }
            } else {
                silenceStart = Date.now();
            }
            requestAnimationFrame(checkSilence);
        };

        scriptNode.onaudioprocess = (audioEvent: AudioProcessingEvent) => {
            if (sirenSocket.readyState !== WebSocket.OPEN) return;
            const inputData = audioEvent.inputBuffer.getChannelData(0);
            const pcmBuffer = new Int16Array(inputData.length);
            for (let i = 0; i < inputData.length; i++) {
                const s = Math.max(-1, Math.min(1, inputData[i]));
                pcmBuffer[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
            }
            sirenSocket.send(pcmBuffer.buffer);
        };

        source.connect(scriptNode);
        scriptNode.connect(ctx.destination);

        const audioRefs = window as Window & AegisAudioRefs;
        audioRefs._aegis_audio_stream = stream;
        audioRefs._aegis_audio_ctx = ctx;
        audioRefs._aegis_audio_node = scriptNode;

        set({ isRecording: true, status: 'listening' });
        requestAnimationFrame(checkSilence);
    } catch (error) {
        console.error('🎤 Mic restart error:', error);
        set({ isRecording: false });
    }
},
```

#### F. `stopSirenStream` — en modo conversación, también limpiar sirenWs

Cuando el usuario cambia de modo o hace logout, `stopSirenStream` debe cerrar todo:

```typescript
stopSirenStream: () => {
    // Limpiar WebSpeech si estaba activo
    const speechRec = (window as unknown as Record<string, unknown>)
        ._aegis_speech_recognition as SpeechRecognition | undefined;
    if (speechRec) {
        try { speechRec.stop(); } catch { /* already stopped */ }
        delete (window as unknown as Record<string, unknown>)._aegis_speech_recognition;
    }

    const { sirenSocket } = get();
    if (sirenSocket?.readyState === WebSocket.OPEN) {
        sirenSocket.send(JSON.stringify({
            sequence_number: 9999, data: '', format: 'VAD_END_SIGNAL', sample_rate: 16000
        }));
        setTimeout(() => {
            if (sirenSocket.readyState === WebSocket.OPEN) sirenSocket.close();
        }, 100);
    } else if (sirenSocket) {
        sirenSocket.close();
    }
    set({ sirenSocket: null });

    const audioRefs = window as Window & AegisAudioRefs;
    if (audioRefs._aegis_audio_stream) {
        audioRefs._aegis_audio_stream.getTracks().forEach(track => track.stop());
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
    set({ isRecording: false });
},
```

### 2. `InputModeSelector.tsx` — al cambiar de modo llamar `stopSirenStream`

Esto ya está implementado en CORE-183. Verificar que el `handleSelect` llame
`stopSirenStream()` antes de `setInputMode()`. No debería necesitar cambios.

---

## Criterios de Aceptación

- [ ] **Modo conversación — TTS**: después de hablar, el asistente responde con voz sin necesidad de presionar ningún botón
- [ ] **Modo conversación — loop**: después del TTS, el mic se reactiva automáticamente y el StatusBadge muestra `Listening`
- [ ] **Modo conversación — sirenWs**: el WebSocket Siren permanece abierto entre turnos (verificar en DevTools Network tab)
- [ ] **Modo audio**: el sirenWs se cierra después de `STT_DONE`, sin TTS, sin reactivación del mic
- [ ] **Modo texto**: sin cambios, sin mic, sin sirenWs
- [ ] Cambiar de modo `conversation` → `text` mientras el mic está activo detiene todo limpiamente
- [ ] Logout cierra el sirenWs correctamente en cualquier modo
- [ ] `npm run build` sin errores TypeScript

---

## Commit message

```
fix(shell): CORE-184 modo conversación — sirenWs persistente + TTS loop sin botón
```

---

*Arquitecto IA — 2026-04-25*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Modo conversación implementado: el store re-graba automáticamente en mode==='conversation' tras el TTS (useAegisStore.ts:541+).
