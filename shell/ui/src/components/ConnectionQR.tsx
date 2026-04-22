import React, { useState, useEffect } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { X, RefreshCw, Smartphone, Globe, Shield } from 'lucide-react';

interface ConnectionInfo {
  local_url: string;
  tunnel_url: string | null;
  tunnel_status: 'active' | 'connecting' | 'disabled';
  qr_url: string;
}

export const ConnectionQR: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [info, setInfo] = useState<ConnectionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchInfo = async () => {
    try {
      const res = await fetch('/api/system/connection-info');
      if (!res.ok) throw new Error('Failed to fetch connection info');
      const data = await res.json();
      setInfo(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchInfo();
    const timer = setInterval(fetchInfo, 30000);
    return () => clearInterval(timer);
  }, []);

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

        <div className="p-8 flex flex-col items-center gap-6">
          {loading && !info ? (
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
