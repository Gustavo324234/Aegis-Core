# CORE-143 — Feature: OAuth en App Mobile — Google y Spotify via expo-auth-session

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `app/`
**Tipo:** feat
**Prioridad:** Alta — prerrequisito de CORE-138
**Asignado a:** Shell Engineer (Antigravity)
**Depende de:** CORE-138 (endpoint receptor de tokens en el servidor)

---

## Arquitectura — App como único OAuth client

```
App mobile
    │  Client IDs compilados en el binario (app.json / constantes)
    │  expo-auth-session maneja el flujo OAuth completo
    ▼
Google / Spotify  →  access_token + refresh_token  →  App
    │
    ▼
App → POST /api/oauth/tokens → Servidor Aegis
    │  { provider, access_token, refresh_token, expires_in, scope }
    │  headers: x-citadel-tenant + x-citadel-key
    ▼
Servidor guarda tokens en TenantDB (SQLCipher)
```

**El servidor nunca habla con Google/Spotify para OAuth.**
**Solo recibe y almacena tokens. Sin Device Flow. Sin redirect URI en el servidor.**

---

## Ventajas de este modelo

- El usuario hace "Conectar con Google" → el browser nativo de su teléfono abre Google → acepta → vuelve a la app. Flujo de 3 segundos.
- `expo-auth-session` maneja los deep links automáticamente con Expo Go y builds nativas.
- Los Client IDs están en `app.json` / constantes TypeScript — visibles en el repo open source, lo cual es correcto y esperado para apps que usan OAuth.
- El servidor no necesita registrar ninguna app en Google ni Spotify. Sin Client IDs en el servidor.

---

## Cambios requeridos

### 1. Dependencias en `app/package.json`

`expo-auth-session` ya es parte de Expo SDK 52 — verificar que está instalado.
Si no:
```bash
npx expo install expo-auth-session expo-web-browser
```

### 2. Constantes OAuth en `app/src/constants/oauth.ts`

```typescript
// Registro de apps:
// Google: console.cloud.google.com → Credenciales → OAuth 2.0 Client ID
//         Tipo: "Aplicación Android" + "Aplicación iOS" (una por plataforma)
//         o "Aplicación web" con redirect URI de Expo
// Spotify: developer.spotify.com → Create App
//          Redirect URI: exp://localhost:8081/--/oauth/spotify (Expo Go)
//                        aegis://oauth/spotify (build nativa)

export const OAUTH_CONFIG = {
  google: {
    clientId: 'PLACEHOLDER_GOOGLE_CLIENT_ID',  // Tavo reemplaza esto
    // Para Expo Go usar el clientId de "Web application"
    // Para builds nativas usar el clientId de Android/iOS
    scopes: [
      'https://www.googleapis.com/auth/youtube.readonly',
      'https://www.googleapis.com/auth/calendar.readonly',
      'https://www.googleapis.com/auth/drive.readonly',
      'https://www.googleapis.com/auth/gmail.readonly',
      'email',
      'profile',
    ],
    authorizationEndpoint: 'https://accounts.google.com/o/oauth2/v2/auth',
    tokenEndpoint: 'https://oauth2.googleapis.com/token',
  },
  spotify: {
    clientId: 'PLACEHOLDER_SPOTIFY_CLIENT_ID',  // Tavo reemplaza esto
    scopes: [
      'user-read-playback-state',
      'user-modify-playback-state',
      'user-read-currently-playing',
      'streaming',
      'playlist-read-private',
    ],
    authorizationEndpoint: 'https://accounts.spotify.com/authorize',
    tokenEndpoint: 'https://accounts.spotify.com/api/token',
  },
} as const;
```

### 3. Servicio `app/src/services/oauthService.ts`

```typescript
import * as AuthSession from 'expo-auth-session';
import * as WebBrowser from 'expo-web-browser';
import { OAUTH_CONFIG } from '@/constants/oauth';
import { buildUrl } from './bffClient';

// Requerido para que el browser se cierre automáticamente en iOS
WebBrowser.maybeCompleteAuthSession();

export interface OAuthTokens {
  provider: 'google' | 'spotify';
  accessToken: string;
  refreshToken: string | null;
  expiresIn: number;
  scope: string;
}

export interface ConnectedAccountStatus {
  connected: boolean;
  email?: string;
  scope?: string;
}

/**
 * Inicia el flujo OAuth para un provider.
 * Abre el browser del sistema, el usuario autoriza, y retorna los tokens.
 */
export async function connectProvider(
  provider: 'google' | 'spotify'
): Promise<OAuthTokens> {
  const config = OAUTH_CONFIG[provider];
  const redirectUri = AuthSession.makeRedirectUri({
    scheme: 'aegis',        // deep link nativo
    path: `oauth/${provider}`,
  });

  const discovery = {
    authorizationEndpoint: config.authorizationEndpoint,
    tokenEndpoint: config.tokenEndpoint,
  };

  const request = new AuthSession.AuthRequest({
    clientId: config.clientId,
    scopes: [...config.scopes],
    redirectUri,
    usePKCE: true,  // PKCE — sin necesidad de Client Secret
    responseType: AuthSession.ResponseType.Code,
  });

  const result = await request.promptAsync(discovery);

  if (result.type !== 'success') {
    throw new Error(
      result.type === 'cancel'
        ? 'Cancelado por el usuario'
        : `OAuth error: ${result.type}`
    );
  }

  // Intercambiar code por tokens
  const tokenResult = await AuthSession.exchangeCodeAsync(
    {
      clientId: config.clientId,
      code: result.params.code,
      redirectUri,
      extraParams: { code_verifier: request.codeVerifier ?? '' },
    },
    discovery
  );

  if (!tokenResult.accessToken) {
    throw new Error('No se recibió access token');
  }

  return {
    provider,
    accessToken: tokenResult.accessToken,
    refreshToken: tokenResult.refreshToken ?? null,
    expiresIn: tokenResult.expiresIn ?? 3600,
    scope: tokenResult.scope ?? config.scopes.join(' '),
  };
}

/**
 * Envía los tokens al servidor Aegis para que los guarde
 * en el enclave SQLCipher del tenant.
 */
export async function saveTokensToServer(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  tokens: OAuthTokens
): Promise<void> {
  const res = await fetch(buildUrl(serverUrl, '/api/oauth/tokens'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
    body: JSON.stringify({
      provider: tokens.provider,
      access_token: tokens.accessToken,
      refresh_token: tokens.refreshToken,
      expires_in: tokens.expiresIn,
      scope: tokens.scope,
    }),
  });

  if (!res.ok) {
    throw new Error(`Server error saving tokens: ${res.status}`);
  }
}

/**
 * Consulta al servidor el estado de las conexiones OAuth del tenant.
 */
export async function getOAuthStatus(
  serverUrl: string,
  tenantId: string,
  sessionKey: string
): Promise<Record<string, ConnectedAccountStatus>> {
  const res = await fetch(buildUrl(serverUrl, '/api/oauth/status'), {
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
  if (!res.ok) return {};
  return res.json();
}

/**
 * Desconecta un provider — elimina tokens del servidor.
 */
export async function disconnectProvider(
  serverUrl: string,
  tenantId: string,
  sessionKey: string,
  provider: string
): Promise<void> {
  await fetch(buildUrl(serverUrl, `/api/oauth/${provider}`), {
    method: 'DELETE',
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
}
```

### 4. Pantalla `app/app/(main)/connected-accounts.tsx`

```tsx
import React, { useState, useEffect } from 'react';
import {
  View, Text, TouchableOpacity, StyleSheet,
  ActivityIndicator, Alert, ScrollView, Linking
} from 'react-native';
import { useAuthStore } from '@/stores/authStore';
import {
  connectProvider, saveTokensToServer,
  getOAuthStatus, disconnectProvider,
  ConnectedAccountStatus
} from '@/services/oauthService';

const PROVIDERS = [
  {
    id: 'google' as const,
    name: 'Google',
    description: 'YouTube Music · Calendar · Drive · Gmail',
    color: '#4285F4',
  },
  {
    id: 'spotify' as const,
    name: 'Spotify',
    description: 'Reproducción de música · Playlists',
    color: '#1DB954',
  },
];

export default function ConnectedAccountsScreen() {
  const { serverUrl, tenantId, sessionKey } = useAuthStore();
  const [status, setStatus] = useState<Record<string, ConnectedAccountStatus>>({});
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState<string | null>(null);

  const fetchStatus = async () => {
    if (!serverUrl || !tenantId || !sessionKey) return;
    const s = await getOAuthStatus(serverUrl, tenantId, sessionKey);
    setStatus(s);
    setLoading(false);
  };

  useEffect(() => { fetchStatus(); }, []);

  const handleConnect = async (provider: 'google' | 'spotify') => {
    if (!serverUrl || !tenantId || !sessionKey) return;
    setConnecting(provider);
    try {
      const tokens = await connectProvider(provider);
      await saveTokensToServer(serverUrl, tenantId, sessionKey, tokens);
      await fetchStatus();
      Alert.alert('✓ Conectado', `Tu cuenta de ${provider === 'google' ? 'Google' : 'Spotify'} fue vinculada exitosamente.`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Error desconocido';
      if (!msg.includes('Cancelado')) {
        Alert.alert('Error', msg);
      }
    } finally {
      setConnecting(null);
    }
  };

  const handleDisconnect = (provider: string) => {
    Alert.alert(
      'Desconectar',
      `¿Querés desconectar tu cuenta de ${provider}?`,
      [
        { text: 'Cancelar', style: 'cancel' },
        {
          text: 'Desconectar',
          style: 'destructive',
          onPress: async () => {
            if (!serverUrl || !tenantId || !sessionKey) return;
            await disconnectProvider(serverUrl, tenantId, sessionKey, provider);
            await fetchStatus();
          },
        },
      ]
    );
  };

  if (loading) {
    return (
      <View style={styles.center}>
        <ActivityIndicator color="#00F2FE" />
      </View>
    );
  }

  return (
    <ScrollView style={styles.container}>
      <Text style={styles.title}>Cuentas Conectadas</Text>
      <Text style={styles.subtitle}>
        Conectá tus cuentas para que Aegis pueda reproducir música,
        consultar tu calendario, archivos y más.
      </Text>

      {PROVIDERS.map((provider) => {
        const s = status[provider.id];
        const isConnected = s?.connected ?? false;
        const isBusy = connecting === provider.id;

        return (
          <View key={provider.id} style={[
            styles.card,
            isConnected && { borderColor: provider.color + '50' }
          ]}>
            <View style={styles.cardInfo}>
              <Text style={styles.providerName}>{provider.name}</Text>
              <Text style={styles.providerDesc}>{provider.description}</Text>
              {isConnected && s?.email && (
                <Text style={[styles.email, { color: provider.color }]}>
                  {s.email}
                </Text>
              )}
            </View>

            {isConnected ? (
              <TouchableOpacity
                onPress={() => handleDisconnect(provider.id)}
                style={styles.disconnectBtn}
              >
                <Text style={styles.disconnectText}>Desconectar</Text>
              </TouchableOpacity>
            ) : (
              <TouchableOpacity
                onPress={() => handleConnect(provider.id)}
                disabled={!!connecting}
                style={[styles.connectBtn, { backgroundColor: provider.color }]}
              >
                {isBusy
                  ? <ActivityIndicator color="#fff" size="small" />
                  : <Text style={styles.connectText}>Conectar</Text>
                }
              </TouchableOpacity>
            )}
          </View>
        );
      })}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#000', padding: 20 },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  title: { fontSize: 20, fontWeight: 'bold', color: '#fff', marginBottom: 8 },
  subtitle: { fontSize: 13, color: 'rgba(255,255,255,0.4)', marginBottom: 24, lineHeight: 18 },
  card: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    backgroundColor: 'rgba(255,255,255,0.05)',
    borderRadius: 16, borderWidth: 1, borderColor: 'rgba(255,255,255,0.1)',
    padding: 16, marginBottom: 12,
  },
  cardInfo: { flex: 1, marginRight: 12 },
  providerName: { fontSize: 15, fontWeight: 'bold', color: '#fff' },
  providerDesc: { fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 },
  email: { fontSize: 11, marginTop: 4, fontWeight: '600' },
  connectBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 8, minWidth: 90, alignItems: 'center' },
  connectText: { color: '#fff', fontSize: 13, fontWeight: 'bold' },
  disconnectBtn: { paddingHorizontal: 12, paddingVertical: 8, borderRadius: 8, borderWidth: 1, borderColor: 'rgba(255,255,255,0.2)' },
  disconnectText: { color: 'rgba(255,255,255,0.5)', fontSize: 12 },
});
```

### 5. Registrar pantalla en la navegación

En `app/app/(main)/_layout.tsx`, agregar la ruta `connected-accounts`.
En `app/app/(main)/settings.tsx`, agregar un item de navegación "Cuentas Conectadas"
que lleve a `/(main)/connected-accounts`.

### 6. Deep link en `app.json`

```json
{
  "expo": {
    "scheme": "aegis",
    "...": "..."
  }
}
```

---

## Acción requerida por Tavo (fuera del código)

**Google Cloud Console:**
1. Crear proyecto "Aegis OS"
2. Habilitar: YouTube Data API v3, Calendar API, Drive API, Gmail API
3. Pantalla de consentimiento OAuth → "Externo" → nombre "Aegis OS"
4. Credenciales → OAuth 2.0 Client ID:
   - Tipo "Android" → package name del app Expo → SHA-1 del keystore
   - Tipo "iOS" → bundle ID del app Expo
   - Tipo "Web application" → redirect URI: `https://auth.expo.io/@<username>/aegis` (para Expo Go)
5. Reemplazar `PLACEHOLDER_GOOGLE_CLIENT_ID` en `oauth.ts`

**Spotify Developer Dashboard:**
1. Crear app "Aegis OS"
2. Redirect URIs: `aegis://oauth/spotify` + `exp://localhost:8081/--/oauth/spotify`
3. Reemplazar `PLACEHOLDER_SPOTIFY_CLIENT_ID` en `oauth.ts`

---

## Criterios de aceptación

- [ ] `expo start` sin errores TypeScript
- [ ] Pantalla "Cuentas Conectadas" accesible desde Settings
- [ ] Con Client IDs reales: "Conectar Google" abre Google en el browser y retorna a la app
- [ ] Tokens se guardan en el servidor — `GET /api/oauth/status` refleja el estado
- [ ] Email de la cuenta conectada visible en la pantalla
- [ ] Desconectar elimina los tokens del servidor
- [ ] Con placeholders: botón muestra error amigable (no crash)

---

## Dependencias

- CORE-138 (endpoint `POST /api/oauth/tokens` en el servidor)

---

## Commit message

```
feat(app): CORE-143 OAuth via expo-auth-session — Google and Spotify connect from mobile app
```
