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
            // Use a local variable so TypeScript can track non-null after construction
            const ctx = new AudioCtx({ sampleRate: 22050 });
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

        // 1. Decode Base64 to binary string
        const binaryString = window.atob(base64PCM);
        const len = binaryString.length;

        // 2. Base64 represents bytes. We need Uint8Array to hold them.
        const bytes = new Uint8Array(len);
        for (let i = 0; i < len; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }

        // 3. Convert Uint8Array to Int16Array (assuming Little Endian)
        const int16Array = new Int16Array(bytes.buffer);

        // 4. Convert Int16Array to Float32Array (Web Audio API native format)
        const float32Array = new Float32Array(int16Array.length);
        for (let i = 0; i < int16Array.length; i++) {
            // SRE Optimization: Direct math conversion
            float32Array[i] = int16Array[i] / 32768.0;
        }

        // 5. Create AudioBuffer (1 channel, Mono)
        const audioBuffer = this.audioContext.createBuffer(1, float32Array.length, sampleRate);
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
}

// Singleton instance
export const ttsPlayer = new TTSPlayer();
