export type AppMode = 'satellite' | 'cloud' | 'hybrid';

export interface LoginResponse {
  session_key: string;
  tenant_id: string;
}

export interface AuthCredentials {
  serverUrl: string;
  email: string;
  password: string;
}
