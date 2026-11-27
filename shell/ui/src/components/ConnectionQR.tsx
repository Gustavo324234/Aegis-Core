import React, { useState, useEffect, useCallback } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { X, RefreshCw, Smartphone, Globe, Shield, Download } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore } from '../store/useAegisStore';
import { ApiError, apiGet } from '../utils/apiClient';

const ANDROID_APK_NIGHTLY_URL =
  'https://github.com/Gustavo324234/Aegis-Core/releases/download/nightly/aegis-android.apk';

interface ConnectionInfo {
  local_url: string;
  tunnel_url: string | null;
  tunnel_status: 'active' | 'connecting' | 'disabled';
  qr_url: string;
}

export const ConnectionQR: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { tenantId, sessionKey } = useAegisStore(
    useShallow((s) => ({ tenantId: s.tenantId, sessionKey: s.sessionKey })),
  );
  const [activeTab, setActiveTab] = useState<'link' | 'download'>('link');
  const [info, setInfo] = useState<ConnectionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchInfo = useCallback(
    async (signal?: AbortSignal) => {
      try {
        const data = await apiGet<ConnectionInfo>('/api/system/connection-info', {
          auth: tenantId && sessionKey ? { tenantId, sessionKey } : false,
          signal,
        });
        setInfo(data);
        setError(null);
      } catch (err) {
        if (err instanceof ApiError && err.code === 'ABORTED') return;
        setError(err instanceof Error ? err.message : 'Unknown error');
      } finally {
        if (!signal?.aborted) {
          setLoading(false);
        }
      }
    },
    [tenantId, sessionKey],
  );

  useEffect(() => {
    const controller = new AbortController();
    void fetchInfo(controller.signal);
    const timer = setInterval(() => {
      void fetchInfo(controller.signal);
    }, 30000);
    return () => {
      controller.abort();
      clearInterval(timer);
    };
  }, [fetchInfo]);

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[100] p-4">
      <div className="bg-[#111] border border-white/10 rounded-2xl w-full max-w-sm overflow-hidden shadow-2xl animate-in fade-in zoom-in duration-200">
        <div className="p-4 border-b border-white/5 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Smartphone className="w-5 h-5 text-blue-400" />
            <h2 className="font-semibold text-white">Conectar App Móvil</h2>
          </div>
          <button 
            onClick={onClose}
            className="p-1.5 hover:bg-white/10 rounded-lg transition-colors text-white/50 hover:text-white"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="px-6 pt-4">
          <div className="grid grid-cols-2 gap-1 bg-white/5 p-1 rounded-xl border border-white/10">
            <button
              type="button"
              onClick={() => setActiveTab('link')}
              className={`py-2 px-3 rounded-lg text-xs font-medium transition-colors ${
                activeTab === 'link'
                  ? 'bg-blue-500/20 text-blue-300 border border-blue-500/30'
                  : 'text-white/50 hover:text-white'
              }`}
            >
              1. Vincular App
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('download')}
              className={`py-2 px-3 rounded-lg text-xs font-medium transition-colors flex items-center justify-center gap-1.5 ${
                activeTab === 'download'
                  ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/30'
                  : 'text-white/50 hover:text-white'
              }`}
            >
              <Download className="w-3.5 h-3.5" /> 2. Bajar APK
            </button>
          </div>
        </div>

        <div className="p-8 flex flex-col items-center gap-6">
          {activeTab === 'download' ? (
            <>
              <div className="p-4 bg-white rounded-2xl shadow-inner">
                <QRCodeSVG
                  value={ANDROID_APK_NIGHTLY_URL}
                  size={192}
                  level="H"
                  includeMargin={false}
                />
              </div>
              <div className="w-full space-y-3">
                <a
                  href={ANDROID_APK_NIGHTLY_URL}
                  target="_blank"
                  rel="noreferrer"
                  className="w-full py-2.5 px-4 rounded-xl bg-emerald-500/20 hover:bg-emerald-500/30 border border-emerald-500/40 text-emerald-300 text-xs font-semibold flex items-center justify-center gap-2 transition-colors"
                >
                  <Download className="w-4 h-4" />
                  Descargar aegis-android.apk
                </a>
                <div className="p-3 bg-white/5 rounded-xl border border-white/10">
                  <p className="text-[11px] text-white/65 leading-snug">
                    Escaneá este QR desde tu teléfono Android para instalar Aegis. Las futuras versiones se actualizan directo desde <strong className="text-white">Ajustes → Buscar Actualizaciones</strong> en la app.
                  </p>
                </div>
              </div>
            </>
          ) : loading && !info ? (
            <div className="w-48 h-48 bg-white/5 rounded-xl flex items-center justify-center animate-pulse">
              <RefreshCw className="w-8 h-8 text-white/20 animate-spin" />
            </div>
          ) : error ? (
            <div className="text-center p-4 bg-red-500/10 border border-red-500/20 rounded-xl text-red-400 text-sm">
              {error}
            </div>
          ) : info && (
            <>
              <div className="p-4 bg-white rounded-2xl shadow-inner">
                <QRCodeSVG 
                  value={info.qr_url} 
                  size={192}
                  level="H"
                  includeMargin={false}
                />
              </div>

              <div className="w-full space-y-3">
                <div className="flex items-center justify-between p-3 bg-white/5 rounded-xl border border-white/5">
                  <div className="flex items-center gap-2">
                    <Globe className="w-4 h-4 text-white/40" />
                    <span className="text-sm text-white/70">Acceso Remoto</span>
                  </div>
                  {info.tunnel_status === 'active' ? (
                    <span className="text-[10px] font-bold uppercase tracking-wider bg-green-500/20 text-green-400 px-2 py-1 rounded-full border border-green-500/20">
                      Activo ✓
                    </span>
                  ) : info.tunnel_status === 'connecting' ? (
                    <span className="text-[10px] font-bold uppercase tracking-wider bg-yellow-500/20 text-yellow-400 px-2 py-1 rounded-full border border-yellow-500/20 flex items-center gap-1">
                      <RefreshCw className="w-2.5 h-2.5 animate-spin" />
                      Activando...
                    </span>
                  ) : (
                    <span className="text-[10px] font-bold uppercase tracking-wider bg-white/10 text-white/40 px-2 py-1 rounded-full">
                      Solo LAN
                    </span>
                  )}
                </div>

                <div className="flex items-center gap-2 p-3 bg-blue-500/10 rounded-xl border border-blue-500/20">
                  <Shield className="w-4 h-4 text-blue-400 shrink-0" />
                  <p className="text-[11px] text-blue-300/80 leading-snug">
                    Escanea este código desde la app de Aegis para sincronizar tu cuenta automáticamente.
                  </p>
                </div>
              </div>
            </>
          )}
        </div>

        <div className="p-4 bg-white/5 border-t border-white/5">
          <p className="text-[10px] text-center text-white/30 uppercase tracking-widest font-medium">
            Aegis Core Secure Link
          </p>
        </div>
      </div>
    </div>
  );
};
