import * as Speech from 'expo-speech';
import { Audio } from 'expo-av';
import * as bffClient from '@/services/bffClient';

export interface VoiceCallbacks {
  onSpeechStart?: () => void;
  onSpeechEnd?: () => void;
  onResult?: (text: string) => void;
  onError?: (error: string) => void;
}

// ── TTS (Text-to-Speech) ──────────────────────────────────────────────

function detectLanguage(text: string): string {
  const spanishChars = /[áéíóúñÁÉÍÓÚÑ]/;
  const commonSpanishWords = /\b(el|la|los|las|un|una|en|es|y|o|pero|por|para|si|no)\b/i;

  if (spanishChars.test(text) || commonSpanishWords.test(text)) {
    return 'es-ES';
  }
  return 'en-US';
}

export async function speak(
  text: string,
  forceLanguage?: string,
  onDone?: () => void
): Promise<void> {
  const isSpeaking = await Speech.isSpeakingAsync();
  if (isSpeaking) {
    await Speech.stop();
  }

  const language = forceLanguage || detectLanguage(text);

  console.log(`[VoiceService] Speaking in ${language}`);

  Speech.speak(text, {
    language,
    pitch: 1.0,
    rate: 1.1,
    onDone: () => {
      console.log('[VoiceService] Speech finished');
      if (onDone) onDone();
    },
    onError: (err) => console.error('[VoiceService] Speech error:', err)
  });
}

export async function stopSpeaking(): Promise<void> {
  await Speech.stop();
}

// ── STT (Speech-to-Text) ──────────────────────────────────────────────

export async function requestPermissions(): Promise<boolean> {
  const { status } = await Audio.requestPermissionsAsync();
  return status === 'granted';
}

// ── Siren Voice Bridge ────────────────────────────────────────────────

export function connectSiren(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  onAudioData: (data: ArrayBuffer) => void,
  onSirenEvent: (event: any) => void,
  onError: (msg: string) => void
): WebSocket {
  const url = bffClient.buildWsUrl(serverUrl, `/ws/siren/${tenantId}`);
  const ws = new WebSocket(url, [sessionKey]);
  
  // Siren Bridge (Marzo 2026): El BFF envía JSON con los SirenEvents del Kernel
  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.event === 'siren_event') {
        const sirenPayload = msg.data;
        
        // Propagar evento para feedback visual (Engine ID, VAD, etc.)
        onSirenEvent(sirenPayload);

        // Si hay audio, decodificar de Base64 (Standard de MessageToDict)
        if (sirenPayload.tts_audio_chunk) {
          const binaryString = atob(sirenPayload.tts_audio_chunk);
          const bytes = new Uint8Array(binaryString.length);
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
          }
          onAudioData(bytes.buffer);
        }
      }
    } catch (e) {
      console.warn('[Siren] Failed to parse non-binary message:', e);
    }
  };

  ws.onerror = () => onError('Siren WebSocket error');
  ws.onclose = (e) => {
    if (e.code !== 1000) onError(`Siren closed: ${e.code}`);
  };

  return ws;
}

export function sendSirenAudio(ws: WebSocket, audioData: Int16Array): void {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(audioData.buffer);
  }
}
