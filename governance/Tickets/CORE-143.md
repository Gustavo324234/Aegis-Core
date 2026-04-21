# CORE-143 — Feature: OAuth en App Mobile — Google y Spotify via expo-auth-session

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `app/`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Shell Engineer (Antigravity)
**Depende de:** CORE-138 (endpoint receptor de tokens en el servidor)

---

## Estado de Client IDs ✅

Los Client IDs ya están configurados en `app/src/constants/oauth.ts`:

| Provider | Tipo | Client ID | Estado |
|---|---|---|---|
| Google | Web (Expo Go) | `201101395662-v13ic0mv07drv8dvrucaos6kkqaaps0i.apps.googleusercontent.com` | ✅ |
| Google | iOS nativo | `201101395662-an905drm5aqqog3sae5qp6ghq97o8vpi.apps.googleusercontent.com` | ✅ |
| Google | Android | Pendiente SHA-1 del keystore EAS | ⏳ |
| Spotify | — | `3b1ff5d1f6d04fb3af5bdb1489062644` | ✅ |

**No hay placeholders que reemplazar — el archivo ya existe con los valores reales.**

---

## Arquitectura

```
App mobile
    │  Client IDs en app/src/constants/oauth.ts
    │  expo-auth-session maneja el flujo OAuth + PKCE
    ▼
Google / Spotify  →  access_token + refresh_token  →  App
    │
    ▼
App → POST /api/oauth/tokens → Servidor Aegis
    │  { provider, access_token, refresh_token, expires_in, scope }
    │  headers: x-citadel-tenant + x-citadel-key
    ▼
Servidor guarda tokens en TenantDB (SQLCipher del tenant)
```

**El servidor nunca habla con Google/Spotify para OAuth.**
**Sin Device Flow. Sin redirect URI en el servidor.**

---

## Cambios requeridos

### 1. Verificar dependencias en `app/package.json`

`expo-auth-session` y `expo-web-browser` deben estar instalados:
```bash
cd app && npx expo install expo-auth-session expo-web-browser
```

### 2. Crear servicio `app/src/services/oauthService.ts`

```typescript
import * as AuthSession from 'expo-auth-session';
import * as WebBrowser from 'expo-web-browser';
import { Platform } from 'react-native';
import { OAUTH_CONFIG } from '@/constants/oauth';
import { buildUrl } from './bffClient';

// Requerido para iOS — cierra el browser automáticamente al volver
WebBrowser.maybeCompleteAuthSession();

export interface OAuthTokens {
  provider: 'google' | 'spotify';
  accessToken: string;
  refreshToken: string | null;
  expiresIn: number;
  scope: string;
  email?: string;
}

export interface ConnectedAccountStatus {
  connected: boolean;
  email?: string;
  scope?: string;
}

/**
 * Inicia el flujo OAuth para un provider via PKCE.
 * Abre el browser del sistema, el usuario autoriza, y retorna los tokens.
 */
export async function connectProvider(
  provider: 'google' | 'spotify'
): Promise<OAuthTokens> {
  const config = OAUTH_CONFIG[provider];

  // Seleccionar el Client ID correcto según plataforma
  let clientId = config.clientId;
  if (provider === 'google') {
    if (Platform.OS === 'ios' && config.iosClientId) {
      clientId = config.iosClientId;
    } else if (Platform.OS === 'android' && config.androidClientId) {
      clientId = config.androidClientId;
    }
    // En web/Expo Go: usa el Web Client ID (default)
  }

  const redirectUri = AuthSession.makeRedirectUri({
    scheme: 'aegis',
    path: `oauth/${provider}`,
  });

  const discovery = {
    authorizationEndpoint: config.authorizationEndpoint,
    tokenEndpoint: config.tokenEndpoint,
  };

  const request = new AuthSession.AuthRequest({
    clientId,
    scopes: [...config.scopes],
    redirectUri,
    usePKCE: true,
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

  const tokenResult = await AuthSession.exchangeCodeAsync(
    {
      clientId,
      code: result.params.code,
      redirectUri,
      extraParams: { code_verifier: request.codeVerifier ?? '' },
    },
    discovery
  );

  if (!tokenResult.accessToken) {
    throw new Error('No se recibió access token');
  }

  // Para Google: obtener email del usuario
  let email: string | undefined;
  if (provider === 'google') {
    try {
      const userInfo = await fetch(
        'https://www.googleapis.com/oauth2/v3/userinfo',
        { headers: { Authorization: `Bearer ${tokenResult.accessToken}` } }
      ).then(r => r.json());
      email = userInfo.email;
    } catch { /* best-effort */ }
  }

  return {
    provider,
    accessToken: tokenResult.accessToken,
    refreshToken: tokenResult.refreshToken ?? null,
    expiresIn: tokenResult.expiresIn ?? 3600,
    scope: tokenResult.scope ?? config.scopes.join(' '),
    email,
  };
}

/**
 * Envía los tokens al servidor Aegis para guardarlos en SQLCipher.
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
      email: tokens.email,
    }),
  });

  if (!res.ok) {
    throw new Error(`Server error saving tokens: ${res.status}`);
  }
}

export async function getOAuthStatus(
  serverUrl: string,
  tenantId: string,
  sessionKey: string
): Promise<Record<string, ConnectedAccountStatus>> {
  try {
    const res = await fetch(buildUrl(serverUrl, '/api/oauth/status'), {
      headers: {
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
      },
    });
    if (!res.ok) return {};
    return res.json();
  } catch { return {}; }
}

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

### 3. Crear pantalla `app/app/(main)/connected-accounts.tsx`

```tsx
import React, { useState, useEffect } from 'react';
import {
  View, Text, TouchableOpacity, StyleSheet,
  ActivityIndicator, Alert, ScrollView,
} from 'react-native';
import { useAuthStore } from '@/stores/authStore';
import {
  connectProvider, saveTokensToServer,
  getOAuthStatus, disconnectProvider,
  ConnectedAccountStatus,
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
    setStatus(await getOAuthStatus(serverUrl, tenantId, sessionKey));
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
      Alert.alert('✓ Conectado',
        `Tu cuenta de ${provider === 'google' ? 'Google' : 'Spotify'} fue vinculada.`);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Error desconocido';
      if (!msg.includes('Cancelado')) Alert.alert('Error', msg);
    } finally {
      setConnecting(null);
    }
  };

  const handleDisconnect = (provider: string) => {
    Alert.alert('Desconectar', `¿Desconectar ${provider}?`, [
      { text: 'Cancelar', style: 'cancel' },
      {
        text: 'Desconectar', style: 'destructive',
        onPress: async () => {
          if (!serverUrl || !tenantId || !sessionKey) return;
          await disconnectProvider(serverUrl, tenantId, sessionKey, provider);
          await fetchStatus();
        },
      },
    ]);
  };

  if (loading) return (
    <View style={s.center}><ActivityIndicator color="#00F2FE" /></View>
  );

  return (
    <ScrollView style={s.container}>
      <Text style={s.title}>Cuentas Conectadas</Text>
      <Text style={s.subtitle}>
        Conectá tus cuentas para que Aegis pueda reproducir música,
        ver tu calendario, archivos y más.
      </Text>
      {PROVIDERS.map((p) => {
        const st = status[p.id];
        const connected = st?.connected ?? false;
        const busy = connecting === p.id;
        return (
          <View key={p.id} style={[s.card, connected && { borderColor: p.color + '50' }]}>
            <View style={s.info}>
              <Text style={s.name}>{p.name}</Text>
              <Text style={s.desc}>{p.description}</Text>
              {connected && st?.email && (
                <Text style={[s.email, { color: p.color }]}>{st.email}</Text>
              )}
            </View>
            {connected ? (
              <TouchableOpacity onPress={() => handleDisconnect(p.id)} style={s.discBtn}>
                <Text style={s.discTxt}>Desconectar</Text>
              </TouchableOpacity>
            ) : (
              <TouchableOpacity
                onPress={() => handleConnect(p.id)}
                disabled={!!connecting}
                style={[s.connBtn, { backgroundColor: p.color }]}
              >
                {busy
                  ? <ActivityIndicator color="#fff" size="small" />
                  : <Text style={s.connTxt}>Conectar</Text>
                }
              </TouchableOpacity>
            )}
          </View>
        );
      })}
    </ScrollView>
  );
}

const s = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#000', padding: 20 },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  title: { fontSize: 20, fontWeight: 'bold', color: '#fff', marginBottom: 8 },
  subtitle: { fontSize: 13, color: 'rgba(255,255,255,0.4)', marginBottom: 24, lineHeight: 18 },
  card: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    backgroundColor: 'rgba(255,255,255,0.05)', borderRadius: 16,
    borderWidth: 1, borderColor: 'rgba(255,255,255,0.1)', padding: 16, marginBottom: 12,
  },
  info: { flex: 1, marginRight: 12 },
  name: { fontSize: 15, fontWeight: 'bold', color: '#fff' },
  desc: { fontSize: 11, color: 'rgba(255,255,255,0.4)', marginTop: 2 },
  email: { fontSize: 11, marginTop: 4, fontWeight: '600' },
  connBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 8, minWidth: 90, alignItems: 'center' },
  connTxt: { color: '#fff', fontSize: 13, fontWeight: 'bold' },
  discBtn: { paddingHorizontal: 12, paddingVertical: 8, borderRadius: 8, borderWidth: 1, borderColor: 'rgba(255,255,255,0.2)' },
  discTxt: { color: 'rgba(255,255,255,0.5)', fontSize: 12 },
});
```

### 4. Agregar ruta en `app/app/(main)/_layout.tsx`

Agregar `connected-accounts` como ruta dentro del layout principal.

### 5. Agregar acceso desde Settings

En `app/app/(main)/settings.tsx`, agregar un item que navegue a `/(main)/connected-accounts`.

---

## Criterios de aceptación

- [ ] `npx expo export` sin errores TypeScript
- [ ] Pantalla "Cuentas Conectadas" accesible desde Settings
- [ ] "Conectar Google" abre Google OAuth y retorna a la app
- [ ] "Conectar Spotify" abre Spotify OAuth y retorna a la app
- [ ] Tokens se guardan en el servidor — `GET /api/oauth/status` refleja el estado
- [ ] Email de la cuenta Google visible en la pantalla
- [ ] Desconectar elimina los tokens del servidor

---

## Dependencias

- CORE-138 (endpoint `POST /api/oauth/tokens` operativo en el servidor)
- `app/src/constants/oauth.ts` ya existe con los Client IDs reales ✅

---

## Commit message

```
feat(app): CORE-143 OAuth via expo-auth-session — Google and Spotify connect from mobile app
```
