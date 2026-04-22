import React, { useState, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Settings, X, Save, Cpu, Volume2, Shield, Globe, Check, Eye, EyeOff, Loader2, AlertTriangle, Link2, Sparkles } from 'lucide-react';
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

type TabId = 'persona' | 'motor' | 'voz' | 'seguridad' | 'accounts';

const TABS = (t: (key: string) => string): { id: TabId; label: string; icon: React.ReactNode }[] => [
    { id: 'persona', label: t('tab_persona'), icon: <Sparkles className="w-4 h-4" /> },
    { id: 'motor', label: t('tab_motor'), icon: <Cpu className="w-4 h-4" /> },
    { id: 'voz', label: t('tab_voz'), icon: <Volume2 className="w-4 h-4" /> },
    { id: 'seguridad', label: t('tab_seguridad'), icon: <Shield className="w-4 h-4" /> },
    { id: 'accounts', label: 'Cuentas', icon: <Link2 className="w-4 h-4" /> },
];

interface EngineStatus {
    provider: string;
    model_name: string;
    api_url: string;
}

interface PersonaData {
    prompt: string;
    name?: string;
}

interface SirenConfig {
    provider: string;
    api_key?: string;
    voice_id: string;
    stt_available?: boolean;
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
                    setPersona({ prompt: data.prompt || '', name: data.name || '' });
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
                body: JSON.stringify({ prompt: persona.prompt, name: persona.name }),
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
                    disabled={isSaving || !persona.prompt}
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

const MotorTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [provider, setProvider] = useState<ProviderType>('custom');
    const [apiUrl, setApiUrl] = useState('');
    const [modelName, setModelName] = useState('');
    const [apiKey, setApiKey] = useState('');
    const [showKey, setShowKey] = useState(false);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [engineStatus, setEngineStatus] = useState<EngineStatus | null>(null);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    useEffect(() => {
        const fetchStatus = async () => {
            try {
                const res = await fetch('/api/engine/status', { headers: getHeaders() });
                if (res.ok) {
                    const data = await res.json();
                    setEngineStatus(data);
                    if (data.provider) {
                        setProvider(data.provider as ProviderType);
                        setApiUrl(data.api_url || '');
                        setModelName(data.model_name || '');
                    }
                }
            } catch (err) {
                console.error('Fetch engine status error:', err);
            } finally {
                setIsLoading(false);
            }
        };
        fetchStatus();
    }, [getHeaders]);

    const handleProviderChange = (newProvider: ProviderType) => {
        setProvider(newProvider);
        if (PROVIDER_PRESETS[newProvider]) {
            setApiUrl(PROVIDER_PRESETS[newProvider].url);
            if (PROVIDER_PRESETS[newProvider].model) {
                setModelName(PROVIDER_PRESETS[newProvider].model);
            }
        }
    };

    const handleSave = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const res = await fetch('/api/engine/configure', {
                method: 'POST',
                headers: getHeaders(),
                body: JSON.stringify({
                    provider,
                    api_url: apiUrl,
                    model_name: modelName,
                    api_key: apiKey,
                }),
            });
            if (res.ok) {
                setMessage({ type: 'success', text: t('engine_saved') });
                setEngineStatus({ provider, model_name: modelName, api_url: apiUrl });
            } else {
                const err = await res.json();
                setMessage({ type: 'error', text: err.detail || t('engine_save_error') });
            }
        } catch (err) {
            setMessage({ type: 'error', text: t('engine_save_error') });
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
            {engineStatus && (
                <div className="p-4 bg-aegis-cyan/5 border border-aegis-cyan/20 rounded-xl">
                    <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-2">
                        {t('current_engine')}
                    </p>
                    <p className="text-sm font-mono text-aegis-cyan">
                        {engineStatus.provider} / {engineStatus.model_name}
                    </p>
                </div>
            )}

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-3">
                    {t('select_provider')}
                </label>
                <div className="grid grid-cols-4 gap-2">
                    {(Object.keys(PROVIDER_PRESETS) as ProviderType[]).map((key) => {
                        const isSelected = provider === key;
                        return (
                            <button
                                key={key}
                                onClick={() => handleProviderChange(key)}
                                className={`p-3 rounded-lg border text-center transition-all ${
                                    isSelected
                                        ? 'bg-aegis-cyan/20 border-aegis-cyan text-aegis-cyan'
                                        : 'bg-white/5 border-white/10 text-white/40 hover:text-white hover:border-white/30'
                                }`}
                            >
                                <span className="text-[10px] font-mono uppercase tracking-widest">{key}</span>
                            </button>
                        );
                    })}
                </div>
            </div>

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                    {t('api_url')}
                </label>
                <input
                    type="url"
                    value={apiUrl}
                    onChange={(e) => setApiUrl(e.target.value)}
                    placeholder="https://api.example.com/v1/chat/completions"
                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2.5 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                />
            </div>

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                    {t('model_name')}
                </label>
                <input
                    type="text"
                    value={modelName}
                    onChange={(e) => setModelName(e.target.value)}
                    placeholder="gpt-4o, llama-3.3-70b-versatile, etc."
                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2.5 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                />
            </div>

            <div>
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest block mb-2">
                    {t('api_key')}
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
                {t('save_engine_config')}
            </button>
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
    }, [fetchConfig, fetchVoices]);

    const handleSave = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const res = await fetch('/api/siren/config', {
                method: 'POST',
                headers: getHeaders(),
                body: JSON.stringify({ provider, api_key: apiKey, voice_id: voiceId }),
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

const SeguridadTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
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

            <div className="space-y-4">
                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest">
                    {t('personal_keys_notice')}
                </p>
                <TenantKeyManager tenantId={tenantId} sessionKey={sessionKey} />
            </div>

            <div className="pt-4 border-t border-white/10">
                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest mb-4">
                    {t('change_password')}
                </p>
                <UserPasswordChange />
            </div>
        </div>
    );
};

const SettingsPanel: React.FC<SettingsPanelProps> = ({ onClose, tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [activeTab, setActiveTab] = useState<TabId>('persona');
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
                            {activeTab === 'persona' && <PersonaTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'motor' && <MotorTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'voz' && <VozTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'seguridad' && <SeguridadTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'accounts' && <ConnectedAccountsTab tenantId={tenantId} sessionKey={sessionKey} />}
                        </motion.div>
                    </AnimatePresence>
                </div>
            </motion.div>
        </motion.div>
    );
};

export default SettingsPanel;