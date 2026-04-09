export const ENDPOINTS = {
  LOGIN: '/api/auth/login',
  STATUS: '/api/status',
  CHAT_WS: (tenantId: string) => `/ws/chat/${tenantId}`,
  SIREN_WS: (tenantId: string) => `/ws/siren/${tenantId}`,
  UPLOAD: '/api/workspace/upload',
  ENGINE_CONFIGURE: '/api/engine/configure',
} as const;
