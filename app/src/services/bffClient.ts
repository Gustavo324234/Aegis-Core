import * as Crypto from 'expo-crypto';
import { ENDPOINTS } from '@/constants/endpoints';
import type { LoginResponse } from '@/types/auth';

function buildUrl(serverUrl: string, path: string): string {
  return `${serverUrl.replace(/\/$/, '')}${path}`;
}

function buildWsUrl(serverUrl: string, path: string): string {
  const base = serverUrl.replace(/\/$/, '');
  if (base.startsWith('https://')) {
    return base.replace('https://', 'wss://') + path;
  }
  if (base.startsWith('http://')) {
    return base.replace('http://', 'ws://') + path;
  }
  return `ws://${base}${path}`;
}

export async function hashPassword(password: string): Promise<string> {
  return Crypto.digestStringAsync(
    Crypto.CryptoDigestAlgorithm.SHA256,
    password,
    { encoding: Crypto.CryptoEncoding.HEX }
  );
}

export async function login(
  serverUrl: string,
  email: string,
  password: string
): Promise<LoginResponse> {
  const passwordHash = await hashPassword(password);
  const response = await fetch(buildUrl(serverUrl, ENDPOINTS.LOGIN), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ tenant_id: email, session_key: passwordHash }),
  });
  
  if (!response.ok) {
    if (response.status === 401) throw new Error('AUTH_FAILURE');
    throw new Error(`HTTP ${response.status}`);
  }
  
  // The kernel returns {"message": "...", "status": "authenticated"}
  // We normalize it to what the AuthStore expects
  return {
    session_key: passwordHash,
    tenant_id: email,
  };
}

export async function getStatus(
  serverUrl: string,
  sessionKey: string
): Promise<boolean> {
  try {
    const response = await fetch(buildUrl(serverUrl, ENDPOINTS.STATUS), {
      headers: { 'x-citadel-key': sessionKey },
    });
    return response.ok;
  } catch {
    return false;
  }
}

// --- WebSocket Chat (Aegis-Core ank-server protocol) ---

export interface ChatCallbacks {
  onToken: (token: string) => void;
  onDone: () => void;
  onError: (message: string) => void;
}

export function connectChat(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  callbacks: ChatCallbacks
): WebSocket {
  const url = buildWsUrl(serverUrl, ENDPOINTS.CHAT_WS(tenantId));
  
  // sessionKey as subprotocol with session-key. prefix per Aegis kernel spec
  const ws = new WebSocket(url, [`session-key.${sessionKey}`]);

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data as string);
      const { event: type, data } = msg;

      switch (type) {
        case 'syslog':
          console.log('[Kernel Syslog]', data);
          break;
        case 'kernel_event':
          if (data.output) {
            callbacks.onToken(data.output);
          } else if (data.thought) {
            // Optional: show thoughts in UI? For now just log
            console.log('[Thought]', data.thought);
          } else if (data.status_update?.state === 'STATE_COMPLETED') {
            callbacks.onDone();
          } else if (data.error) {
            callbacks.onError(data.error);
          }
          break;
        case 'error':
          callbacks.onError(data || 'Unknown server error');
          break;
      }
    } catch (e) {
      console.error('WS Parse Error', e);
      callbacks.onError('Failed to parse server message');
    }
  };

  ws.onerror = () => callbacks.onError('WebSocket connection error');
  ws.onclose = (event) => {
    if (event.code !== 1000) {
      callbacks.onError(`Connection closed unexpectedly (${event.code})`);
    }
  };

  return ws;
}

export function sendChatMessage(
  ws: WebSocket,
  message: string,
  taskType: string = 'chat'
): void {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      prompt: message,
      action: 'submit',
      task_type: taskType
    }));
  }
}

export async function uploadFile(
  serverUrl: string,
  formData: FormData
): Promise<any> {
  const response = await fetch(buildUrl(serverUrl, ENDPOINTS.UPLOAD), {
    method: 'POST',
    body: formData,
  });
  return response.json();
}

export async function fetchPersona(
  serverUrl: string,
  tenantId: string,
  sessionKey: string
): Promise<{ persona: string; is_configured: boolean }> {
  const res = await fetch(buildUrl(serverUrl, '/api/persona'), {
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
  if (!res.ok) return { persona: '', is_configured: false };
  return res.json();
}

export { buildUrl, buildWsUrl };
