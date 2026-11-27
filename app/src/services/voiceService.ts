import * as Speech from 'expo-speech';
import { Audio } from 'expo-av';
import * as FileSystem from 'expo-file-system';
import * as bffClient from '@/services/bffClient';
import {
  hasNativeAssistantModule,
  recordNativePcm16Utterance,
} from '@/services/aegisAssistantNative';

export interface VoiceCallbacks {
  onSpeechStart?: () => void;
  onSpeechEnd?: () => void;
  onResult?: (text: string) => void;
  onError?: (error: string) => void;
}

export interface UtteranceApiResult {
  accepted: boolean;
  reason?: string;
  command: string;
  transcript: string;
  matched_speaker?: string | null;
  score?: number;
  wake_only?: boolean;
  agent_name?: string;
}

export interface EnrolledVoiceItem {
  id: string;
  label: string;
  sample_count: number;
  threshold: number;
}

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

  Speech.speak(text, {
    language,
    pitch: 1.0,
    rate: 1.08,
    onDone: () => {
      if (onDone) onDone();
    },
    onError: (err) => console.error('[VoiceService] Speech error:', err),
  });
}

export async function stopSpeaking(): Promise<void> {
  await Speech.stop();
}

export async function requestPermissions(): Promise<boolean> {
  const { status } = await Audio.requestPermissionsAsync();
  return status === 'granted';
}

/**
 * Records a breath-tolerant utterance using native 16kHz PCM (`AudioRecord` via Kotlin module)
 * when built in Android Studio, or Expo-AV metering fallback in Expo Go, then posts to
 * `/api/siren/utterance` for speaker verification + Groq Whisper transcription.
 */
export async function captureAndProcessUtterance(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  options?: {
    hangoverMs?: number;
    mode?: 'direct' | 'wakeword' | 'followup';
    lang?: string;
  }
): Promise<UtteranceApiResult> {
  const hangoverMs = options?.hangoverMs ?? 2400;
  const mode = options?.mode ?? 'direct';
  const lang = options?.lang ?? 'es-AR';
  const baseUrl = serverUrl.replace(/\/+$/, '');

  let bodyPayload: Record<string, unknown>;

  if (hasNativeAssistantModule()) {
    const nativeCap = await recordNativePcm16Utterance(hangoverMs, 0);
    if (!nativeCap || !nativeCap.hadSpeech || !nativeCap.pcmB64) {
      return { accepted: false, reason: 'empty_speech', command: '', transcript: '' };
    }
    bodyPayload = {
      pcm_b64: nativeCap.pcmB64,
      sample_rate: nativeCap.sampleRate || 16000,
      mode,
      lang,
    };
  } else {
    // Fallback for Expo Go / iOS using Expo AV metering with 2.4s breath pause tolerance
    await Audio.setAudioModeAsync({
      allowsRecordingIOS: true,
      playsInSilentModeIOS: true,
    });
    const recording = new Audio.Recording();
    await recording.prepareToRecordAsync({
      ...Audio.RecordingOptionsPresets.HIGH_QUALITY,
      isMeteringEnabled: true,
    });
    await recording.startAsync();

    const startedAt = Date.now();
    let hasSpoken = false;
    let lastSpeechAt = startedAt;

    while (true) {
      await new Promise((r) => setTimeout(r, 150));
      const status = await recording.getStatusAsync();
      if (!status.isRecording) break;
      const now = Date.now();
      const db = typeof status.metering === 'number' ? status.metering : -60;
      if (db > -36) {
        hasSpoken = true;
        lastSpeechAt = now;
      }
      if (!hasSpoken && now - startedAt >= 9000) break;
      if (hasSpoken && now - lastSpeechAt >= hangoverMs) break;
      if (now - startedAt >= 35000) break;
    }

    await recording.stopAndUnloadAsync();
    const uri = recording.getURI();
    if (!hasSpoken || !uri) {
      return { accepted: false, reason: 'empty_speech', command: '', transcript: '' };
    }
    const audioB64 = await FileSystem.readAsStringAsync(uri, {
      encoding: 'base64',
    });
    bodyPayload = {
      audio_b64: audioB64,
      mime_type: 'audio/m4a',
      mode,
      lang,
    };
  }

  const resp = await fetch(`${baseUrl}/api/siren/utterance`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
    body: JSON.stringify(bodyPayload),
  });

  if (!resp.ok) {
    const errBody = await resp.text();
    throw new Error(errBody || `HTTP ${resp.status}`);
  }
  return (await resp.json()) as UtteranceApiResult;
}

export async function fetchEnrolledVoices(
  serverUrl: string,
  tenantId: string,
  sessionKey: string
): Promise<{ profiles: EnrolledVoiceItem[]; gateEnabled: boolean; agentName: string }> {
  const baseUrl = serverUrl.replace(/\/+$/, '');
  const resp = await fetch(`${baseUrl}/api/siren/voices/enrolled`, {
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
  if (!resp.ok) {
    throw new Error(`HTTP ${resp.status}`);
  }
  const data = await resp.json();
  return {
    profiles: data.profiles || [],
    gateEnabled: data.gate_enabled !== false,
    agentName: data.agent_name || 'Aegis',
  };
}

export async function enrollVoiceSampleBatch(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  label: string
): Promise<boolean> {
  if (!hasNativeAssistantModule()) {
    throw new Error(
      'El registro biométrico en 16kHz PCM requiere la APK compilada en Android Studio (`npx expo prebuild --platform android`).'
    );
  }
  const samples: string[] = [];
  for (let i = 0; i < 3; i++) {
    const sample = await recordNativePcm16Utterance(0, 3000);
    if (!sample?.pcmB64) {
      throw new Error('No se capturó audio suficiente en la muestra.');
    }
    samples.push(sample.pcmB64);
  }

  const baseUrl = serverUrl.replace(/\/+$/, '');
  const resp = await fetch(`${baseUrl}/api/siren/voices/enroll`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
    body: JSON.stringify({
      label: label.trim() || 'Mi voz (Celular)',
      samples_pcm_b64: samples,
      sample_rate: 16000,
    }),
  });
  return resp.ok;
}

export async function deleteEnrolledVoice(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  id: string
): Promise<boolean> {
  const baseUrl = serverUrl.replace(/\/+$/, '');
  const resp = await fetch(`${baseUrl}/api/siren/voices/enrolled/${encodeURIComponent(id)}`, {
    method: 'DELETE',
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
  return resp.ok;
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
  const ws = new WebSocket(url, [`session-key.${sessionKey}`]);

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.event === 'siren_event') {
        const sirenPayload = msg.data;
        onSirenEvent(sirenPayload);

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
