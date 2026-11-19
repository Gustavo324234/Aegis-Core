import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import {
  Chrome,
  Music2,
  CheckCircle,
  Unplug,
  Loader2,
  MessageCircle,
  QrCode,
  Shield,
  Send,
  Trash2,
  RefreshCw,
  Copy,
  Check,
  ChevronDown,
  ChevronUp,
  AlertCircle,
  Clock,
  Sliders,
  LogOut,
  Key,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

interface OAuthProviderStatus {
  connected: boolean;
  scope?: string | null;
  email?: string | null;
  client_configured?: boolean;
  client_id?: string | null;
  has_client_secret?: boolean;
  redirect_uri?: string | null;
}

interface WhatsAppRateConfig {
  min_interval_secs: number;
  max_per_minute: number;
  max_per_hour: number;
  max_per_day: number;
  max_per_recipient_10m: number;
  dedup_window_secs: number;
}

interface WhatsAppRateUsage {
  sent_last_minute: number;
  sent_last_hour: number;
  sent_last_day: number;
  config: WhatsAppRateConfig;
}

interface WhatsAppPendingMessage {
  id: string;
  to_input: string;
  to_jid: string;
  recipient_label: string;
  text: string;
  created_at_epoch: number;
  expires_at_epoch: number;
}

interface WhatsAppConnectionState {
  tenant_id?: string;
  status: 'disconnected' | 'connecting' | 'qr_ready' | 'connected' | 'logged_out' | string;
  connected: boolean;
  phone?: string | null;
  name?: string | null;
  jid?: string | null;
  qr_data_url?: string | null;
  qr_updated_at?: string | null;
  last_error?: string | null;
  chats_count?: number;
}

interface WhatsAppStatusResponse {
  bridge_available: boolean;
  has_saved_session: boolean;
  require_confirmation: boolean;
  rate_usage: WhatsAppRateUsage;
  pending_messages: WhatsAppPendingMessage[];
  connection: WhatsAppConnectionState;
}

function getApiBase(): string {
  if (typeof window === 'undefined') return '';
  const match = window.location.pathname.match(/^\/u\/[^/]+/);
  return match ? match[0] : '';
}

function formatEpochTime(epochSecs: number): string {
  if (!epochSecs) return '';
  try {
    return new Date(epochSecs * 1000).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return '';
  }
}

const ConnectedAccountsTab: React.FC<{
  tenantId: string;
  sessionKey: string;
}> = ({ tenantId, sessionKey }) => {
  const assistantDisplayName = useAegisStore((s) => s.agentName) || 'Aegis';
  const apiBase = useMemo(() => getApiBase(), []);
  const defaultRedirectUri = useMemo(() => {
    if (typeof window === 'undefined') return '/api/oauth/google/callback';
    return `${window.location.origin}${apiBase}/api/oauth/google/callback`;
  }, [apiBase]);

  const headers = useMemo(
    () => ({
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    }),
    [tenantId, sessionKey],
  );

  // OAuth state
  const [oauthStatus, setOauthStatus] = useState<Record<string, OAuthProviderStatus>>({});
  const [isLoadingOAuth, setIsLoadingOAuth] = useState(true);
  const [oauthBanner, setOauthBanner] = useState<{ type: 'ok' | 'err'; text: string } | null>(null);
  const [showGoogleConfig, setShowGoogleConfig] = useState(false);
  const [clientIdInput, setClientIdInput] = useState('');
  const [clientSecretInput, setClientSecretInput] = useState('');
  const [redirectUriInput, setRedirectUriInput] = useState('');
  const [includeExtraScopes, setIncludeExtraScopes] = useState(false);
  const [savingGoogleConfig, setSavingGoogleConfig] = useState(false);
  const [startingGoogleOAuth, setStartingGoogleOAuth] = useState(false);
  const [fallbackAuthUrl, setFallbackAuthUrl] = useState<string | null>(null);
  const [copiedRedirect, setCopiedRedirect] = useState(false);
  const googleConfigInitRef = useRef(false);
  const oauthPopupTimerRef = useRef<number | null>(null);

  // WhatsApp state
  const [waStatus, setWaStatus] = useState<WhatsAppStatusResponse | null>(null);
  const [isLoadingWa, setIsLoadingWa] = useState(true);
  const [waBusy, setWaBusy] = useState(false);
  const [pendingActionId, setPendingActionId] = useState<string | null>(null);
  const [waBanner, setWaBanner] = useState<{ type: 'ok' | 'err'; text: string } | null>(null);
  const [showRateConfig, setShowRateConfig] = useState(false);
  const [rateDraft, setRateDraft] = useState<WhatsAppRateConfig>({
    min_interval_secs: 5,
    max_per_minute: 6,
    max_per_hour: 30,
    max_per_day: 120,
    max_per_recipient_10m: 5,
    dedup_window_secs: 60,
  });
  const rateDraftDirtyRef = useRef(false);

  const fetchOAuthStatus = useCallback(async () => {
    try {
      const res = await fetch(`${apiBase}/api/oauth/status`, { headers });
      if (res.ok) {
        const data: Record<string, OAuthProviderStatus> = await res.json();
        setOauthStatus(data);
        const g = data.google;
        if (g && !googleConfigInitRef.current) {
          setClientIdInput(g.client_id || '');
          setRedirectUriInput(g.redirect_uri || '');
          if (!g.client_configured && !g.connected) {
            setShowGoogleConfig(true);
          }
          googleConfigInitRef.current = true;
        }
      }
    } catch {
      // Ignore transient network errors
    } finally {
      setIsLoadingOAuth(false);
    }
  }, [apiBase, headers]);

  const fetchWaStatus = useCallback(async () => {
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/status`, { headers });
      if (res.ok) {
        const data: WhatsAppStatusResponse = await res.json();
        setWaStatus(data);
        if (!rateDraftDirtyRef.current && data.rate_usage?.config) {
          setRateDraft(data.rate_usage.config);
        }
      }
    } catch {
      // Ignore transient network errors
    } finally {
      setIsLoadingWa(false);
    }
  }, [apiBase, headers]);

  useEffect(() => {
    fetchOAuthStatus();
    fetchWaStatus();
  }, [fetchOAuthStatus, fetchWaStatus]);

  // Listen for OAuth popup postMessage completion
  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const payload = event.data;
      if (!payload || typeof payload !== 'object') return;
      if (payload.type !== 'aegis-oauth-complete') return;

      if (oauthPopupTimerRef.current) {
        window.clearInterval(oauthPopupTimerRef.current);
        oauthPopupTimerRef.current = null;
      }
      setStartingGoogleOAuth(false);
      setFallbackAuthUrl(null);
      if (payload.success) {
        setOauthBanner({
          type: 'ok',
          text: payload.email
            ? `Google conectado como ${payload.email}.`
            : 'Google conectado correctamente.',
        });
      } else {
        const errDetail = payload.error || payload.message;
        setOauthBanner({
          type: 'err',
          text: errDetail
            ? `Error de Google OAuth: ${errDetail}`
            : 'No se pudo completar la conexión con Google.',
        });
      }
      fetchOAuthStatus();
    };
    window.addEventListener('message', onMessage);
    return () => {
      window.removeEventListener('message', onMessage);
      if (oauthPopupTimerRef.current) {
        window.clearInterval(oauthPopupTimerRef.current);
      }
    };
  }, [fetchOAuthStatus]);

  // Poll WhatsApp status faster while QR pairing / connecting
  const waConnState = waStatus?.connection?.status;
  useEffect(() => {
    const intervalMs =
      waConnState === 'connecting' || waConnState === 'qr_ready' ? 2000 : 6000;
    const timer = window.setInterval(() => {
      fetchWaStatus();
    }, intervalMs);
    return () => window.clearInterval(timer);
  }, [waConnState, fetchWaStatus]);

  const effectiveRedirectUri = redirectUriInput.trim() || oauthStatus.google?.redirect_uri || defaultRedirectUri;

  const handleCopyRedirect = async () => {
    try {
      await navigator.clipboard.writeText(effectiveRedirectUri);
      setCopiedRedirect(true);
      window.setTimeout(() => setCopiedRedirect(false), 1800);
    } catch {
      // Clipboard might be unavailable in insecure context
    }
  };

  const handleSaveGoogleConfig = async () => {
    const trimmedId = clientIdInput.trim();
    if (!trimmedId) {
      setOauthBanner({ type: 'err', text: 'Ingresá un Client ID de Google OAuth válido.' });
      return;
    }
    setSavingGoogleConfig(true);
    setOauthBanner(null);
    try {
      const body: Record<string, unknown> = {
        client_id: trimmedId,
      };
      if (clientSecretInput.trim()) {
        body.client_secret = clientSecretInput.trim();
      }
      if (redirectUriInput.trim()) {
        body.redirect_uri = redirectUriInput.trim();
      }
      const res = await fetch(`${apiBase}/api/oauth/google/config`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setClientSecretInput('');
      setOauthBanner({ type: 'ok', text: 'Credenciales de Google OAuth guardadas.' });
      await fetchOAuthStatus();
    } catch (err) {
      setOauthBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'Error guardando la configuración de Google OAuth.',
      });
    } finally {
      setSavingGoogleConfig(false);
    }
  };

  const handleClearGoogleConfig = async () => {
    setSavingGoogleConfig(true);
    setOauthBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/oauth/google/config`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ client_id: '', clear: true }),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setClientIdInput('');
      setClientSecretInput('');
      setRedirectUriInput('');
      setOauthBanner({ type: 'ok', text: 'Configuración de Google OAuth limpiada.' });
      await fetchOAuthStatus();
    } catch (err) {
      setOauthBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'Error limpiando la configuración.',
      });
    } finally {
      setSavingGoogleConfig(false);
    }
  };

  const handleConnectGoogle = async () => {
    setStartingGoogleOAuth(true);
    setOauthBanner(null);
    setFallbackAuthUrl(null);
    try {
      const res = await fetch(`${apiBase}/api/oauth/google/start`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({
          redirect_uri: effectiveRedirectUri,
          include_extra_scopes: includeExtraScopes,
        }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      const authUrl: string = data.auth_url;
      const popup = window.open(
        authUrl,
        'aegis-google-oauth',
        'width=540,height=680,menubar=no,toolbar=no,location=yes',
      );
      if (!popup) {
        setFallbackAuthUrl(authUrl);
        setStartingGoogleOAuth(false);
        setOauthBanner({
          type: 'err',
          text: 'El navegador bloqueó la ventana emergente. Abrí el enlace manual abajo para autorizar Google.',
        });
        return;
      }

      if (oauthPopupTimerRef.current) {
        window.clearInterval(oauthPopupTimerRef.current);
      }
      oauthPopupTimerRef.current = window.setInterval(() => {
        fetchOAuthStatus();
        if (popup.closed) {
          if (oauthPopupTimerRef.current) {
            window.clearInterval(oauthPopupTimerRef.current);
            oauthPopupTimerRef.current = null;
          }
          setStartingGoogleOAuth(false);
        }
      }, 1500);
    } catch (err) {
      setStartingGoogleOAuth(false);
      setShowGoogleConfig(true);
      setOauthBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'No se pudo iniciar Google OAuth.',
      });
    }
  };

  const handleDisconnectOAuth = async (provider: string, clearClient = false) => {
    setOauthBanner(null);
    try {
      const suffix = clearClient ? '?clear_client=true' : '';
      const res = await fetch(`${apiBase}/api/oauth/${provider}${suffix}`, {
        method: 'DELETE',
        headers,
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      if (clearClient) {
        setClientIdInput('');
        setClientSecretInput('');
        setRedirectUriInput('');
      }
      setOauthBanner({
        type: 'ok',
        text: provider === 'google' ? 'Cuenta de Google desconectada.' : 'Cuenta de Spotify desconectada.',
      });
      await fetchOAuthStatus();
    } catch (err) {
      setOauthBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'No se pudo desconectar la cuenta.',
      });
    }
  };

  // WhatsApp handlers
  const handleWaConnect = async () => {
    setWaBusy(true);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/connect`, {
        method: 'POST',
        headers,
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'No se pudo iniciar la sesión de WhatsApp.',
      });
    } finally {
      setWaBusy(false);
    }
  };

  const handleWaDisconnect = async (logout: boolean) => {
    setWaBusy(true);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/disconnect`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ logout }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setWaBanner({
        type: 'ok',
        text: logout
          ? 'Sesión de WhatsApp desvinculada del dispositivo.'
          : 'Puente de WhatsApp desconectado (sesión guardada).',
      });
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'Error desconectando WhatsApp.',
      });
    } finally {
      setWaBusy(false);
    }
  };

  const handleToggleWaConfirmation = async () => {
    if (!waStatus) return;
    const nextValue = !waStatus.require_confirmation;
    setWaBusy(true);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/settings`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ require_confirmation: nextValue }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setWaBanner({
        type: 'ok',
        text: nextValue
          ? 'Confirmación activada: los envíos quedarán como borrador hasta aprobarlos.'
          : 'Confirmación desactivada: los mensajes pasarán directo por el limitador anti-spam.',
      });
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'Error actualizando política de confirmación.',
      });
    } finally {
      setWaBusy(false);
    }
  };

  const handleSaveRateConfig = async () => {
    setWaBusy(true);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/settings`, {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ rate_config: rateDraft }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      rateDraftDirtyRef.current = false;
      setWaBanner({ type: 'ok', text: 'Límites anti-spam de WhatsApp actualizados.' });
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'Error guardando límites anti-spam.',
      });
    } finally {
      setWaBusy(false);
    }
  };

  const handleConfirmPending = async (id: string) => {
    setPendingActionId(id);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/pending/${encodeURIComponent(id)}/confirm`, {
        method: 'POST',
        headers,
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setWaBanner({ type: 'ok', text: 'Mensaje enviado por WhatsApp.' });
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'No se pudo enviar el borrador.',
      });
    } finally {
      setPendingActionId(null);
    }
  };

  const handleCancelPending = async (id: string) => {
    setPendingActionId(id);
    setWaBanner(null);
    try {
      const res = await fetch(`${apiBase}/api/whatsapp/pending/${encodeURIComponent(id)}`, {
        method: 'DELETE',
        headers,
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      setWaBanner({
        type: 'ok',
        text: id === 'all' ? 'Todos los borradores pendientes fueron cancelados.' : 'Borrador cancelado.',
      });
      await fetchWaStatus();
    } catch (err) {
      setWaBanner({
        type: 'err',
        text: err instanceof Error ? err.message : 'No se pudo cancelar el borrador.',
      });
    } finally {
      setPendingActionId(null);
    }
  };

  const google = oauthStatus.google;
  const spotify = oauthStatus.spotify;
  const conn = waStatus?.connection;
  const pendingList = waStatus?.pending_messages || [];
  const usage = waStatus?.rate_usage;

  const waBadge = (() => {
    switch (conn?.status) {
      case 'connected':
        return { cls: 'bg-green-500/10 border-green-500/30 text-green-400', label: 'Conectado' };
      case 'qr_ready':
        return { cls: 'bg-aegis-cyan/10 border-aegis-cyan/40 text-aegis-cyan', label: 'QR listo' };
      case 'connecting':
        return { cls: 'bg-yellow-500/10 border-yellow-500/30 text-yellow-300', label: 'Conectando' };
      case 'logged_out':
        return { cls: 'bg-amber-500/10 border-amber-500/30 text-amber-300', label: 'Sesión cerrada' };
      default:
        return { cls: 'bg-white/5 border-white/10 text-white/40', label: 'Desconectado' };
    }
  })();

  return (
    <div className="space-y-6">
      {/* ============================================================
          SECTION 1: GOOGLE & SPOTIFY OAUTH
      ============================================================ */}
      <div className="space-y-3">
        <div>
          <h3 className="text-xs font-mono font-bold uppercase tracking-widest text-white/80">
            Cuentas OAuth conectadas
          </h3>
          <p className="text-[11px] font-mono text-white/40 mt-1">
            Conectá Google Calendar directamente desde la web reutilizando el OAuth Client de Orion Creative (o uno propio).
          </p>
        </div>

        {oauthBanner && (
          <div
            className={`p-3 rounded-xl border text-xs font-mono flex items-start gap-2 ${
              oauthBanner.type === 'ok'
                ? 'bg-green-500/10 border-green-500/30 text-green-300'
                : 'bg-red-500/10 border-red-500/30 text-red-300'
            }`}
          >
            {oauthBanner.type === 'ok' ? (
              <CheckCircle className="w-4 h-4 shrink-0 mt-0.5" />
            ) : (
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            )}
            <span className="flex-1 break-words">{oauthBanner.text}</span>
          </div>
        )}

        {fallbackAuthUrl && (
          <div className="p-3 rounded-xl border border-aegis-cyan/30 bg-aegis-cyan/5 text-xs font-mono">
            <a
              href={fallbackAuthUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-aegis-cyan underline break-all"
            >
              Abrir autorización de Google en una nueva pestaña
            </a>
          </div>
        )}

        {isLoadingOAuth ? (
          <div className="flex items-center justify-center py-6">
            <Loader2 className="w-5 h-5 animate-spin text-aegis-cyan" />
          </div>
        ) : (
          <div className="space-y-3">
            {/* Google Card */}
            <div className="p-4 bg-white/[0.03] border border-white/10 rounded-xl space-y-3">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div className="p-2 rounded-lg bg-white/5 shrink-0">
                    <Chrome className="w-5 h-5 text-blue-400" />
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-sm font-mono font-bold text-white/90">
                        Google Calendar &amp; Workspace
                      </span>
                      {google?.connected ? (
                        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-mono bg-green-500/10 border border-green-500/30 text-green-400">
                          <CheckCircle className="w-3 h-3" />
                          Conectado
                        </span>
                      ) : (
                        <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-white/5 border border-white/10 text-white/40">
                          Desconectado
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] font-mono text-white/40 truncate mt-0.5">
                      {google?.connected
                        ? google.email || 'Cuenta vinculada (token activo con refresh automático)'
                        : google?.client_configured
                        ? 'OAuth Client listo para conectar'
                        : 'Falta configurar Client ID / Client Secret (o usar el de Orion Creative)'}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2 flex-wrap">
                  <button
                    type="button"
                    onClick={() => setShowGoogleConfig((v) => !v)}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-[11px] font-mono text-white/70 transition-colors"
                  >
                    <Key className="w-3.5 h-3.5 text-aegis-cyan" />
                    OAuth Client
                    {showGoogleConfig ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                  </button>

                  {google?.connected ? (
                    <>
                      <button
                        type="button"
                        onClick={handleConnectGoogle}
                        disabled={startingGoogleOAuth}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30 text-[11px] font-mono text-aegis-cyan transition-colors disabled:opacity-50"
                      >
                        {startingGoogleOAuth ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RefreshCw className="w-3.5 h-3.5" />}
                        Reautorizar
                      </button>
                      <button
                        type="button"
                        onClick={() => handleDisconnectOAuth('google', false)}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-[11px] font-mono text-red-400 transition-colors"
                      >
                        <Unplug className="w-3.5 h-3.5" />
                        Desconectar
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      onClick={handleConnectGoogle}
                      disabled={startingGoogleOAuth || !google?.client_configured}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-aegis-cyan text-black font-bold text-[11px] font-mono hover:opacity-90 transition-opacity disabled:opacity-40"
                    >
                      {startingGoogleOAuth ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Chrome className="w-3.5 h-3.5" />}
                      Conectar Google
                    </button>
                  )}
                </div>
              </div>

              {google?.connected && google.scope && (
                <div className="text-[10px] font-mono text-white/35 break-all bg-black/30 px-2.5 py-1.5 rounded-lg border border-white/5">
                  Scopes: {google.scope}
                </div>
              )}

              {showGoogleConfig && (
                <div className="pt-3 border-t border-white/10 space-y-3">
                  <div className="p-3 rounded-xl bg-black/40 border border-white/10 space-y-2">
                    <p className="text-[11px] font-mono text-white/60 leading-relaxed">
                      Podés reutilizar el mismo <strong className="text-white/90">Google OAuth Client</strong> de Orion Creative o crear uno aparte. Solo tenés que agregar esta URI de redirección autorizada en Google Cloud Console:
                    </p>
                    <div className="flex items-center gap-2">
                      <code className="flex-1 text-[11px] font-mono text-aegis-cyan bg-black/60 border border-white/10 rounded-lg px-2.5 py-1.5 break-all">
                        {effectiveRedirectUri}
                      </code>
                      <button
                        type="button"
                        onClick={handleCopyRedirect}
                        className="px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-[11px] font-mono text-white/80 flex items-center gap-1 shrink-0"
                      >
                        {copiedRedirect ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
                        {copiedRedirect ? 'Copiado' : 'Copiar'}
                      </button>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-[10px] font-mono uppercase tracking-wider text-white/40 mb-1">
                        Client ID
                      </label>
                      <input
                        type="text"
                        value={clientIdInput}
                        onChange={(e) => setClientIdInput(e.target.value)}
                        placeholder="1234567890-abc.apps.googleusercontent.com"
                        className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/10 text-xs font-mono text-white placeholder:text-white/25 focus:outline-none focus:border-aegis-cyan/60"
                      />
                    </div>
                    <div>
                      <label className="block text-[10px] font-mono uppercase tracking-wider text-white/40 mb-1">
                        Client Secret {google?.has_client_secret ? '(ya guardado)' : ''}
                      </label>
                      <input
                        type="password"
                        value={clientSecretInput}
                        onChange={(e) => setClientSecretInput(e.target.value)}
                        placeholder={
                          google?.has_client_secret
                            ? 'Guardado — dejá vacío para conservar'
                            : 'GOCSPX-...'
                        }
                        className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/10 text-xs font-mono text-white placeholder:text-white/25 focus:outline-none focus:border-aegis-cyan/60"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-[10px] font-mono uppercase tracking-wider text-white/40 mb-1">
                      Redirect URI personalizada (opcional)
                    </label>
                    <input
                      type="text"
                      value={redirectUriInput}
                      onChange={(e) => setRedirectUriInput(e.target.value)}
                      placeholder={defaultRedirectUri}
                      className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/10 text-xs font-mono text-white placeholder:text-white/25 focus:outline-none focus:border-aegis-cyan/60"
                    />
                  </div>

                  <label className="flex items-start gap-2 text-[11px] font-mono text-white/60 cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={includeExtraScopes}
                      onChange={(e) => setIncludeExtraScopes(e.target.checked)}
                      className="mt-0.5 rounded border-white/20 bg-black/60 text-aegis-cyan focus:ring-aegis-cyan"
                    />
                    <span>
                      Incluir también permisos de Gmail, Drive y YouTube (dejalo desmarcado para Calendar-only si el OAuth Client está en modo prueba).
                    </span>
                  </label>

                  <div className="flex items-center justify-between gap-2 flex-wrap">
                    {google?.client_configured ? (
                      <button
                        type="button"
                        onClick={handleClearGoogleConfig}
                        disabled={savingGoogleConfig}
                        className="px-3 py-1.5 rounded-lg bg-white/5 hover:bg-red-500/10 border border-white/10 hover:border-red-500/30 text-[11px] font-mono text-white/60 hover:text-red-300 transition-colors"
                      >
                        Borrar credenciales guardadas
                      </button>
                    ) : (
                      <span />
                    )}
                    <button
                      type="button"
                      onClick={handleSaveGoogleConfig}
                      disabled={savingGoogleConfig}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-white/20 text-[11px] font-mono font-bold text-white transition-colors disabled:opacity-50"
                    >
                      {savingGoogleConfig && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                      Guardar Client ID / Secret
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Spotify Card */}
            <div className="flex items-center justify-between p-4 bg-white/[0.03] border border-white/10 rounded-xl gap-3">
              <div className="flex items-center gap-3 min-w-0">
                <div className="p-2 rounded-lg bg-white/5 shrink-0">
                  <Music2 className="w-5 h-5 text-green-400" />
                </div>
                <div className="min-w-0">
                  <p className="text-sm font-mono font-bold text-white/90">Spotify</p>
                  <p className="text-[11px] font-mono text-white/40 truncate">
                    {spotify?.connected
                      ? spotify.email || 'Conectado vía Orion Connect'
                      : 'Desconectado (se vincula desde Orion Connect / app móvil)'}
                  </p>
                </div>
              </div>
              {spotify?.connected ? (
                <div className="flex items-center gap-2 shrink-0">
                  <span className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-mono bg-green-500/10 border border-green-500/30 text-green-400">
                    <CheckCircle className="w-3 h-3" />
                    Conectado
                  </span>
                  <button
                    type="button"
                    onClick={() => handleDisconnectOAuth('spotify')}
                    className="p-1.5 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/20 text-red-400 transition-colors"
                    title="Desconectar Spotify"
                  >
                    <Unplug className="w-3.5 h-3.5" />
                  </button>
                </div>
              ) : (
                <span className="px-2 py-0.5 rounded text-[10px] font-mono bg-white/5 border border-white/10 text-white/30 shrink-0">
                  Desconectado
                </span>
              )}
            </div>
          </div>
        )}
      </div>

      {/* ============================================================
          SECTION 2: WHATSAPP PERSONAL (BAILEYS QR + ANTI-SPAM)
      ============================================================ */}
      <div className="space-y-4 pt-2 border-t border-white/10">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-xs font-mono font-bold uppercase tracking-widest text-white/80 flex items-center gap-2">
              <MessageCircle className="w-4 h-4 text-green-400" />
              WhatsApp (Opción A · QR Personal)
            </h3>
            <p className="text-[11px] font-mono text-white/40 mt-1">
              Sesión vinculada como dispositivo secundario con limitador anti-spam estricto y aprobación configurable.
            </p>
          </div>
          <button
            type="button"
            onClick={fetchWaStatus}
            disabled={isLoadingWa}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-[10px] font-mono text-white/60 transition-colors"
          >
            <RefreshCw className={`w-3 h-3 ${isLoadingWa ? 'animate-spin' : ''}`} />
            Actualizar
          </button>
        </div>

        {waBanner && (
          <div
            className={`p-3 rounded-xl border text-xs font-mono flex items-start gap-2 ${
              waBanner.type === 'ok'
                ? 'bg-green-500/10 border-green-500/30 text-green-300'
                : 'bg-red-500/10 border-red-500/30 text-red-300'
            }`}
          >
            {waBanner.type === 'ok' ? (
              <CheckCircle className="w-4 h-4 shrink-0 mt-0.5" />
            ) : (
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            )}
            <span className="flex-1 break-words">{waBanner.text}</span>
          </div>
        )}

        {isLoadingWa && !waStatus ? (
          <div className="flex items-center justify-center py-6">
            <Loader2 className="w-5 h-5 animate-spin text-aegis-cyan" />
          </div>
        ) : (
          <div className="space-y-4">
            {/* Connection & QR Box */}
            <div className="p-4 bg-white/[0.03] border border-white/10 rounded-xl space-y-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-sm font-mono font-bold text-white/90">
                      Estado del enlace
                    </span>
                    <span className={`px-2 py-0.5 rounded text-[10px] font-mono border ${waBadge.cls}`}>
                      {waBadge.label}
                    </span>
                  </div>
                  <p className="text-[11px] font-mono text-white/45 mt-0.5 truncate">
                    {conn?.connected
                      ? `${conn.name || 'Dispositivo'} ${conn.phone ? `(+${conn.phone})` : ''}`.trim()
                      : waStatus?.has_saved_session
                      ? 'Hay credenciales guardadas listas para reconectar'
                      : 'Escaneá el código QR para vincular tu WhatsApp'}
                  </p>
                </div>

                <div className="flex items-center gap-2 flex-wrap">
                  {!conn?.connected && (
                    <button
                      type="button"
                      onClick={handleWaConnect}
                      disabled={waBusy || !waStatus?.bridge_available}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-green-500/90 hover:bg-green-500 text-black font-bold text-[11px] font-mono transition-colors disabled:opacity-40"
                    >
                      {waBusy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <QrCode className="w-3.5 h-3.5" />}
                      {conn?.status === 'qr_ready' ? 'Regenerar QR' : 'Conectar / Ver QR'}
                    </button>
                  )}

                  {(conn?.connected || conn?.status === 'connecting' || conn?.status === 'qr_ready') && (
                    <button
                      type="button"
                      onClick={() => handleWaDisconnect(false)}
                      disabled={waBusy}
                      className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-[11px] font-mono text-white/75 transition-colors disabled:opacity-50"
                    >
                      <Unplug className="w-3.5 h-3.5" />
                      Desconectar
                    </button>
                  )}

                  {(conn?.connected || waStatus?.has_saved_session) && (
                    <button
                      type="button"
                      onClick={() => handleWaDisconnect(true)}
                      disabled={waBusy}
                      className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-[11px] font-mono text-red-400 transition-colors disabled:opacity-50"
                      title="Cerrar sesión en este dispositivo y borrar credenciales"
                    >
                      <LogOut className="w-3.5 h-3.5" />
                      Desvincular
                    </button>
                  )}
                </div>
              </div>

              {!waStatus?.bridge_available && (
                <div className="p-3 rounded-lg bg-amber-500/10 border border-amber-500/30 text-[11px] font-mono text-amber-200">
                  El puente Node.js de WhatsApp (`tools/whatsapp_bridge`) no está activo o le falta `npm install`. Verificá que `node` esté en el PATH y reiniciá Aegis.
                </div>
              )}

              {conn?.last_error && !conn?.connected && (
                <div className="p-2.5 rounded-lg bg-red-500/10 border border-red-500/25 text-[11px] font-mono text-red-300">
                  {conn.last_error}
                </div>
              )}

              {conn?.qr_data_url && !conn.connected && (
                <div className="flex flex-col sm:flex-row items-center gap-4 p-4 rounded-xl bg-black/40 border border-white/10">
                  <div className="bg-white p-2.5 rounded-xl shrink-0">
                    <img
                      src={conn.qr_data_url}
                      alt="Código QR para vincular WhatsApp"
                      className="w-48 h-48 object-contain"
                    />
                  </div>
                  <div className="space-y-1.5 text-[11px] font-mono text-white/65">
                    <p className="text-xs font-bold text-aegis-cyan">Escaneá desde tu teléfono:</p>
                    <p>1. Abrí WhatsApp en tu celular.</p>
                    <p>2. Entrá en <strong>Dispositivos vinculados</strong> &rarr; <strong>Vincular un dispositivo</strong>.</p>
                    <p>3. Apuntá la cámara a este QR (se actualiza automáticamente si expira).</p>
                  </div>
                </div>
              )}
            </div>

            {/* Confirmation Policy Card */}
            <div className="p-4 bg-white/[0.03] border border-white/10 rounded-xl flex items-start justify-between gap-4">
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <Shield className="w-4 h-4 text-aegis-cyan shrink-0" />
                  <span className="text-xs font-mono font-bold text-white/90">
                    Confirmar antes de enviar mensajes
                  </span>
                </div>
                <p className="text-[11px] font-mono text-white/45 leading-relaxed">
                  Si está activado, {assistantDisplayName} crea un borrador cancelable en lugar de enviar de inmediato. También podés cambiarlo desde el chat diciendo <em>&quot;no confirmes, enviá directo&quot;</em> o <em>&quot;volvé a pedir confirmación&quot;</em>.
                </p>
              </div>
              <button
                type="button"
                onClick={handleToggleWaConfirmation}
                disabled={waBusy || !waStatus}
                className={`px-3 py-1.5 rounded-lg border text-[11px] font-mono font-bold shrink-0 transition-colors ${
                  waStatus?.require_confirmation
                    ? 'bg-aegis-cyan/15 border-aegis-cyan/40 text-aegis-cyan hover:bg-aegis-cyan/25'
                    : 'bg-amber-500/15 border-amber-500/40 text-amber-300 hover:bg-amber-500/25'
                }`}
              >
                {waStatus?.require_confirmation ? 'ACTIVO (Con confirmación)' : 'DESACTIVADO (Envío directo)'}
              </button>
            </div>

            {/* Pending Drafts Queue */}
            <div className="p-4 bg-white/[0.03] border border-white/10 rounded-xl space-y-3">
              <div className="flex items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Clock className="w-4 h-4 text-aegis-cyan" />
                  <span className="text-xs font-mono font-bold text-white/90">
                    Mensajes pendientes de confirmación ({pendingList.length})
                  </span>
                </div>
                {pendingList.length > 0 && (
                  <button
                    type="button"
                    onClick={() => handleCancelPending('all')}
                    disabled={pendingActionId === 'all'}
                    className="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-[10px] font-mono text-red-300 transition-colors"
                  >
                    <Trash2 className="w-3 h-3" />
                    Cancelar todos
                  </button>
                )}
              </div>

              {pendingList.length === 0 ? (
                <p className="text-[11px] font-mono text-white/35">
                  No hay borradores pendientes. Cuando {assistantDisplayName} proponga un mensaje con confirmación activa aparecerá acá y en el chat.
                </p>
              ) : (
                <div className="space-y-2.5">
                  {pendingList.map((msg) => {
                    const isWorking = pendingActionId === msg.id;
                    return (
                      <div
                        key={msg.id}
                        className="p-3 rounded-xl bg-black/40 border border-white/10 space-y-2"
                      >
                        <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] font-mono">
                          <div className="text-white/85 font-bold">
                            Para: {msg.recipient_label || msg.to_input}
                            {msg.to_input && msg.recipient_label !== msg.to_input && (
                              <span className="text-white/40 font-normal ml-1.5">({msg.to_input})</span>
                            )}
                          </div>
                          <span className="text-[10px] text-white/35">
                            {msg.id} · expira {formatEpochTime(msg.expires_at_epoch)}
                          </span>
                        </div>
                        <p className="text-xs font-mono text-white/75 whitespace-pre-wrap bg-white/[0.02] p-2.5 rounded-lg border border-white/5">
                          {msg.text}
                        </p>
                        <div className="flex items-center justify-end gap-2">
                          <button
                            type="button"
                            onClick={() => handleCancelPending(msg.id)}
                            disabled={isWorking}
                            className="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-[11px] font-mono text-red-300 transition-colors disabled:opacity-50"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                            Cancelar
                          </button>
                          <button
                            type="button"
                            onClick={() => handleConfirmPending(msg.id)}
                            disabled={isWorking || !conn?.connected}
                            className="flex items-center gap-1 px-3 py-1 rounded-lg bg-green-500/90 hover:bg-green-500 text-black font-bold text-[11px] font-mono transition-colors disabled:opacity-40"
                          >
                            {isWorking ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Send className="w-3.5 h-3.5" />}
                            Enviar ahora
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Anti-Spam Rate Limiter */}
            <div className="p-4 bg-white/[0.03] border border-white/10 rounded-xl space-y-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <Sliders className="w-4 h-4 text-aegis-cyan" />
                  <span className="text-xs font-mono font-bold text-white/90">
                    Limitador Anti-Spam (Protección anti-bloqueo)
                  </span>
                </div>
                <button
                  type="button"
                  onClick={() => setShowRateConfig((v) => !v)}
                  className="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-[10px] font-mono text-white/70"
                >
                  Configurar límites
                  {showRateConfig ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                </button>
              </div>

              {usage && (
                <div className="grid grid-cols-3 gap-2 text-center">
                  <div className="p-2 rounded-lg bg-black/35 border border-white/5">
                    <div className="text-[10px] font-mono text-white/40 uppercase">Último min</div>
                    <div className="text-xs font-mono font-bold text-white/85 mt-0.5">
                      {usage.sent_last_minute} / {usage.config.max_per_minute}
                    </div>
                  </div>
                  <div className="p-2 rounded-lg bg-black/35 border border-white/5">
                    <div className="text-[10px] font-mono text-white/40 uppercase">Última hora</div>
                    <div className="text-xs font-mono font-bold text-white/85 mt-0.5">
                      {usage.sent_last_hour} / {usage.config.max_per_hour}
                    </div>
                  </div>
                  <div className="p-2 rounded-lg bg-black/35 border border-white/5">
                    <div className="text-[10px] font-mono text-white/40 uppercase">Últimas 24h</div>
                    <div className="text-xs font-mono font-bold text-white/85 mt-0.5">
                      {usage.sent_last_day} / {usage.config.max_per_day}
                    </div>
                  </div>
                </div>
              )}

              {showRateConfig && (
                <div className="pt-3 border-t border-white/10 space-y-3">
                  <p className="text-[10px] font-mono text-white/40 leading-relaxed">
                    El kernel bloquea automáticamente ráfagas, envíos duplicados en cadena y prohíbe enviar mensajes desde agentes en segundo plano.
                  </p>
                  <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                    {(
                      [
                        ['min_interval_secs', 'Pausa mínima (seg)', 1, 300],
                        ['max_per_minute', 'Máx. por minuto', 1, 15],
                        ['max_per_hour', 'Máx. por hora', 1, 100],
                        ['max_per_day', 'Máx. por día', 1, 500],
                        ['max_per_recipient_10m', 'Máx. contacto (10m)', 1, 15],
                        ['dedup_window_secs', 'Antiduplicado (seg)', 0, 3600],
                      ] as const
                    ).map(([field, label, min, max]) => (
                      <div key={field}>
                        <label className="block text-[10px] font-mono text-white/45 mb-1">
                          {label}
                        </label>
                        <input
                          type="number"
                          min={min}
                          max={max}
                          value={rateDraft[field]}
                          onChange={(e) => {
                            rateDraftDirtyRef.current = true;
                            const val = Number(e.target.value);
                            setRateDraft((prev) => ({
                              ...prev,
                              [field]: Number.isFinite(val) ? val : min,
                            }));
                          }}
                          className="w-full px-2.5 py-1.5 rounded-lg bg-black/50 border border-white/10 text-xs font-mono text-white focus:outline-none focus:border-aegis-cyan/60"
                        />
                      </div>
                    ))}
                  </div>
                  <div className="flex justify-end">
                    <button
                      type="button"
                      onClick={handleSaveRateConfig}
                      disabled={waBusy}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-white/20 text-[11px] font-mono font-bold text-white transition-colors disabled:opacity-50"
                    >
                      {waBusy && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                      Guardar límites anti-spam
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default ConnectedAccountsTab;
