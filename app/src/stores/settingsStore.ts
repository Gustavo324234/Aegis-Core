import { create } from 'zustand';
import * as secureStorage from '@/services/secureStorage';
import { PROVIDERS } from '@/constants/providers';
import { Language } from '@/constants/i18n';

interface SettingsState {
  selectedProviderId: string;
  selectedModel: string;
  language: Language;
  apiKeys: Record<string, boolean>;
  
  loadSettings: () => Promise<void>;
  setProvider: (providerId: string) => Promise<void>;
  setModel: (model: string) => Promise<void>;
  setLanguage: (lang: Language) => Promise<void>;
  refreshApiKeys: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  selectedProviderId: 'openai',
  selectedModel: 'gpt-4o-mini',
  language: 'es',
  apiKeys: {},

  loadSettings: async () => {
    const providerId = await secureStorage.getActiveProvider() || 'openai';
    const provider = PROVIDERS.find((p) => p.id === providerId) || PROVIDERS[0];
    const model = await secureStorage.getSelectedModel(provider.id, provider.defaultModel);
    const lang = await secureStorage.getItem('app_lang') as Language || 'es';

    await get().refreshApiKeys();
    set({ selectedProviderId: provider.id, selectedModel: model, language: lang });
  },

  setProvider: async (providerId) => {
    const provider = PROVIDERS.find((p) => p.id === providerId) || PROVIDERS[0];
    await secureStorage.saveActiveProvider(providerId);
    const model = await secureStorage.getSelectedModel(providerId, provider.defaultModel);
    set({ selectedProviderId: providerId, selectedModel: model });
  },

  setModel: async (model) => {
    const { selectedProviderId } = get();
    await secureStorage.saveSelectedModel(selectedProviderId, model);
    set({ selectedModel: model });
  },

  setLanguage: async (lang) => {
    await secureStorage.setItem('app_lang', lang);
    set({ language: lang });
  },

  refreshApiKeys: async () => {
    const keys: Record<string, boolean> = {};
    for (const p of PROVIDERS) {
      const key = await secureStorage.getApiKey(p.id);
      keys[p.id] = !!key;
    }
    set({ apiKeys: keys });
  }
}));
