/**
 * TTSPlayer.ts
 * Implementación de Gapless Playback API para Aegis OS
 * Convierte Base64 PCM Int16 a Float32Array para The Orb.
 */

interface WindowWithWebkit extends Window {
    AudioContext?: typeof AudioContext;
    webkitAudioContext?: typeof AudioContext;
}

export class TTSPlayer {
    private audioContext: AudioContext | null = null;
    private nextStartTime: number = 0;

    public async initialize() {
        if (!this.audioContext) {
            const win = window as WindowWithWebkit;
            const AudioCtx = win.AudioContext ?? win.webkitAudioContext;
            if (!AudioCtx) throw new Error('Web Audio API not supported');
            // CORE-369: Use native hardware sampleRate so createBuffer(..., sampleRate)
            // resamples cleanly for both 16 kHz (Voxtral) and 22.05 kHz (ElevenLabs/Espeak).
            const ctx = new AudioCtx();
            this.audioContext = ctx;

            if (ctx.state === 'suspended') {
                await ctx.resume();
            }

            // Reset timing on initialization to current time
            this.nextStartTime = ctx.currentTime;
            return;
        }

        const ctx = this.audioContext;
        if (ctx.state === 'suspended') {
            await ctx.resume();
        }

        // Reset timing on initialization to current time
        this.nextStartTime = ctx.currentTime;
    }

    public playChunk(base64PCM: string, sampleRate: number = 22050) {
        if (!this.audioContext) {
            console.warn("TTSPlayer: AudioContext not initialized. Call initialize() first.");
            return;
        }
        if (!base64PCM) return;

        // 1. Decode Base64 to binary string
        const binaryString = window.atob(base64PCM);
        const len = binaryString.length;
        // CORE-369: Guard odd-byte PCM payloads so Int16Array never throws RangeError
        const sampleCount = Math.floor(len / 2);
        if (sampleCount === 0) return;

        // 2. Base64 represents bytes. We need Uint8Array to hold them.
        const byteLen = sampleCount * 2;
        const bytes = new Uint8Array(byteLen);
        for (let i = 0; i < byteLen; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }

        // 3. Convert Uint8Array to Int16Array (assuming Little Endian)
        const int16Array = new Int16Array(bytes.buffer, 0, sampleCount);

        // 4. Convert Int16Array to Float32Array (Web Audio API native format)
        const float32Array = new Float32Array(int16Array.length);
        for (let i = 0; i < int16Array.length; i++) {
            // SRE Optimization: Direct math conversion
            float32Array[i] = int16Array[i] / 32768.0;
        }

        // 5. Create AudioBuffer (1 channel, Mono)
        const safeRate = sampleRate >= 8000 && sampleRate <= 96000 ? sampleRate : 22050;
        const audioBuffer = this.audioContext.createBuffer(1, float32Array.length, safeRate);
        audioBuffer.copyToChannel(float32Array, 0);

        // 6. Jitter Protection Resync
        const currentTime = this.audioContext.currentTime;
        if (this.nextStartTime < currentTime) {
            // We fell behind (network jitter or lag). Resync to current time + 50ms buffer
            this.nextStartTime = currentTime + 0.05;
        }

        // 7. Schedule gapless playback
        const source = this.audioContext.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(this.audioContext.destination);
        source.start(this.nextStartTime);

        // 8. Move pointer mathematically
        this.nextStartTime += audioBuffer.duration;

        // 9. Memory Management: Clean up node once finished
        source.onended = () => {
            source.disconnect();
        };
    }

    public getNextStartTime(): number {
        return this.nextStartTime;
    }

    public getAudioContext(): AudioContext | null {
        return this.audioContext;
    }
}

// Singleton instance
export const ttsPlayer = new TTSPlayer();
