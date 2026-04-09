import * as SecureStore from 'expo-secure-store';

const KEYS = {
  SESSION: 'session_key',
  SERVER_URL: 'server_url',
  TENANT_ID: 'tenant_id',
  MODE: 'active_mode',
  ACTIVE_PROVIDER: 'active_provider',
  API_KEY_PREFIX: 'api_key_',
  MODEL_PREFIX: 'model_',
} as const;

export async function setItem(key: string, value: string): Promise<void> {
  await SecureStore.setItemAsync(key, value);
}

export async function getItem(key: string): Promise<string | null> {
  return SecureStore.getItemAsync(key);
}

// Session — Satellite Mode
export async function saveSessionKey(key: string): Promise<void> {
  await SecureStore.setItemAsync(KEYS.SESSION, key);
}
export async function getSessionKey(): Promise<string | null> {
  return SecureStore.getItemAsync(KEYS.SESSION);
}
export async function clearSession(): Promise<void> {
  await SecureStore.deleteItemAsync(KEYS.SESSION);
}

// Server config — Satellite Mode
export async function saveServerUrl(url: string): Promise<void> {
  await SecureStore.setItemAsync(KEYS.SERVER_URL, url);
}
export async function getServerUrl(): Promise<string | null> {
  return SecureStore.getItemAsync(KEYS.SERVER_URL);
}
export async function saveTenantId(id: string): Promise<void> {
  await SecureStore.setItemAsync(KEYS.TENANT_ID, id);
}
export async function getTenantId(): Promise<string | null> {
  return SecureStore.getItemAsync(KEYS.TENANT_ID);
}

// App mode
export async function saveActiveMode(mode: 'satellite' | 'cloud' | 'hybrid'): Promise<void> {
  await SecureStore.setItemAsync(KEYS.MODE, mode);
}
export async function getActiveMode(): Promise<'satellite' | 'cloud' | 'hybrid'> {
  const val = await SecureStore.getItemAsync(KEYS.MODE);
  if (val === 'cloud' || val === 'hybrid') return val as 'cloud' | 'hybrid';
  return 'satellite';
}

// Active Provider selection
export async function saveActiveProvider(providerId: string): Promise<void> {
  await SecureStore.setItemAsync(KEYS.ACTIVE_PROVIDER, providerId);
}
export async function getActiveProvider(): Promise<string | null> {
  return SecureStore.getItemAsync(KEYS.ACTIVE_PROVIDER);
}

// API keys — Cloud Mode
export async function saveApiKey(providerId: string, key: string): Promise<void> {
  await SecureStore.setItemAsync(`${KEYS.API_KEY_PREFIX}${providerId}`, key);
}
export async function getApiKey(providerId: string): Promise<string | null> {
  return SecureStore.getItemAsync(`${KEYS.API_KEY_PREFIX}${providerId}`);
}
export async function removeApiKey(providerId: string): Promise<void> {
  await SecureStore.deleteItemAsync(`${KEYS.API_KEY_PREFIX}${providerId}`);
}

// Model selection — Cloud Mode
export async function saveSelectedModel(providerId: string, model: string): Promise<void> {
  await SecureStore.setItemAsync(`${KEYS.MODEL_PREFIX}${providerId}`, model);
}
export async function getSelectedModel(providerId: string, defaultModel: string): Promise<string> {
  const saved = await SecureStore.getItemAsync(`${KEYS.MODEL_PREFIX}${providerId}`);
  return saved ?? defaultModel;
}

// Nuclear option
export async function clearAll(): Promise<void> {
  await Promise.all([
    SecureStore.deleteItemAsync(KEYS.SESSION),
    SecureStore.deleteItemAsync(KEYS.SERVER_URL),
    SecureStore.deleteItemAsync(KEYS.TENANT_ID),
    SecureStore.deleteItemAsync(KEYS.MODE),
    SecureStore.deleteItemAsync(KEYS.ACTIVE_PROVIDER),
  ]);
}
