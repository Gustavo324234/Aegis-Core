/**
 * smartVadRecorder.ts (EPIC 63 / CORE-413)
 * - 16 kHz PCM16 mono capture with linear downsampling
 * - 500ms circular pre-roll so the first syllable is never clipped
 * - Adaptive room noise floor + breath-tolerant hangover (default 2400ms)
 * - Continuous local wake-word standby (0 tokens spent during silence)
 */

export interface UtteranceResult {
    pcmB64: string;
    sampleRate: number;
    durationMs: number;
    hadSpeech: boolean;
}

export interface CaptureHandle {
    stopNow: () => void;
    cancel: () => void;
}

export function playWakeChime(kind: 'ready' | 'done' | 'rejected'): void {
    if (typeof window === 'undefined') return;
    const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtx) return;

    try {
        const ctx = new AudioCtx();
        const now = ctx.currentTime;
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);

        if (kind === 'ready') {
            osc.frequency.setValueAtTime(587.33, now);
            osc.frequency.setValueAtTime(880.0, now + 0.06);
            gain.gain.setValueAtTime(0.08, now);
            gain.gain.exponentialRampToValueAtTime(0.001, now + 0.16);
            osc.start(now);
            osc.stop(now + 0.17);
        } else if (kind === 'done') {
            osc.frequency.setValueAtTime(783.99, now);
            gain.gain.setValueAtTime(0.06, now);
            gain.gain.exponentialRampToValueAtTime(0.001, now + 0.12);
            osc.start(now);
            osc.stop(now + 0.13);
        } else {
            osc.frequency.setValueAtTime(240.0, now);
            osc.frequency.setValueAtTime(185.0, now + 0.08);
            gain.gain.setValueAtTime(0.07, now);
            gain.gain.exponentialRampToValueAtTime(0.001, now + 0.2);
            osc.start(now);
            osc.stop(now + 0.21);
        }

        setTimeout(() => {
            ctx.close().catch(() => {});
        }, 300);
    } catch {
        // Ignore audio policy errors cleanly
    }
}

export function downsampleTo16k(input: Float32Array, srcRate: number): Int16Array {
    const targetRate = 16000;
    if (srcRate <= 0 || input.length === 0) return new Int16Array(0);
    if (srcRate === targetRate) {
        const out = new Int16Array(input.length);
        for (let i = 0; i < input.length; i++) {
            const s = Math.max(-1, Math.min(1, input[i]));
            out[i] = s < 0 ? s * 32768 : s * 32767;
        }
        return out;
    }

    const outLen = Math.max(1, Math.round((input.length * targetRate) / srcRate));
    const out = new Int16Array(outLen);
    const ratio = srcRate / targetRate;
    for (let i = 0; i < outLen; i++) {
        const pos = i * ratio;
        const idx = Math.min(Math.floor(pos), input.length - 1);
        const next = Math.min(idx + 1, input.length - 1);
        const frac = pos - idx;
        const sample = input[idx] + (input[next] - input[idx]) * frac;
        const clamped = Math.max(-1, Math.min(1, sample));
        out[i] = clamped < 0 ? clamped * 32768 : clamped * 32767;
    }
    return out;
}

export function int16ToBase64(samples: Int16Array): string {
    const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
    let binary = '';
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
        binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    return btoa(binary);
}

function concatInt16Chunks(chunks: Int16Array[]): Int16Array {
    let total = 0;
    for (const c of chunks) total += c.length;
    const out = new Int16Array(total);
    let offset = 0;
    for (const c of chunks) {
        out.set(c, offset);
        offset += c.length;
    }
    return out;
}

/**
 * Records a fixed duration sample at 16 kHz PCM16 mono for voice enrollment or live speaker testing.
 */
export async function recordFixedSamplePcm16(
    durationMs: number,
    onLevel?: (rms: number) => void,
): Promise<UtteranceResult> {
    const stream = await navigator.mediaDevices.getUserMedia({
        audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });
    const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    const ctx = new AudioCtx({ sampleRate: 16000 });
    const source = ctx.createMediaStreamSource(stream);
    const processor = ctx.createScriptProcessor(2048, 1, 1);
    const silentGain = ctx.createGain();
    silentGain.gain.value = 0;

    const chunks: Int16Array[] = [];
    let maxRms = 0;

    processor.onaudioprocess = (e) => {
        const data = e.inputBuffer.getChannelData(0);
        let sq = 0;
        for (let i = 0; i < data.length; i++) sq += data[i] * data[i];
        const rms = Math.sqrt(sq / Math.max(1, data.length));
        if (rms > maxRms) maxRms = rms;
        onLevel?.(rms);
        chunks.push(downsampleTo16k(data, ctx.sampleRate));
    };

    source.connect(processor);
    processor.connect(silentGain);
    silentGain.connect(ctx.destination);

    await new Promise<void>((resolve) => setTimeout(resolve, durationMs));

    try {
        processor.disconnect();
        source.disconnect();
        silentGain.disconnect();
        stream.getTracks().forEach((t) => t.stop());
        await ctx.close();
    } catch {
        // ignore cleanup
    }

    const merged = concatInt16Chunks(chunks);
    return {
        pcmB64: int16ToBase64(merged),
        sampleRate: 16000,
        durationMs: Math.round((merged.length / 16000) * 1000),
        hadSpeech: maxRms > 0.008,
    };
}

/**
 * Breath-aware single utterance capture:
 * - Buffers 500ms pre-roll before voice onset
 * - Waits up to initialWaitMs (default 10s) for user to start speaking
 * - Allows natural breathing pauses up to silenceHangoverMs (default 2400ms)
 */
export function startBreathAwareUtteranceCapture(opts: {
    silenceHangoverMs?: number;
    initialWaitMs?: number;
    maxUtteranceMs?: number;
    onSpeechStart?: () => void;
    onLevel?: (rms: number) => void;
    onComplete: (result: UtteranceResult) => void;
    onError: (err: Error) => void;
}): CaptureHandle {
    const silenceHangoverMs = opts.silenceHangoverMs ?? 2400;
    const initialWaitMs = opts.initialWaitMs ?? 10000;
    const maxUtteranceMs = opts.maxUtteranceMs ?? 45000;

    let stopped = false;
    let stream: MediaStream | null = null;
    let ctx: AudioContext | null = null;
    let processor: ScriptProcessorNode | null = null;
    let source: MediaStreamAudioSourceNode | null = null;
    let silentGain: GainNode | null = null;

    let hasSpoken = false;
    let activeSpeechChunks: Int16Array[] = [];
    const preRollChunks: Int16Array[] = [];
    const MAX_PREROLL_SAMPLES = 8000; // 500ms @ 16kHz
    let preRollSamples = 0;

    let noiseEma = 0.006;
    let consecutiveSpeech = 0;
    let speechStartAt = 0;
    let lastSpeechAt = 0;
    const captureStartedAt = Date.now();

    const cleanup = () => {
        try {
            processor?.disconnect();
            source?.disconnect();
            silentGain?.disconnect();
            stream?.getTracks().forEach((t) => t.stop());
            ctx?.close().catch(() => {});
        } catch {
            // ignore
        }
    };

    const finish = (cancelled: boolean) => {
        if (stopped) return;
        stopped = true;
        cleanup();
        if (cancelled) return;
        const merged = concatInt16Chunks(activeSpeechChunks);
        opts.onComplete({
            pcmB64: int16ToBase64(merged),
            sampleRate: 16000,
            durationMs: Math.round((merged.length / 16000) * 1000),
            hadSpeech: hasSpoken && merged.length >= 3200,
        });
    };

    navigator.mediaDevices
        .getUserMedia({
            audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        })
        .then((mediaStream) => {
            if (stopped) {
                mediaStream.getTracks().forEach((t) => t.stop());
                return;
            }
            stream = mediaStream;
            const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
            ctx = new AudioCtx({ sampleRate: 16000 });
            source = ctx.createMediaStreamSource(mediaStream);
            processor = ctx.createScriptProcessor(2048, 1, 1);
            silentGain = ctx.createGain();
            silentGain.gain.value = 0;

            processor.onaudioprocess = (e) => {
                if (stopped) return;
                const data = e.inputBuffer.getChannelData(0);
                let sq = 0;
                for (let i = 0; i < data.length; i++) sq += data[i] * data[i];
                const rms = Math.sqrt(sq / Math.max(1, data.length));
                opts.onLevel?.(rms);

                const pcm16 = downsampleTo16k(data, ctx ? ctx.sampleRate : 16000);
                const now = Date.now();
                const threshold = Math.max(0.012, noiseEma * 2.4);

                if (!hasSpoken) {
                    preRollChunks.push(pcm16);
                    preRollSamples += pcm16.length;
                    while (preRollSamples > MAX_PREROLL_SAMPLES && preRollChunks.length > 1) {
                        const dropped = preRollChunks.shift();
                        if (dropped) preRollSamples -= dropped.length;
                    }

                    if (rms > threshold) {
                        consecutiveSpeech++;
                        if (consecutiveSpeech >= 2) {
                            hasSpoken = true;
                            speechStartAt = now;
                            lastSpeechAt = now;
                            activeSpeechChunks = [...preRollChunks];
                            opts.onSpeechStart?.();
                        }
                    } else {
                        consecutiveSpeech = 0;
                        noiseEma = noiseEma * 0.94 + rms * 0.06;
                        if (now - captureStartedAt > initialWaitMs) {
                            finish(false);
                        }
                    }
                } else {
                    activeSpeechChunks.push(pcm16);
                    if (rms > threshold * 0.85) {
                        lastSpeechAt = now;
                    }
                    if (now - lastSpeechAt >= silenceHangoverMs || now - speechStartAt >= maxUtteranceMs) {
                        finish(false);
                    }
                }
            };

            source.connect(processor);
            processor.connect(silentGain);
            silentGain.connect(ctx.destination);
        })
        .catch((err) => {
            stopped = true;
            cleanup();
            opts.onError(err instanceof Error ? err : new Error(String(err)));
        });

    return {
        stopNow: () => finish(false),
        cancel: () => finish(true),
    };
}

/**
 * Continuous zero-token local VAD wake monitor: captures spoken segments whenever the mic
 * hears human speech, leaving silence 100% local, and pauses itself while assistant is speaking/thinking.
 */
export function startContinuousWakeMonitor(opts: {
    getHangoverMs: () => number;
    isPaused: () => boolean;
    onSegmentReady: (result: UtteranceResult) => void;
    onError: (err: Error) => void;
}): CaptureHandle {
    let stopped = false;
    let stream: MediaStream | null = null;
    let ctx: AudioContext | null = null;
    let processor: ScriptProcessorNode | null = null;
    let source: MediaStreamAudioSourceNode | null = null;
    let silentGain: GainNode | null = null;

    let inSpeech = false;
    let speechChunks: Int16Array[] = [];
    const preRollChunks: Int16Array[] = [];
    const MAX_PREROLL_SAMPLES = 8000;
    let preRollSamples = 0;
    let noiseEma = 0.007;
    let consecutiveSpeech = 0;
    let speechStartAt = 0;
    let lastSpeechAt = 0;

    const cleanup = () => {
        try {
            processor?.disconnect();
            source?.disconnect();
            silentGain?.disconnect();
            stream?.getTracks().forEach((t) => t.stop());
            ctx?.close().catch(() => {});
        } catch {
            // ignore
        }
    };

    const resetSpeech = () => {
        inSpeech = false;
        speechChunks = [];
        consecutiveSpeech = 0;
    };

    navigator.mediaDevices
        .getUserMedia({
            audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        })
        .then((mediaStream) => {
            if (stopped) {
                mediaStream.getTracks().forEach((t) => t.stop());
                return;
            }
            stream = mediaStream;
            const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
            ctx = new AudioCtx({ sampleRate: 16000 });
            source = ctx.createMediaStreamSource(mediaStream);
            processor = ctx.createScriptProcessor(2048, 1, 1);
            silentGain = ctx.createGain();
            silentGain.gain.value = 0;

            processor.onaudioprocess = (e) => {
                if (stopped) return;
                if (opts.isPaused()) {
                    if (inSpeech) resetSpeech();
                    return;
                }

                const data = e.inputBuffer.getChannelData(0);
                let sq = 0;
                for (let i = 0; i < data.length; i++) sq += data[i] * data[i];
                const rms = Math.sqrt(sq / Math.max(1, data.length));
                const pcm16 = downsampleTo16k(data, ctx ? ctx.sampleRate : 16000);
                const now = Date.now();
                const threshold = Math.max(0.013, noiseEma * 2.5);

                if (!inSpeech) {
                    preRollChunks.push(pcm16);
                    preRollSamples += pcm16.length;
                    while (preRollSamples > MAX_PREROLL_SAMPLES && preRollChunks.length > 1) {
                        const dropped = preRollChunks.shift();
                        if (dropped) preRollSamples -= dropped.length;
                    }

                    if (rms > threshold) {
                        consecutiveSpeech++;
                        if (consecutiveSpeech >= 2) {
                            inSpeech = true;
                            speechStartAt = now;
                            lastSpeechAt = now;
                            speechChunks = [...preRollChunks];
                        }
                    } else {
                        consecutiveSpeech = 0;
                        noiseEma = noiseEma * 0.95 + rms * 0.05;
                    }
                } else {
                    speechChunks.push(pcm16);
                    if (rms > threshold * 0.85) {
                        lastSpeechAt = now;
                    }
                    const hangoverMs = opts.getHangoverMs();
                    if (now - lastSpeechAt >= hangoverMs || now - speechStartAt >= 30000) {
                        const merged = concatInt16Chunks(speechChunks);
                        const durationMs = Math.round((merged.length / 16000) * 1000);
                        resetSpeech();
                        if (durationMs >= 450) {
                            opts.onSegmentReady({
                                pcmB64: int16ToBase64(merged),
                                sampleRate: 16000,
                                durationMs,
                                hadSpeech: true,
                            });
                        }
                    }
                }
            };

            source.connect(processor);
            processor.connect(silentGain);
            silentGain.connect(ctx.destination);
        })
        .catch((err) => {
            stopped = true;
            cleanup();
            opts.onError(err instanceof Error ? err : new Error(String(err)));
        });

    return {
        stopNow: () => {
            stopped = true;
            cleanup();
        },
        cancel: () => {
            stopped = true;
            cleanup();
        },
    };
}
