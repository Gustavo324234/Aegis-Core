import * as AuthSession from 'expo-auth-session';
import * as WebBrowser from 'expo-web-browser';
import { OAUTH_CONFIG } from '@/constants/oauth';
import { buildUrl } from './bffClient';

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

export async function connectProvider(
  provider: 'google' | 'spotify'
): Promise<OAuthTokens> {
  const config = OAUTH_CONFIG[provider];
  const redirectUri = AuthSession.makeRedirectUri({
    scheme: 'aegis',
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