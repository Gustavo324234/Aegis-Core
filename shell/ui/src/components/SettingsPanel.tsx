import React, { useState, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Settings, X, Save, Cpu, Volume2, Globe, Check, Eye, EyeOff, Loader2, AlertTriangle, Sparkles, Shield, Server, Zap, Box, Cloud, Terminal, Activity, Info, Search, RefreshCw, Copy, FileText } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import { PROVIDER_PRESETS, ProviderType } from '../constants/enginePresets';
import TenantKeyManager from './RouterConfig/TenantKeyManager';
import UserPasswordChange from './UserPasswordChange';
import ConnectedAccountsTab from './ConnectedAccountsTab';

interface SettingsPanelProps {
    onClose: () => void;
    tenantId: string;
    sessionKey: string;
}

type TabId = 'perfil' | 'motor' | 'voz' | 'logs' | 'cuenta';

const TABS = (t: (key: string) => string): { id: TabId; label: string; icon: React.ReactNode }[] => [
    { id: 'perfil', label: t('tab_persona'), icon: <Sparkles className="w-4 h-4" /> },
    { id: 'motor', label: t('tab_motor'), icon: <Cpu className="w-4 h-4" /> },
    { id: 'voz', label: t('tab_voz'), icon: <Volume2 className="w-4 h-4" /> },
    { id: 'logs', label: t('tab_logs'), icon: <Terminal className="w-4 h-4" /> },
    { id: 'cuenta', label: 'Cuenta', icon: <Globe className="w-4 h-4" /> },
];

interface PersonaData {
    prompt: string;
    name: string;
}

const NAME_PREFIX = 'Tu nombre es ';

function parsePersonaString(raw: string): PersonaData {
    const start = raw.indexOf(NAME_PREFIX);
    if (start === -1) return { name: '', prompt: raw };
    const after = raw.slice(start + NAME_PREFIX.length);
    const dotIdx = after.indexOf('.');
    if (dotIdx === -1) return { name: after.trim(), prompt: '' };
    const name = after.slice(0, dotIdx).trim();
    const prompt = after.slice(dotIdx + 1).trim();
    return { name, prompt };
}

function composePersonaString(data: PersonaData): string {
    const n = data.name.trim();
    const p = data.prompt.trim();
    if (!n) return p;
    return p ? `${NAME_PREFIX}${n}. ${p}` : `${NAME_PREFIX}${n}.`;
}

interface SirenConfig {
    provider: string;
    api_key?: string;
    voice_id: string;
    stt_available?: boolean;
    stt_provider?: string;
    stt_api_key?: string;
}

interface Voice {
    id: string;
    name: string;
    provider: string;
}

const PersonaTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [persona, setPersona] = useState<PersonaData>({ prompt: '', name: '' });
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    useEffect(() => {
        const fetchPersona = async () => {
            try {
                const res = await fetch('/api/persona', { headers: getHeaders() });
                if (res.ok) {
                    const data = await res.json();
                    setPersona(parsePersonaString(data.persona || ''));
                }
            } catch (err) {
                console.error('Fetch persona error:', err);
            } finally {
                setIsLoading(false);
            }
        };
        fetchPersona();
    }, [getHeaders]);

    const handleSave = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const res = await fetch('/api/persona', {
                method: 'POST',
                headers: getHeaders(),
                body: JSON.stringify({ persona: composePersonaString(persona) }),
            });
            if (res.ok) {
                setMessage({ type: 'success', text: t('persona_saved') });
            } else {
                const err = await res.json();
                setMessage({ type: 'error', text: err.detail || t('persona_save_error') });
            }
        } catch (err) {
            setMessage({ type: 'error', text: t('persona_save_error') });
        } finally {
            setIsSaving(false);
        }
    };

    const handleReset = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const res = await fetch('/api/persona', {
                method: 'DELETE',
                headers: getHeaders(),
            });
            if (res.ok) {
                setPersona({ prompt: '', name: '' });
                setMessage({ type: 'success', text: t('persona_reset') });
            } else {
                setMessage({ type: 'error', text: t('persona_save_error') });
            }
        } catch (err) {
            setMessage({ type: 'error', text: t('persona_save_error') });
        } finally {
            setIsSaving(false);
        }
    };

    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-12">
                <Loader2 className="w-6 h-6 text-aegis-cyan animate-spin" />
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <div className="p-4 bg-aegis-cyan/5 border border-aegis-cyan/20 rounded-xl">
                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-1">
                    {t('tab_persona')}
                </p>
                <p className="text-sm font-mono text-aegis-cyan">
                    {t('persona_desc')}
                </p>
            </div>

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                    {t('persona_name')}
                </label>
                <input
                    type="text"
                    value={persona.name || ''}
                    onChange={(e) => setPersona({ ...persona, name: e.target.value })}
                    placeholder={t('persona_name_placeholder')}
                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2.5 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                />
            </div>

            <div>
                <div className="flex justify-between items-center mb-2">
                    <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest">
                        {t('persona_prompt')}
                    </label>
                    <span className="text-[10px] font-mono text-white/20">
                        {persona.prompt.length} / 4000
                    </span>
                </div>
                <textarea
                    value={persona.prompt}
                    onChange={(e) => setPersona({ ...persona, prompt: e.target.value.slice(0, 4000) })}
                    placeholder={t('persona_prompt_placeholder')}
                    rows={8}
                    className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10 resize-none"
                />
            </div>

            {message && (
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className={`p-3 rounded-lg flex items-center gap-2 text-xs font-mono ${
                        message.type === 'success'
                            ? 'bg-green-500/10 border border-green-500/30 text-green-400'
                            : 'bg-red-500/10 border border-red-500/30 text-red-400'
                    }`}
                >
                    {message.type === 'success' && <Check className="w-4 h-4" />}
                    {message.text}
                </motion.div>
            )}

            <div className="flex gap-3 pt-4 border-t border-white/10">
                <button
                    onClick={handleReset}
                    disabled={isSaving || (!persona.prompt && !persona.name)}
                    className="flex-1 px-4 py-2.5 border border-white/10 rounded-lg text-[10px] font-mono text-white/40 hover:text-white hover:bg-white/5 transition-colors uppercase tracking-widest disabled:opacity-30"
                >
                    <div className="text-left">
                        <span className="block">{t('persona_reset')}</span>
                        <span className="text-[8px] opacity-60 normal-case tracking-normal">{t('persona_reset_hint')}</span>
                    </div>
                </button>
                <button
                    onClick={handleSave}
                    disabled={isSaving}
                    className="flex-1 flex items-center justify-center gap-2 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-lg text-[10px] font-bold text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors uppercase tracking-widest disabled:opacity-50"
                >
                    {isSaving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Save className="w-3 h-3" />}
                    {t('save_persona')}
                </button>
            </div>
        </div>
    );
};

interface GlobalProviderEntry {
    key_id: string;
    provider: string;
    label?: string;
    is_active: boolean;
    is_free_tier: boolean;
    rate_limited_until?: string;
    active_models?: string[];
}

const MotorTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const { lastRoutingInfo, lastError } = useAegisStore();
    const [globalProviders, setGlobalProviders] = useState<GlobalProviderEntry[]>([]);
    const [isLoading, setIsLoading] = useState(true);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    useEffect(() => {
        const fetchGlobalProviders = async () => {
            try {
                const res = await fetch('/api/router/keys/global', { headers: getHeaders() });
                if (res.ok) {
                    const data = await res.json();
                    setGlobalProviders(data.keys || []);
                }
            } catch (err) {
                console.error('Fetch global providers error:', err);
            } finally {
                setIsLoading(false);
            }
        };
        fetchGlobalProviders();
    }, [getHeaders]);

    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-12">
                <Loader2 className="w-6 h-6 text-aegis-cyan animate-spin" />
            </div>
        );
    }

    const engineStatusBadge = (() => {
        if (lastError?.includes('401') || lastError?.includes('AUTH')) {
            return { icon: '❌', text: 'Error de autenticación', color: 'text-red-400' };
        }
        if (lastRoutingInfo) {
            return { icon: '✅', text: `Operativo — ${lastRoutingInfo.provider} / ${lastRoutingInfo.model_id}`, color: 'text-green-400' };
        }
        return { icon: '⚠️', text: 'Configurado, sin verificar', color: 'text-amber-400' };
    })();

    const getProviderIcon = (prov: string) => {
        switch (prov) {
            case 'openai': return <Globe className="w-4 h-4 text-aegis-cyan" />;
            case 'anthropic': return <Zap className="w-4 h-4 text-amber-400" />;
            case 'groq': return <Activity className="w-4 h-4 text-green-400" />;
            case 'grok': return <Box className="w-4 h-4 text-purple-400" />;
            case 'openrouter': return <Globe className="w-4 h-4 text-blue-400" />;
            case 'ollama': return <Server className="w-4 h-4 text-emerald-400" />;
            case 'ollama_cloud': return <Cloud className="w-4 h-4 text-sky-400" />;
            case 'gemini': return <Shield className="w-4 h-4 text-red-400" />;
            case 'custom': return <Terminal className="w-4 h-4 text-indigo-400" />;
            default: return <Cpu className="w-4 h-4 text-white/40" />;
        }
    };

    return (
        <div className="space-y-6">
            {/* Active Engine Banner */}
            <div className="p-4 bg-aegis-cyan/5 border border-aegis-cyan/20 rounded-xl relative overflow-hidden">
                <div className="absolute top-0 right-0 p-2 opacity-5">
                    <Cpu className="w-16 h-16 text-aegis-cyan" />
                </div>
                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-2">
                    {t('current_engine')}
                </p>
                <p className="text-sm font-mono text-aegis-cyan font-bold uppercase">
                    {lastRoutingInfo ? `${lastRoutingInfo.provider} / ${lastRoutingInfo.model_id}` : 'Cognitive Router (CMR v2)'}
                </p>
                <p className={`text-[10px] font-mono mt-2 ${engineStatusBadge.color} flex items-center gap-1.5 uppercase`}>
                    <span>{engineStatusBadge.icon}</span> <span>{engineStatusBadge.text}</span>
                </p>
            </div>

            {/* Global Providers Section */}
            <div className="space-y-4">
                <div>
                    <h3 className="text-xs font-mono font-bold tracking-widest uppercase text-white/80">
                        Proveedores del Sistema (Global)
                    </h3>
                    <p className="text-[9px] font-mono text-white/30 uppercase mt-0.5">
                        Configurados y optimizados por tu administrador
                    </p>
                </div>

                {globalProviders.length === 0 ? (
                    <div className="p-6 border border-dashed border-white/10 rounded-2xl bg-white/2 text-center text-xs font-mono text-white/30 uppercase tracking-widest">
                        Sin proveedores globales disponibles
                    </div>
                ) : (
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        {globalProviders.map(p => {
                            const isRateLimited = p.rate_limited_until && new Date(p.rate_limited_until) > new Date();
                            const presetLabel = PROVIDER_PRESETS[p.provider as ProviderType]?.label ?? p.provider;
                            return (
                                <div key={p.key_id} className={`p-4 rounded-xl border bg-white/[0.02] transition-all flex flex-col gap-2 ${p.is_active ? 'border-white/10' : 'border-white/5 opacity-50'}`}>
                                    <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <div className="p-1.5 rounded bg-white/5">{getProviderIcon(p.provider)}</div>
                                            <span className="text-xs font-mono font-bold uppercase text-white/80">{p.label || presetLabel}</span>
                                        </div>
                                        <div className="flex items-center gap-1.5">
                                            {isRateLimited && (
                                                <span className="px-1.5 py-0.5 rounded text-[8px] font-mono uppercase bg-yellow-500/10 border border-yellow-500/30 text-yellow-400">
                                                    Limited
                                                </span>
                                            )}
                                            <span className={`px-1.5 py-0.5 rounded text-[8px] font-mono uppercase font-bold border ${p.is_active ? 'bg-green-500/10 border-green-500/30 text-green-400' : 'bg-red-500/10 border-red-500/30 text-red-400'}`}>
                                                {p.is_active ? 'Activo' : 'Inactivo'}
                                            </span>
                                        </div>
                                    </div>
                                    {p.active_models && p.active_models.length > 0 && (
                                        <div className="text-[9px] font-mono text-white/30 truncate uppercase tracking-wide">
                                            Modelos: {p.active_models.slice(0, 3).join(', ')}
                                            {p.active_models.length > 3 && ` (+${p.active_models.length - 3})`}
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                )}

                <div className="flex items-start gap-2 p-3 bg-white/5 border border-white/5 rounded-xl text-[9px] font-mono text-white/30 uppercase tracking-widest leading-loose">
                    <Info className="w-4 h-4 text-aegis-cyan shrink-0 mt-0.5" />
                    <span>Estos proveedores los gestiona tu administrador. Tu router los consumirá automáticamente.</span>
                </div>
            </div>

            {/* Divider */}
            <div className="relative flex items-center justify-center py-2">
                <div className="absolute inset-0 flex items-center"><div className="w-full border-t border-white/10"></div></div>
                <span className="relative px-4 bg-black text-[9px] font-mono text-white/30 uppercase tracking-widest">o configurá tus propias llaves</span>
            </div>

            {/* Tenant Keys Manager */}
            <div className="pt-2">
                <TenantKeyManager tenantId={tenantId} sessionKey={sessionKey} />
            </div>
        </div>
    );
};

const VozTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const { sttAvailable, fetchSirenConfig } = useAegisStore();
    const [provider, setProvider] = useState('voxtral');
    const [apiKey, setApiKey] = useState('');
    const [voiceId, setVoiceId] = useState('');
    const [voices, setVoices] = useState<Voice[]>([]);
    const [showKey, setShowKey] = useState(false);
    const [sttProvider, setSttProvider] = useState('browser');
    const [sttApiKey, setSttApiKey] = useState('');
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

    const isHttpWarning = typeof window !== 'undefined' && 
        window.location.protocol === 'http:' && 
        !['localhost', '127.0.0.1'].includes(window.location.hostname);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    const fetchConfig = useCallback(async () => {
        try {
            const res = await fetch('/api/siren/config', { headers: getHeaders() });
            if (res.ok) {
                const data: SirenConfig = await res.json();
                setProvider(data.provider || 'voxtral');
                setApiKey(data.api_key || '');
                setVoiceId(data.voice_id || '');
                setSttProvider(data.stt_provider || 'browser');
                setSttApiKey(data.stt_api_key || '');
            }
        } catch (err) {
            console.error('Fetch siren config error:', err);
        } finally {
            setIsLoading(false);
        }
    }, [getHeaders]);

    const fetchVoices = useCallback(async () => {
        try {
            const res = await fetch('/api/siren/voices', { headers: getHeaders() });
            if (res.ok) {
                const data = await res.json();
                setVoices(data.voices || []);
            }
        } catch (err) {
            console.error('Fetch voices error:', err);
        }
    }, [getHeaders]);

    useEffect(() => {
        fetchConfig();
        fetchVoices();
        fetchSirenConfig(); // CORE-233: refrescar sttAvailable desde el servidor al abrir la tab
    }, [fetchConfig, fetchVoices, fetchSirenConfig]);

    const handleSave = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const res = await fetch('/api/siren/config', {
                method: 'POST',
                headers: getHeaders(),
                body: JSON.stringify({ provider, api_key: apiKey, voice_id: voiceId, stt_provider: sttProvider, stt_api_key: sttApiKey }),
            });
            if (res.ok) {
                setMessage({ type: 'success', text: t('siren_saved') });
                fetchSirenConfig();
            } else {
                const err = await res.json();
                setMessage({ type: 'error', text: err.detail || t('siren_save_error') });
            }
        } catch (err) {
            setMessage({ type: 'error', text: t('siren_save_error') });
        } finally {
            setIsSaving(false);
        }
    };

    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-12">
                <Loader2 className="w-6 h-6 text-aegis-cyan animate-spin" />
            </div>
        );
    }

    const providersList = [
        { id: 'voxtral', label: 'Voxtral' },
        { id: 'mock', label: 'Mock' },
        { id: 'elevenlabs', label: 'ElevenLabs' },
    ];

    return (
        <div className="space-y-6">
            {isHttpWarning && (
                <div className="p-4 bg-amber-500/10 border border-amber-500/30 rounded-xl flex items-start gap-3">
                    <AlertTriangle className="w-5 h-5 text-amber-500 flex-shrink-0 mt-0.5" />
                    <div>
                        <p className="text-xs font-mono text-amber-400 uppercase tracking-widest">
                            {t('https_warning_title')}
                        </p>
                        <p className="text-[10px] font-mono text-white/40 mt-1">
                            {t('https_warning_desc')}
                        </p>
                    </div>
                </div>
            )}

            <div className="p-4 bg-white/5 border border-white/10 rounded-xl">
                <div className="flex items-center justify-between">
                    <div>
                        <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest">
                            {t('stt_status')}
                        </p>
                        <p className="text-sm font-mono text-aegis-cyan mt-1">
                            {sttAvailable ? t('stt_available') : t('stt_not_available')}
                        </p>
                    </div>
                    <div className={`w-3 h-3 rounded-full ${sttAvailable ? 'bg-green-500' : 'bg-red-500'}`} />
                </div>
            </div>

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-3">
                    {t('tts_provider')}
                </label>
                <div className="grid grid-cols-3 gap-2">
                    {providersList.map((p) => (
                        <button
                            key={p.id}
                            onClick={() => setProvider(p.id)}
                            className={`p-3 rounded-lg border text-center transition-all ${
                                provider === p.id
                                    ? 'bg-aegis-cyan/20 border-aegis-cyan text-aegis-cyan'
                                    : 'bg-white/5 border-white/10 text-white/40 hover:text-white hover:border-white/30'
                            }`}
                        >
                            <span className="text-[10px] font-mono uppercase tracking-widest">{p.label}</span>
                        </button>
                    ))}
                </div>
            </div>

            {provider !== 'voxtral' && provider !== 'mock' && (
                <div>
                    <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                        {t('provider_api_key')}
                    </label>
                    <div className="relative">
                        <input
                            type={showKey ? 'text' : 'password'}
                            value={apiKey}
                            onChange={(e) => setApiKey(e.target.value)}
                            placeholder={t('api_key_placeholder')}
                            className="w-full bg-black/40 border border-white/10 rounded-lg py-2.5 px-4 pr-10 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                        />
                        <button
                            type="button"
                            onClick={() => setShowKey(!showKey)}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-white/20 hover:text-white/50 transition-colors"
                        >
                            {showKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                        </button>
                    </div>
                </div>
            )}

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                    {t('voice_id')}
                </label>
                <select
                    value={voiceId}
                    onChange={(e) => setVoiceId(e.target.value)}
                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2.5 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all appearance-none cursor-pointer"
                >
                    <option value="">{t('auto_voice')}</option>
                    {voices.filter((v) => v.provider === provider).map((v) => (
                        <option key={v.id} value={v.id}>
                            {v.name}
                        </option>
                    ))}
                </select>
            </div>

            {message && (
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className={`p-3 rounded-lg flex items-center gap-2 text-xs font-mono ${
                        message.type === 'success'
                            ? 'bg-green-500/10 border border-green-500/30 text-green-400'
                            : 'bg-red-500/10 border border-red-500/30 text-red-400'
                    }`}
                >
                    {message.type === 'success' && <Check className="w-4 h-4" />}
                    {message.text}
                </motion.div>
            )}

            <button
                onClick={handleSave}
                disabled={isSaving}
                className="w-full flex items-center justify-center gap-2 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-lg py-3 text-[10px] font-bold text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors uppercase tracking-widest disabled:opacity-50"
            >
                {isSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                {t('save_voice_config')}
            </button>
        </div>
    );
};

const CuentaTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const currentLang = localStorage.getItem('aegis_language') || 'es';

    const handleLanguageChange = (lang: string) => {
        localStorage.setItem('aegis_language', lang);
        window.location.reload();
    };

    return (
        <div className="space-y-8">
            <div className="flex items-center gap-4 p-4 bg-white/5 border border-white/10 rounded-xl">
                <Globe className="w-5 h-5 text-aegis-cyan" />
                <div className="flex-1">
                    <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-2">
                        {t('language')}
                    </p>
                    <select
                        value={currentLang}
                        onChange={(e) => handleLanguageChange(e.target.value)}
                        className="bg-black/40 border border-white/10 rounded-lg py-2 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all appearance-none cursor-pointer"
                    >
                        <option value="es">Español</option>
                        <option value="en">English</option>
                    </select>
                </div>
            </div>

            <div className="pt-4 border-t border-white/10">
                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-4">
                    {t('change_password')}
                </p>
                <UserPasswordChange />
            </div>

            <div className="pt-4 border-t border-white/10">
                <ConnectedAccountsTab tenantId={tenantId} sessionKey={sessionKey} />
            </div>
        </div>
    );
};

const LogsTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [subTab, setSubTab] = useState<'history' | 'traces'>('history');
    const [logs, setLogs] = useState<any[]>([]);
    const [searchTerm, setSearchTerm] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [copied, setCopied] = useState(false);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    const fetchLogs = useCallback(async () => {
        setIsLoading(true);
        try {
            const endpoint = subTab === 'history' ? '/api/chat/history?limit=100' : '/api/chat/traces?limit=200';
            const res = await fetch(endpoint, { headers: getHeaders() });
            if (res.ok) {
                const data = await res.json();
                if (subTab === 'history') {
                    setLogs(data.messages || []);
                } else {
                    setLogs(data.traces || []);
                }
            } else {
                setLogs([]);
            }
        } catch (err) {
            console.error('Fetch logs error:', err);
            setLogs([]);
        } finally {
            setIsLoading(false);
        }
    }, [subTab, getHeaders]);

    useEffect(() => {
        fetchLogs();
    }, [fetchLogs]);

    const handleCopy = () => {
        let textToCopy = '';
        if (subTab === 'history') {
            textToCopy = logs
                .map((msg: any) => `[${msg.timestamp}] ${msg.role.toUpperCase()}: ${msg.content}`)
                .join('\n');
        } else {
            textToCopy = logs.join('\n');
        }
        navigator.clipboard.writeText(textToCopy);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const filteredLogs = logs.filter((log: any) => {
        if (!searchTerm) return true;
        const term = searchTerm.toLowerCase();
        if (subTab === 'history') {
            return (
                log.content.toLowerCase().includes(term) ||
                log.role.toLowerCase().includes(term) ||
                log.timestamp.toLowerCase().includes(term)
            );
        } else {
            return log.toLowerCase().includes(term);
        }
    });

    const formatTimestamp = (ts: string) => {
        try {
            const date = new Date(ts);
            return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
        } catch {
            return ts;
        }
    };

    return (
        <div className="space-y-4 flex flex-col h-[52vh]">
            <div className="flex flex-col sm:flex-row gap-3 justify-between items-start sm:items-center shrink-0">
                {/* Dual Sub-Tabs */}
                <div className="flex bg-black/40 border border-white/5 p-1 rounded-xl w-full sm:w-auto">
                    <button
                        onClick={() => {
                            setSubTab('history');
                            setLogs([]);
                        }}
                        className={`flex-1 sm:flex-none flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-xs font-mono uppercase tracking-wider transition-all ${
                            subTab === 'history'
                                ? 'bg-aegis-cyan/10 text-aegis-cyan border border-aegis-cyan/20'
                                : 'text-white/40 hover:text-white border border-transparent'
                        }`}
                    >
                        <FileText className="w-3.5 h-3.5" />
                        {t('logs_chat_history')}
                    </button>
                    <button
                        onClick={() => {
                            setSubTab('traces');
                            setLogs([]);
                        }}
                        className={`flex-1 sm:flex-none flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-xs font-mono uppercase tracking-wider transition-all ${
                            subTab === 'traces'
                                ? 'bg-aegis-cyan/10 text-aegis-cyan border border-aegis-cyan/20'
                                : 'text-white/40 hover:text-white border border-transparent'
                        }`}
                    >
                        <Terminal className="w-3.5 h-3.5" />
                        {t('logs_agent_traces')}
                    </button>
                </div>

                {/* Sub Tab Description / Actions */}
                <div className="flex items-center gap-2 w-full sm:w-auto justify-end">
                    <button
                        onClick={fetchLogs}
                        disabled={isLoading}
                        className="p-2 bg-white/5 border border-white/10 hover:bg-white/10 disabled:opacity-50 text-white/80 hover:text-white rounded-lg transition-all"
                        title={t('logs_refresh')}
                    >
                        <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin text-aegis-cyan' : ''}`} />
                    </button>
                    <button
                        onClick={handleCopy}
                        disabled={logs.length === 0}
                        className="flex items-center gap-2 px-3 py-2 bg-white/5 border border-white/10 hover:bg-white/10 disabled:opacity-50 text-white/80 hover:text-white rounded-lg transition-all text-xs font-mono uppercase tracking-wider"
                    >
                        {copied ? (
                            <>
                                <Check className="w-3.5 h-3.5 text-emerald-400" />
                                <span className="text-emerald-400">{t('logs_copied')}</span>
                            </>
                        ) : (
                            <>
                                <Copy className="w-3.5 h-3.5" />
                                <span>{t('logs_copy')}</span>
                            </>
                        )}
                    </button>
                </div>
            </div>

            {/* Description Info Alert */}
            <div className="p-3 bg-white/5 border border-white/10 rounded-xl flex items-start gap-3 shrink-0">
                <Info className="w-4 h-4 text-aegis-cyan/70 mt-0.5 shrink-0" />
                <p className="text-[11px] font-mono text-white/60 leading-relaxed">
                    {subTab === 'history' ? t('logs_dialogue_desc') : t('logs_traces_desc')}
                </p>
            </div>

            {/* Filter Search Bar */}
            <div className="relative shrink-0">
                <Search className="absolute left-3 top-2.5 w-4 h-4 text-white/30" />
                <input
                    type="text"
                    value={searchTerm}
                    onChange={(e) => setSearchTerm(e.target.value)}
                    placeholder={t('logs_search_placeholder')}
                    className="w-full bg-black/40 border border-white/10 focus:border-aegis-cyan/50 focus:ring-0 rounded-xl pl-10 pr-4 py-2 text-xs font-mono placeholder:text-white/20 transition-all"
                />
            </div>

            {/* Log Display Window */}
            <div className="flex-1 min-h-0 bg-black/60 border border-white/10 rounded-xl p-4 overflow-y-auto font-mono text-xs text-white/80 select-text scrollbar-thin">
                {isLoading && logs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full py-12 gap-3">
                        <Loader2 className="w-6 h-6 text-aegis-cyan animate-spin" />
                        <span className="text-[10px] uppercase tracking-widest text-white/40">{t('connecting_telemetry')}</span>
                    </div>
                ) : filteredLogs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full py-12 text-center text-white/30">
                        <Terminal className="w-8 h-8 mb-2 opacity-20" />
                        <p className="text-xs">{t('logs_empty')}</p>
                    </div>
                ) : (
                    <div className="space-y-2">
                        {subTab === 'history' ? (
                            (filteredLogs as any[]).map((msg, idx) => (
                                <div key={idx} className="border-b border-white/5 pb-2 last:border-0 last:pb-0 flex flex-col sm:flex-row sm:items-start gap-2">
                                    <span className="text-[10px] text-white/30 shrink-0 font-mono">
                                        [{formatTimestamp(msg.timestamp)}]
                                    </span>
                                    <span className={`text-[10px] uppercase font-bold tracking-wider px-1.5 py-0.5 rounded shrink-0 ${
                                        msg.role === 'user'
                                            ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                                            : 'bg-aegis-cyan/10 text-aegis-cyan border border-aegis-cyan/20'
                                    }`}>
                                        {msg.role === 'user' ? 'USER' : 'AEGIS'}
                                    </span>
                                    <span className="text-white/90 break-words flex-1 whitespace-pre-wrap">
                                        {msg.content}
                                    </span>
                                </div>
                            ))
                        ) : (
                            (filteredLogs as string[]).map((line, idx) => {
                                const tsMatch = line.match(/^\[(.*?)\]\s*(.*)$/);
                                if (!tsMatch) return <div key={idx} className="whitespace-pre-wrap break-all text-white/70">{line}</div>;

                                const ts = tsMatch[1];
                                const body = tsMatch[2];

                                return (
                                    <div key={idx} className="py-1 border-b border-white/5 last:border-0 flex flex-col sm:flex-row sm:items-start gap-2 text-white/80 font-mono">
                                        <span className="text-[10px] text-white/30 shrink-0 font-mono">
                                            [{formatTimestamp(ts)}]
                                        </span>
                                        <span className="text-white/90 break-words flex-1 whitespace-pre-wrap">
                                            {body}
                                        </span>
                                    </div>
                                );
                            })
                        )}
                    </div>
                )}
            </div>
        </div>
    );
};

const SettingsPanel: React.FC<SettingsPanelProps> = ({ onClose, tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [activeTab, setActiveTab] = useState<TabId>('perfil');
    const tabs = TABS(t);

    return (
        <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md"
        >
            <motion.div
                initial={{ scale: 0.95, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.95, opacity: 0 }}
                className="w-full max-w-2xl bg-aegis-steel border border-white/10 rounded-2xl overflow-hidden shadow-2xl max-h-[90vh] flex flex-col"
            >
                <div className="p-6 border-b border-white/5 flex justify-between items-center bg-black/40 shrink-0">
                    <div className="flex items-center gap-3">
                        <Settings className="w-5 h-5 text-aegis-cyan" />
                        <h3 className="text-sm font-mono font-bold tracking-widest text-aegis-cyan uppercase">
                            {t('settings_panel')}
                        </h3>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-white/20 hover:text-white transition-colors p-1"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>

                <div className="flex gap-1 p-2 border-b border-white/5 bg-black/20 shrink-0 overflow-x-auto">
                    {tabs.map((tab) => (
                        <button
                            key={tab.id}
                            onClick={() => setActiveTab(tab.id)}
                            className={`flex items-center gap-2 px-4 py-2.5 rounded-lg transition-all text-xs font-mono uppercase tracking-widest whitespace-nowrap ${
                                activeTab === tab.id
                                    ? 'bg-aegis-cyan/20 text-aegis-cyan border border-aegis-cyan/30'
                                    : 'text-white/40 hover:text-white hover:bg-white/5 border border-transparent'
                            }`}
                        >
                            {tab.icon}
                            {tab.label}
                        </button>
                    ))}
                </div>

       <div className="p-6 overflow-y-auto flex-1">
                    <AnimatePresence mode="wait">
                        <motion.div
                            key={activeTab}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -10 }}
                            transition={{ duration: 0.15 }}
                        >
                            {activeTab === 'perfil' && <PersonaTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'motor' && <MotorTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'voz' && <VozTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'logs' && <LogsTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'cuenta' && <CuentaTab tenantId={tenantId} sessionKey={sessionKey} />}
                        </motion.div>
                    </AnimatePresence>
                </div>
            </motion.div>
        </motion.div>
    );
};

export default SettingsPanel;