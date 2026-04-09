import { create } from 'zustand';
import type { AppMode } from '@/types/auth';
import * as secureStorage from '@/services/secureStorage';

interface AuthState {
  // Runtime state
  isAuthenticated: boolean;
  sessionKey: string | null;
  tenantId: string | null;
  serverUrl: string | null;
  mode: AppMode;

  // Actions
  initFromStorage: () => Promise<void>;
  loginSuccess: (params: {
    sessionKey: string;
    tenantId: string;
    serverUrl: string;
  }) => Promise<void>;
  setMode: (mode: AppMode) => Promise<void>;
  logout: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  isAuthenticated: false,
  sessionKey: null,
  tenantId: null,
  serverUrl: null,
  mode: 'satellite',

  initFromStorage: async () => {
    const [sessionKey, tenantId, serverUrl, mode] = await Promise.all([
      secureStorage.getSessionKey(),
      secureStorage.getTenantId(),
      secureStorage.getServerUrl(),
      secureStorage.getActiveMode(),
    ]);

    const activeMode = (mode as AppMode) || 'satellite';

    set({
      isAuthenticated: !!sessionKey || activeMode === 'cloud',
      sessionKey,
      tenantId,
      serverUrl,
      mode: activeMode,
    });
  },

  loginSuccess: async ({ sessionKey, tenantId, serverUrl }) => {
    await Promise.all([
      secureStorage.saveSessionKey(sessionKey),
      secureStorage.saveTenantId(tenantId),
      secureStorage.saveServerUrl(serverUrl),
      secureStorage.saveActiveMode('satellite'),
    ]);
    set({
      isAuthenticated: true,
      sessionKey,
      tenantId,
      serverUrl,
      mode: 'satellite'
    });
  },

  setMode: async (mode) => {
    await secureStorage.saveActiveMode(mode);
    const sessionKey = await secureStorage.getSessionKey();
    set({
      mode,
      isAuthenticated: mode === 'cloud' || !!sessionKey
    });
  },

  logout: async () => {
    await secureStorage.clearSession();
    set({
      isAuthenticated: false,
      sessionKey: null,
      tenantId: null,
      mode: 'satellite',
    });
  },
}));
