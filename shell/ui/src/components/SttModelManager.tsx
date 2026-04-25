import React, { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Download, CheckCircle, Cpu, HardDrive, Star, AlertTriangle, Loader2 } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

interface WhisperModel {
    id: string;
    label: string;
    size_mb: number;
    ram_mb: number;
    recommended: boolean;
    description: string;
}

interface DownloadStatus {
    downloading: boolean;
    progress: number;
    current_model: string | null;
    error: string | null;
}

function formatMb(mb: number): string {
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
    return `${mb} MB`;
}

const SttModelManager: React.FC<{ activeModel: string | null; onModelActivated: () => void }> = ({
    activeModel,
    onModelActivated,
}) => {
    const { t } = useTranslation();
    const { tenantId, sessionKey } = useAegisStore();
    const [models, setModels] = useState<WhisperModel[]>([]);
    const [status, setStatus] = useState<DownloadStatus>({
        downloading: false,
        progress: 0,
        current_model: null,
        error: null,
    });
    const [justCompleted, setJustCompleted] = useState<string | null>(null);
    const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const authHeaders = {
        'x-citadel-tenant': tenantId ?? '',
        'x-citadel-key': sessionKey ?? '',
    };

    const fetchModels = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/siren/stt/models', { headers: authHeaders });
            if (res.ok) {
                const data = await res.json();
                setModels(data.models ?? []);
            }
        } catch (_) {}
    }, [tenantId, sessionKey]);

    const fetchStatus = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/siren/stt/status', { headers: authHeaders });
            if (res.ok) {
                const data: DownloadStatus = await res.json();
                setStatus(data);

                if (!data.downloading && data.progress >= 1 && data.current_model) {
                    setJustCompleted(data.current_model);
                    onModelActivated();
                    if (pollRef.current) {
                        clearInterval(pollRef.current);
                        pollRef.current = null;
                    }
                }
            }
        } catch (_) {}
    }, [tenantId, sessionKey, onModelActivated]);

    useEffect(() => {
        fetchModels();
        fetchStatus();
    }, [fetchModels, fetchStatus]);

    // Poll while downloading
    useEffect(() => {
        if (status.downloading && !pollRef.current) {
            pollRef.current = setInterval(fetchStatus, 1000);
        }
        if (!status.downloading && pollRef.current) {
            clearInterval(pollRef.current);
            pollRef.current = null;
        }
        return () => {
            if (pollRef.current) clearInterval(pollRef.current);
        };
    }, [status.downloading, fetchStatus]);

    const handleDownload = async (modelId: string) => {
        if (!tenantId || !sessionKey) return;
        setJustCompleted(null);
        try {
            const res = await fetch('/api/siren/stt/download', {
                method: 'POST',
                headers: { ...authHeaders, 'Content-Type': 'application/json' },
                body: JSON.stringify({ model: modelId }),
            });
            if (res.ok) {
                setStatus({ downloading: true, progress: 0, current_model: modelId, error: null });
            }
        } catch (_) {}
    };

    if (models.length === 0) return null;

    return (
        <div className="space-y-4">
            <div className="flex items-start gap-3 mb-2">
                <Cpu className="w-4 h-4 text-aegis-cyan mt-0.5 flex-shrink-0" />
                <div>
                    <p className="text-[10px] font-mono font-bold uppercase tracking-widest text-white/60">
                        {t('stt_model_manager_title')}
                    </p>
                    <p className="text-[9px] font-mono text-white/30 mt-0.5">
                        {t('stt_model_manager_subtitle')}
                    </p>
                </div>
            </div>

            <div className="grid grid-cols-1 gap-3">
                {models.map((model) => {
                    const isActive = activeModel === model.id || justCompleted === model.id;
                    const isDownloading = status.downloading && status.current_model === model.id;
                    const progress = isDownloading ? status.progress : 0;

                    return (
                        <motion.div
                            key={model.id}
                            initial={{ opacity: 0, y: 4 }}
                            animate={{ opacity: 1, y: 0 }}
                            className={`relative rounded-xl border p-4 transition-all duration-300 overflow-hidden ${
                                isActive
                                    ? 'bg-aegis-cyan/10 border-aegis-cyan/50'
                                    : 'bg-white/3 border-white/8 hover:border-white/15'
                            }`}
                        >
                            {/* Progress fill bar */}
                            {isDownloading && (
                                <div
                                    className="absolute inset-0 bg-aegis-cyan/5 transition-all duration-500 ease-out"
                                    style={{ width: `${progress * 100}%` }}
                                />
                            )}

                            <div className="relative z-10 flex items-center justify-between gap-4">
                                {/* Left: info */}
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-2 flex-wrap">
                                        <span className="text-xs font-mono font-bold uppercase tracking-wider text-white">
                                            {model.label}
                                        </span>
                                        {model.recommended && (
                                            <span className="flex items-center gap-1 text-[8px] font-mono uppercase tracking-wider text-amber-400 bg-amber-400/10 border border-amber-400/30 rounded-full px-2 py-0.5">
                                                <Star className="w-2.5 h-2.5" />
                                                {t('stt_model_recommended')}
                                            </span>
                                        )}
                                        {isActive && (
                                            <span className="flex items-center gap-1 text-[8px] font-mono uppercase tracking-wider text-aegis-cyan bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-full px-2 py-0.5">
                                                <CheckCircle className="w-2.5 h-2.5" />
                                                {t('stt_model_active')}
                                            </span>
                                        )}
                                    </div>

                                    <p className="text-[9px] font-mono text-white/35 mt-1 leading-snug">
                                        {model.description}
                                    </p>

                                    <div className="flex items-center gap-4 mt-2">
                                        <span className="flex items-center gap-1 text-[9px] font-mono text-white/40">
                                            <HardDrive className="w-2.5 h-2.5" />
                                            {t('stt_model_disk')}: {formatMb(model.size_mb)}
                                        </span>
                                        <span className="flex items-center gap-1 text-[9px] font-mono text-white/40">
                                            <Cpu className="w-2.5 h-2.5" />
                                            {t('stt_model_ram')}: ~{formatMb(model.ram_mb)}
                                        </span>
                                    </div>

                                    {/* Progress bar */}
                                    {isDownloading && (
                                        <div className="mt-2">
                                            <div className="w-full h-1 bg-white/10 rounded-full overflow-hidden">
                                                <motion.div
                                                    className="h-full bg-aegis-cyan rounded-full"
                                                    initial={{ width: 0 }}
                                                    animate={{ width: `${progress * 100}%` }}
                                                    transition={{ ease: 'linear', duration: 0.5 }}
                                                />
                                            </div>
                                            <p className="text-[9px] font-mono text-aegis-cyan/70 mt-1">
                                                {Math.round(progress * 100)}%
                                            </p>
                                        </div>
                                    )}
                                </div>

                                {/* Right: button */}
                                {!isActive && (
                                    <button
                                        type="button"
                                        disabled={status.downloading}
                                        onClick={() => handleDownload(model.id)}
                                        className={`flex-shrink-0 flex items-center gap-2 px-4 py-2 rounded-lg text-[9px] font-mono uppercase tracking-wider transition-all duration-300 border ${
                                            isDownloading
                                                ? 'border-aegis-cyan/40 text-aegis-cyan bg-aegis-cyan/10 cursor-wait'
                                                : status.downloading
                                                ? 'border-white/10 text-white/20 cursor-not-allowed'
                                                : 'border-white/20 text-white/60 hover:border-aegis-cyan/50 hover:text-aegis-cyan hover:bg-aegis-cyan/5'
                                        }`}
                                    >
                                        {isDownloading ? (
                                            <Loader2 className="w-3 h-3 animate-spin" />
                                        ) : (
                                            <Download className="w-3 h-3" />
                                        )}
                                        {isDownloading ? t('stt_model_downloading') : t('stt_model_download')}
                                    </button>
                                )}
                            </div>
                        </motion.div>
                    );
                })}
            </div>

            <AnimatePresence>
                {status.error && (
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-lg"
                    >
                        <AlertTriangle className="w-3.5 h-3.5 text-red-400 flex-shrink-0" />
                        <span className="text-[9px] font-mono text-red-400">{status.error}</span>
                    </motion.div>
                )}
                {justCompleted && !status.downloading && (
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        className="flex items-center gap-2 p-3 bg-green-500/10 border border-green-500/30 rounded-lg"
                    >
                        <CheckCircle className="w-3.5 h-3.5 text-green-400 flex-shrink-0" />
                        <span className="text-[9px] font-mono text-green-400 uppercase tracking-widest">
                            {t('stt_download_complete')}
                        </span>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default SttModelManager;
