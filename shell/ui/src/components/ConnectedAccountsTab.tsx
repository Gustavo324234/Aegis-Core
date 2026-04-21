import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { Chrome, Music2, CheckCircle, Unplug, Smartphone, Loader2 } from 'lucide-react';

interface OAuthStatus {
  connected: boolean;
  scope?: string;
  email?: string;
}

const PROVIDERS = [
  {
    id: 'google',
    name: 'Google',
    icon: Chrome,
    color: 'text-blue-400',
    borderColor: 'border-blue-500/30',
    bgColor: 'bg-blue-500/10',
    description: 'YouTube Music · Calendar · Drive · Gmail',
  },
  {
    id: 'spotify',
    name: 'Spotify',
    icon: Music2,
    color: 'text-green-400',
    borderColor: 'border-green-500/30',
    bgColor: 'bg-green-500/10',
    description: 'Reproducción de música · Playlists',
  },
];

const ConnectedAccountsTab: React.FC<{
  tenantId: string;
  sessionKey: string;
}> = ({ tenantId, sessionKey }) => {
  const [status, setStatus] = useState<Record<string, OAuthStatus>>({});
  const [isLoading, setIsLoading] = useState(true);

  const headers = useMemo(() => ({
    'x-citadel-tenant': tenantId,
    'x-citadel-key': sessionKey,
  }), [tenantId, sessionKey]);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch('/api/oauth/status', { headers });
      if (res.ok) setStatus(await res.json());
    } catch { /* best-effort */ }
    finally { setIsLoading(false); }
  }, [headers]);

  useEffect(() => { fetchStatus(); }, [fetchStatus]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleDisconnect = async (provider: string) => {
    await fetch(`/api/oauth/${provider}`, { method: 'DELETE', headers });
    await fetchStatus();
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="w-5 h-5 text-aegis-cyan animate-spin" />
      </div>
    );
  }

  const hasAnyConnected = Object.values(status).some(s => s.connected);

  return (
    <div className="space-y-4 max-w-2xl">
      <div className="glass p-5 rounded-2xl border border-white/10">
        <h2 className="text-sm font-mono font-bold tracking-widest uppercase mb-1">
          Cuentas Conectadas
        </h2>
        <p className="text-[10px] font-mono text-white/40 leading-relaxed">
          Conectá tus cuentas desde la app Aegis en tu teléfono para que el agente
          pueda reproducir música, ver tu calendario, Drive y más.
        </p>
      </div>

      {!hasAnyConnected && (
        <div className="glass p-5 rounded-2xl border border-aegis-cyan/20 bg-aegis-cyan/5 flex items-start gap-3">
          <Smartphone className="w-5 h-5 text-aegis-cyan mt-0.5 shrink-0" />
          <div>
            <p className="text-xs font-mono text-aegis-cyan font-bold uppercase tracking-widest mb-1">
              Conectar desde la app
            </p>
            <p className="text-[10px] font-mono text-white/50 leading-relaxed">
              Abrí la app Aegis en tu teléfono → Settings → Cuentas Conectadas →
              tocá "Conectar Google" o "Conectar Spotify". Una vez autorizado,
              aparecerá aquí automáticamente.
            </p>
          </div>
        </div>
      )}

      {PROVIDERS.map((provider) => {
        const s = status[provider.id];
        const isConnected = s?.connected ?? false;
        const Icon = provider.icon;

        return (
          <div
            key={provider.id}
            className={`glass p-5 rounded-2xl border transition-colors
              ${isConnected ? provider.borderColor : 'border-white/10'}`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className={`p-2 rounded-xl ${isConnected ? provider.bgColor : 'bg-white/5'}`}>
                  <Icon className={`w-5 h-5 ${isConnected ? provider.color : 'text-white/30'}`} />
                </div>
                <div>
                  <p className="text-sm font-mono font-bold">{provider.name}</p>
                  <p className="text-[10px] font-mono text-white/40 mt-0.5">
                    {provider.description}
                  </p>
                  {isConnected && s?.email && (
                    <p className={`text-[10px] font-mono mt-1 font-semibold ${provider.color}`}>
                      {s.email}
                    </p>
                  )}
                </div>
              </div>

              {isConnected ? (
                <div className="flex items-center gap-3">
                  <span className="flex items-center gap-1 text-[9px] font-mono text-green-400 uppercase">
                    <CheckCircle className="w-3 h-3" /> Conectado
                  </span>
                  <button
                    onClick={() => handleDisconnect(provider.id)}
                    className="p-1.5 rounded hover:bg-red-500/10 text-white/20 hover:text-red-400 transition-colors"
                    title="Desconectar"
                  >
                    <Unplug className="w-3.5 h-3.5" />
                  </button>
                </div>
              ) : (
                <span className="text-[9px] font-mono text-white/20 uppercase">
                  No conectado
                </span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};

export default ConnectedAccountsTab;
