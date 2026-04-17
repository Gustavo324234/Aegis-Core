import React, { useState, useEffect, useCallback } from 'react';
import { motion } from 'framer-motion';
import { Mic, Shield, Check, Terminal, Eye, EyeOff, Save, RefreshCw, Volume2, AlertTriangle } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

const SIREN_PROVIDERS = (t: (key: string) => string) => [
    { id: 'voxtral', label: 'Voxtral Local', desc: t('voxtral_desc'), icon: <Mic className="w-5 h-5 text-aegis-cyan" /> },
    { id: 'elevenlabs', label: 'ElevenLabs Cloud', desc: t('elevenlabs_desc'), icon: <Shield className="w-5 h-5 text-aegis-purple" /> },
    { id: 'fish', label: 'Fish Speech', desc: t('fish_desc'), icon: <Volume2 className="w-5 h-5 text-green-400" /> },
];

interface Voice {
    id: string;
    name: string;
    provider: string;
}

const SirenConfigTab: React.FC = () => {
    const { t } = useTranslation();
    const { tenantId, sessionKey, sttAvailable } = useAegisStore();
    const providersList = SIREN_PROVIDERS(t);
    const [provider, setProvider] = useState('voxtral');
    const [apiKey, setApiKey] = useState('');
    const [voiceId, setVoiceId] = useState('');
    const [voices, setVoices] = useState<Voice[]>([]);
    const [showKey, setShowKey] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    const fetchConfig = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/siren/config', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                const data = await res.json();
                setProvider(data.provider || 'voxtral');
                setApiKey(data.api_key || '');
                setVoiceId(data.voice_id || '');
            }
        } catch (err) {
            console.error('Fetch config error:', err);
        } finally {
            setIsLoading(false);
        }
    }, [tenantId, sessionKey]);

    const fetchVoices = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/siren/voices', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                const data = await res.json();
                setVoices(data.voices || []);
            }
        } catch (err) {
            console.error('Fetch voices error:', err);
        }
    }, [tenantId, sessionKey]);

    useEffect(() => {
        fetchConfig();
        fetchVoices();
    }, [fetchConfig, fetchVoices]);

    const handleSave = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!tenantId || !sessionKey) return;
        setIsSaving(true);
        setError(null);
        setSuccess(false);
        try {
            const response = await fetch('/api/siren/config', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ provider, api_key: apiKey, voice_id: voiceId })
            });
            if (response.ok) {
                setSuccess(true);
                setTimeout(() => setSuccess(false), 3000);
            } else {
                const errData = await response.json();
                setError(errData.detail || t('voice_config_save_error'));
            }
        } catch (err) {
            setError(t('bff_communication_error'));
        } finally {
            setIsSaving(false);
        }
    };

    if (isLoading) return <div className="flex justify-center p-12 animate-pulse font-mono text-white/30 text-xs uppercase tracking-widest">{t('syncing_siren_protocol')}</div>;

    return (
        <div className="space-y-8 max-h-[70vh] overflow-y-auto pr-4 custom-scrollbar">
            <div className="glass p-8 rounded-2xl border border-white/10 shadow-2xl relative overflow-hidden">
                <div className="absolute top-0 right-0 p-4 opacity-10">
                    <Mic className="w-24 h-24 text-aegis-cyan" />
                </div>
                
                <div className="flex items-center gap-4 mb-8 relative z-10">
                    <div className="p-3 rounded-xl bg-aegis-cyan/10 border border-aegis-cyan/30">
                        <Volume2 className="w-6 h-6 text-aegis-cyan" />
                    </div>
                    <div>
                        <h2 className="text-xl font-bold tracking-[0.2em] uppercase text-white">{t('voice_config_siren')}</h2>
                        <p className="text-[10px] font-mono text-white/30 uppercase tracking-[0.2em] mt-1">{t('neural_synth_control')}</p>
                    </div>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8 relative z-10">
                    {providersList.map((p) => (
                        <button key={p.id} type="button" onClick={() => setProvider(p.id)} className={`p-6 rounded-2xl border transition-all duration-500 text-left group flex flex-col gap-3 ${provider === p.id ? 'bg-aegis-cyan/20 border-aegis-cyan shadow-[0_0_30px_rgba(0,186,211,0.1)]' : 'bg-white/5 border-white/10 hover:bg-white/10'}`}>
                            <div className="p-2 w-fit rounded-lg bg-black/30 border border-white/5">{p.icon}</div>
                            <div>
                                <h3 className="text-xs font-mono font-bold tracking-widest uppercase">{p.label}</h3>
                                <p className="text-[9px] font-mono text-white/30 uppercase mt-1 leading-tight">{p.desc}</p>
                            </div>
                        </button>
                    ))}
                </div>

                <form onSubmit={handleSave} className="space-y-6 relative z-10">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                        {provider !== 'voxtral' && (
                            <div className="space-y-2">
                                <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">API Key</label>
                                <div className="relative">
                                    <input type={showKey ? 'text' : 'password'} value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder={t('provider_key_placeholder')} className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 pr-10 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10" required={provider !== 'voxtral'} />
                                    <button type="button" onClick={() => setShowKey(!showKey)} className="absolute right-3 top-1/2 -translate-y-1/2 text-white/20 hover:text-white/50 transition-colors">
                                        {showKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                    </button>
                                </div>
                            </div>
                        )}
                        <div className="space-y-2 col-span-1">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('default_voice')}</label>
                            <select value={voiceId} onChange={(e) => setVoiceId(e.target.value)} className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all appearance-none cursor-pointer">
                                <option value="">{t('auto_default')}</option>
                                {voices.filter(v => v.provider === provider).map(v => (
                                    <option key={v.id} value={v.id}>{v.name}</option>
                                ))}
                            </select>
                        </div>
                    </div>

                    {error && (
                        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-3">
                            <Terminal className="w-4 h-4 text-red-500 flex-shrink-0" />
                            <span className="text-[10px] font-mono text-red-400 leading-tight">{error}</span>
                        </motion.div>
                    )}
                    {success && (
                        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="bg-green-500/10 border border-green-500/30 p-3 rounded-lg flex items-center gap-3">
                            <Check className="w-4 h-4 text-green-500" />
                            <span className="text-[10px] font-mono text-green-400 leading-tight uppercase tracking-widest">{t('siren_sync_success')}</span>
                        </motion.div>
                    )}

                    <button type="submit" disabled={isSaving} className={`w-full group relative overflow-hidden rounded-xl py-5 transition-all duration-700 ${isSaving ? "bg-white/10 cursor-wait" : "bg-aegis-cyan/10 hover:bg-aegis-cyan/25 border border-aegis-cyan/40 hover:shadow-[0_0_40px_rgba(0,186,211,0.2)]"}`}>
                        <div className="relative z-10 flex items-center justify-center gap-4">
                            {isSaving ? <RefreshCw className="w-4 h-4 text-aegis-cyan animate-spin" /> : <Save className="w-4 h-4 text-aegis-cyan" />}
                            <span className="text-sm font-mono font-bold tracking-[0.4em] uppercase text-aegis-cyan">{isSaving ? "SYNCING..." : t('save_config')}</span>
                        </div>
                    </button>
                </form>
            </div>
            
            <div className="glass p-6 rounded-2xl border border-white/5 bg-white/2 shadow-xl">
                <h3 className="text-xs font-mono font-bold tracking-widest uppercase text-white/40 flex items-center gap-2 mb-4">
                    <Terminal className="w-3.5 h-3.5 text-aegis-cyan" /> {t('technical_info')}
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-[10px] font-mono uppercase tracking-widest text-white/30">
                    <div className="flex justify-between border-b border-white/5 pb-2">
                        <span>{t('sample_rate')}</span>
                        <span className="text-white/60">16kHz (Standard)</span>
                    </div>
                    <div className="flex justify-between border-b border-white/5 pb-2">
                        <span>{t('output_format')}</span>
                        <span className="text-white/60">PCM 16-bit Mono</span>
                    </div>
                    <div className="flex justify-between border-b border-white/5 pb-2">
                        <span>{t('bridge_protocol')}</span>
                        <span className="text-white/60">gRPC Bi-Stream (Siren)</span>
                    </div>
                    <div className="flex justify-between border-b border-white/5 pb-2">
                        <span>{t('estimated_latency')}</span>
                        <span className="text-white/60">~120ms (LAN)</span>
                    </div>
                </div>
                {!sttAvailable && (
                    <motion.div 
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        className="mt-4 p-4 bg-amber-500/10 border border-amber-500/30 rounded-lg"
                    >
                        <div className="flex items-start gap-3">
                            <AlertTriangle className="w-4 h-4 text-amber-500 flex-shrink-0 mt-0.5" />
                            <div className="space-y-2">
                                <p className="text-[10px] font-mono text-amber-400 uppercase tracking-widest">{t('stt_not_available')}</p>
                                <p className="text-[9px] font-mono text-white/40">{t('stt_unavailable_instructions')}</p>
                                <code className="block mt-2 p-2 bg-black/40 rounded text-[9px] font-mono text-white/60">
                                    mkdir -p $AEGIS_DATA_DIR/models &amp;&amp; wget -O $AEGIS_DATA_DIR/models/ggml-base.bin &lt;url&gt;
                                </code>
                            </div>
                        </div>
                    </motion.div>
                )}
                {sttAvailable && (
                    <div className="mt-4 flex items-center gap-2 text-[10px] font-mono text-green-400/60">
                        <Check className="w-3 h-3" />
                        <span>{t('stt_available')}</span>
                    </div>
                )}
            </div>
        </div>
    );
};

export default SirenConfigTab;
