import React, { useState, useEffect, useCallback } from 'react';
import { Plus, Trash2, RefreshCw, Key, Info, Globe, Zap, Box, Server, Activity, Terminal, Eye, EyeOff, Loader2, Check, Search, X, Shield, Settings, Cloud } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useTranslation } from '../../i18n';
import { PROVIDER_PRESETS, ProviderType } from '../../constants/enginePresets';

interface KeyInfo {
    key_id: string;
    provider: string;
    api_key: string;
    api_url?: string;
    label?: string;
    is_active: boolean;
    rate_limited_until?: string;
    active_models?: string[];
    is_free_tier?: boolean;
}

const ModelSelector: React.FC<{ 
    models: string[]; 
    selectedModels: string[]; 
    onChange: (selected: string[]) => void;
}> = ({ models, selectedModels, onChange }) => {
    const { t } = useTranslation();
    const [search, setSearch] = useState('');
    const filtered = models.filter(m => m.toLowerCase().includes(search.toLowerCase()));

    const toggle = (model: string) => {
        if (selectedModels.includes(model)) {
            onChange(selectedModels.filter(m => m !== model));
        } else {
            onChange([...selectedModels, model]);
        }
    };

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest">
                    Modelos Detectados ({selectedModels.length}/{models.length})
                </label>
                <div className="flex gap-3">
                    <button type="button" onClick={() => onChange(filtered)} className="text-[9px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan uppercase tracking-widest">{t('all')}</button>
                    <button type="button" onClick={() => onChange([])} className="text-[9px] font-mono text-white/20 hover:text-white/40 uppercase tracking-widest">{t('none')}</button>
                </div>
            </div>

            <div className="relative">
                <input 
                    type="text" 
                    value={search} 
                    onChange={e => setSearch(e.target.value)} 
                    placeholder={t('filter_models')} 
                    className="w-full bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 pl-10 text-[11px] font-mono focus:border-aegis-cyan/50 placeholder:text-white/10 outline-none text-white"
                />
                <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-white/20" />
            </div>

            <div className="max-h-40 overflow-y-auto space-y-1 pr-2 custom-scrollbar">
                {filtered.map(model => (
                    <label 
                        key={model} 
                        className="flex items-center gap-3 px-3 py-2 rounded-xl hover:bg-white/5 cursor-pointer group transition-colors"
                    >
                        <div className={`w-4 h-4 rounded border flex items-center justify-center transition-all ${selectedModels.includes(model) ? 'bg-aegis-cyan border-aegis-cyan' : 'border-white/20 bg-black/40'}`}>
                            {selectedModels.includes(model) && <Check className="w-3 h-3 text-black" />}
                        </div>
                        <input 
                            type="checkbox" 
                            className="hidden" 
                            checked={selectedModels.includes(model)} 
                            onChange={() => toggle(model)} 
                        />
                        <span className="text-[11px] font-mono text-white/60 group-hover:text-white transition-colors truncate">
                            {model}
                        </span>
                    </label>
                ))}
            </div>
        </div>
    );
};

const KeyModal: React.FC<{
    onClose: () => void;
    onSaved: () => void;
    tenantId: string;
    sessionKey: string;
    initialKey?: KeyInfo;
}> = ({ onClose, onSaved, tenantId, sessionKey, initialKey }) => {
    const isEdit = !!initialKey;
    const { t } = useTranslation();
    const isKeylessProvider = (p: ProviderType) => p === 'ollama' || p === 'custom';
    const [selectedProvider, setSelectedProvider] = useState<ProviderType>(
        (initialKey?.provider as ProviderType) || 'openai'
    );
    const [apiKey, setApiKey] = useState('');
    const [label, setLabel] = useState(initialKey?.label || '');
    const [showKey, setShowKey] = useState(false);
    const [isVerifying, setIsVerifying] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [models, setModels] = useState<string[]>([]);
    const [selectedModels, setSelectedModels] = useState<string[]>(initialKey?.active_models || []);
    const [isFreeTier, setIsFreeTier] = useState<boolean>(initialKey?.is_free_tier ?? false);
    const [isActive, setIsActive] = useState<boolean>(initialKey?.is_active ?? true);
    const [verifyError, setVerifyError] = useState<string | null>(null);
    const [step, setStep] = useState<'config' | 'models'>(isEdit ? 'models' : 'config');

    const handleVerify = useCallback(async () => {
        setIsVerifying(true);
        setVerifyError(null);
        try {
            const res = await fetch('/api/providers/models', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    provider: selectedProvider,
                    ...(isKeylessProvider(selectedProvider) ? {} : { api_key: apiKey || '' }),
                    api_url: PROVIDER_PRESETS[selectedProvider].url
                })
            });
            if (res.ok) {
                const data = await res.json();
                const models = Array.isArray(data.models) ? data.models : [];
                setModels(models);
                if (!isEdit) setSelectedModels(models);
                setStep('models');
                if (models.length === 0) {
                    setVerifyError(t('no_models_returned'));
                }
            } else {
                let errMessage = '';
                try {
                    const errData = await res.json();
                    errMessage = errData.error || errData.detail || '';
                } catch { /* silencio */ }
                const prefix = res.status === 502 ? `${t('provider_rejected_key')}: ` : '';
                setVerifyError(prefix + (errMessage || t('invalid_api_key_error')));
            }
        } catch (err) {
            setVerifyError(t('provider_connection_error'));
        } finally {
            setIsVerifying(false);
        }
    }, [apiKey, isEdit, selectedProvider, sessionKey, tenantId, t]);

    useEffect(() => {
        if (isEdit && models.length === 0) {
            handleVerify();
        }
    }, [isEdit, models.length, handleVerify]);

    const handleSave = async () => {
        setIsSaving(true);
        try {
            const url = isEdit 
                ? `/api/router/keys/tenant/${initialKey.key_id}?tenant_id=${encodeURIComponent(tenantId)}`
                : '/api/router/keys/tenant';
            
            const res = await fetch(url, {
                method: isEdit ? 'PUT' : 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    tenant_id: tenantId,
                    provider: selectedProvider,
                    ...(apiKey.trim() ? { api_key: apiKey.trim() } : {}),
                    api_url: PROVIDER_PRESETS[selectedProvider].url,
                    models: selectedModels,
                    is_free_tier: isFreeTier,
                    is_active: isActive,
                    label: label || null,
                })
            });
            if (res.ok) {
                onSaved();
                onClose();
            } else {
                const err = await res.json();
                setVerifyError(err.detail || t('provider_save_error'));
            }
        } catch (err) {
            setVerifyError(t('network_save_error'));
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <motion.div 
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
        >
            <div className="glass w-full max-w-xl rounded-3xl border border-white/10 shadow-2xl overflow-hidden relative">
                <button 
                    onClick={onClose}
                    className="absolute top-6 right-6 p-2 rounded-full hover:bg-white/5 text-white/30 hover:text-white transition-colors"
                >
                    <X className="w-5 h-5" />
                </button>

                <div className="p-8">
                    <div className="flex items-center gap-4 mb-6">
                        <div className="p-3 rounded-2xl bg-aegis-cyan/10 border border-aegis-cyan/20">
                            {isEdit ? <Settings className="w-6 h-6 text-aegis-cyan" /> : <Plus className="w-6 h-6 text-aegis-cyan" />}
                        </div>
                        <div>
                            <h2 className="text-xl font-bold tracking-[0.2em] uppercase text-white">
                                {isEdit ? 'Editar Clave' : 'Vincular Clave'}
                            </h2>
                            <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest mt-1">
                                {isEdit ? initialKey.label || selectedProvider : 'Llave cognitiva del tenant'}
                            </p>
                        </div>
                    </div>

                    <AnimatePresence mode="wait">
                        {step === 'config' ? (
                            <motion.div 
                                key="config"
                                initial={{ opacity: 0, x: -20 }}
                                animate={{ opacity: 1, x: 0 }}
                                exit={{ opacity: 0, x: 20 }}
                                className="space-y-6"
                            >
                                <div className="grid grid-cols-4 sm:grid-cols-9 gap-2">
                                    {(Object.keys(PROVIDER_PRESETS) as ProviderType[]).map((key) => {
                                        const preset = PROVIDER_PRESETS[key];
                                        const isSelected = selectedProvider === key;
                                        return (
                                            <button 
                                                key={key} 
                                                disabled={isEdit}
                                                type="button"
                                                onClick={() => {
                                                    setSelectedProvider(key);
                                                    setVerifyError(null);
                                                }} 
                                                className={`p-2.5 rounded-xl border transition-all duration-300 flex flex-col items-center gap-1.5 group ${isSelected ? 'bg-aegis-cyan/20 border-aegis-cyan shadow-[0_0_15px_rgba(0,186,211,0.2)] scale-105' : 'bg-white/5 border-white/10 hover:border-white/30 hover:bg-white/10 opacity-60 hover:opacity-100'} ${isEdit && !isSelected ? 'hidden' : ''}`}
                                                title={preset.label}
                                            >
                                                <div className={`p-1 rounded-lg ${isSelected ? 'text-aegis-cyan' : 'text-white/40'}`}>
                                                    {key === 'openai' && <Globe className="w-3.5 h-3.5" />}
                                                    {key === 'anthropic' && <Zap className="w-3.5 h-3.5" />}
                                                    {key === 'groq' && <Activity className="w-3.5 h-3.5" />}
                                                    {key === 'grok' && <Box className="w-3.5 h-3.5" />}
                                                    {key === 'openrouter' && <Globe className="w-3.5 h-3.5" />}
                                                    {key === 'ollama' && <Server className="w-3.5 h-3.5" />}
                                                    {key === 'ollama_cloud' && <Cloud className="w-3.5 h-3.5" />}
                                                    {key === 'gemini' && <Shield className="w-3.5 h-3.5" />}
                                                    {key === 'custom' && <Terminal className="w-3.5 h-3.5" />}
                                                </div>
                                            </button>
                                        );
                                    })}
                                </div>

                                <div className="space-y-4">
                                    {!isKeylessProvider(selectedProvider) && (
                                        <div className="space-y-1.5">
                                            <div className="flex justify-between items-center ml-1">
                                                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest">API Key de {PROVIDER_PRESETS[selectedProvider].label}</label>
                                            </div>
                                            <div className="relative">
                                                <input
                                                    type={showKey ? 'text' : 'password'}
                                                    value={apiKey}
                                                    onChange={(e) => setApiKey(e.target.value)}
                                                    placeholder={isEdit ? "••••••••••••••••" : "sk-..."}
                                                    className="w-full bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 pr-10 text-xs font-mono focus:border-aegis-cyan/50 outline-none text-white"
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

                                    <div className="space-y-1.5">
                                        <label className="block text-[10px] font-mono text-white/40 uppercase tracking-widest ml-1">Identificador de la Clave</label>
                                        <input
                                            type="text"
                                            value={label}
                                            onChange={(e) => setLabel(e.target.value)}
                                            placeholder="Mi llave personal"
                                            className="w-full bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 text-xs font-mono focus:border-aegis-cyan/50 outline-none text-white"
                                        />
                                    </div>

                                    {verifyError && (
                                        <div className="bg-red-500/10 border border-red-500/30 p-3 rounded-xl flex items-center gap-3">
                                            <Terminal className="w-4 h-4 text-red-500" />
                                            <p className="text-[10px] font-mono text-red-400 uppercase tracking-tight">{verifyError}</p>
                                        </div>
                                    )}

                                    <button
                                        type="button"
                                        onClick={handleVerify}
                                        disabled={isVerifying || (!apiKey && !isEdit && !isKeylessProvider(selectedProvider))}
                                        className={`w-full group relative overflow-hidden rounded-xl py-3.5 transition-all duration-500 ${isVerifying ? "bg-aegis-cyan/20 cursor-wait" : "bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30"}`}
                                    >
                                        <div className="relative z-10 flex items-center justify-center gap-3">
                                            {isVerifying ? <Loader2 className="w-4 h-4 text-aegis-cyan animate-spin" /> : <Shield className="w-4 h-4 text-aegis-cyan" />}
                                            <span className="text-[10px] font-mono font-bold tracking-[0.3em] uppercase text-aegis-cyan">
                                                {isVerifying ? 'Verificando...' : 'Verificar y ver modelos'}
                                            </span>
                                        </div>
                                    </button>
                                </div>
                            </motion.div>
                        ) : (
                            <motion.div 
                                key="models"
                                initial={{ opacity: 0, x: 20 }}
                                animate={{ opacity: 1, x: 0 }}
                                exit={{ opacity: 0, x: -20 }}
                                className="space-y-4"
                            >
                                <ModelSelector models={models} selectedModels={selectedModels} onChange={setSelectedModels} />

                                <div className="space-y-3">
                                    {/* Active state toggle */}
                                    <button
                                        type="button"
                                        onClick={() => setIsActive(v => !v)}
                                        className={`w-full flex items-center justify-between px-4 py-3 rounded-xl border transition-all ${isActive ? 'bg-green-500/10 border-green-500/30' : 'bg-white/5 border-white/10 hover:border-white/20'}`}
                                    >
                                        <div className="text-left">
                                            <p className={`text-[10px] font-mono font-bold uppercase tracking-widest ${isActive ? 'text-green-400' : 'text-white/40'}`}>
                                                {isActive ? 'Llave Activa' : 'Llave Inactiva'}
                                            </p>
                                            <p className="text-[8px] font-mono text-white/20 mt-0.5 uppercase">
                                                {isActive ? 'Disponible para enrutamiento' : 'Desactivada para enrutamiento'}
                                            </p>
                                        </div>
                                        <div className={`w-10 h-5 rounded-full transition-all relative ${isActive ? 'bg-green-500/40' : 'bg-white/10'}`}>
                                            <div className={`absolute top-0.5 w-4 h-4 rounded-full transition-all ${isActive ? 'left-5 bg-green-400' : 'left-0.5 bg-white/30'}`} />
                                        </div>
                                    </button>

                                    {/* Free tier toggle */}
                                    <button
                                        type="button"
                                        onClick={() => setIsFreeTier(v => !v)}
                                        className={`w-full flex items-center justify-between px-4 py-3 rounded-xl border transition-all ${isFreeTier ? 'bg-emerald-500/10 border-emerald-500/30' : 'bg-white/5 border-white/10 hover:border-white/20'}`}
                                    >
                                        <div className="text-left">
                                            <p className={`text-[10px] font-mono font-bold uppercase tracking-widest ${isFreeTier ? 'text-emerald-400' : 'text-white/40'}`}>
                                                {isFreeTier ? 'Clave de nivel Gratuito' : 'Clave de nivel Pago'}
                                            </p>
                                            <p className="text-[8px] font-mono text-white/20 mt-0.5 uppercase">
                                                {isFreeTier ? 'Se consume primero' : 'Consumo secundario'}
                                            </p>
                                        </div>
                                        <div className={`w-10 h-5 rounded-full transition-all relative ${isFreeTier ? 'bg-emerald-500/40' : 'bg-white/10'}`}>
                                            <div className={`absolute top-0.5 w-4 h-4 rounded-full transition-all ${isFreeTier ? 'left-5 bg-emerald-400' : 'left-0.5 bg-white/30'}`} />
                                        </div>
                                    </button>
                                </div>

                                <div className="flex gap-4 pt-4 border-t border-white/10">
                                    <button 
                                        type="button"
                                        onClick={() => setStep('config')}
                                        disabled={isSaving}
                                        className="flex-1 px-4 py-2.5 border border-white/10 rounded-xl text-[10px] font-mono text-white/40 hover:bg-white/5 transition-colors uppercase tracking-widest"
                                    >
                                        ← {isEdit ? 'Editar Key' : 'Atrás'}
                                    </button>
                                    <button 
                                        type="button"
                                        onClick={handleSave}
                                        disabled={isSaving || selectedModels.length === 0}
                                        className="flex-[2] px-8 py-2.5 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-xl text-[10px] font-mono text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors uppercase tracking-widest font-bold flex items-center justify-center gap-2"
                                    >
                                        {isSaving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
                                        {isSaving ? 'Guardando...' : isEdit ? 'Actualizar Clave' : 'Activar Clave'}
                                    </button>
                                </div>
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>
            </div>
        </motion.div>
    );
};

const TenantKeyManager: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [keys, setKeys] = useState<KeyInfo[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [showModal, setShowModal] = useState(false);
    const [editingKey, setEditingKey] = useState<KeyInfo | null>(null);

    // Aegis Connect state
    const [activeTab, setActiveTab] = useState<'keys' | 'connect'>('keys');
    const [orionToken, setOrionToken] = useState('');
    const [orionStatus, setOrionStatus] = useState<string | null>(null);
    const [tunnelUrl, setTunnelUrl] = useState<string | null>(null);
    const [isSavingOrion, setIsSavingOrion] = useState(false);
    const [orionError, setOrionError] = useState<string | null>(null);
    const [orionSuccess, setOrionSuccess] = useState(false);

    const fetchKeys = useCallback(async () => {
        setIsLoading(true);
        try {
            const res = await fetch(
                `/api/router/keys/tenant?tenant_id=${encodeURIComponent(tenantId)}`,
                { headers: { 'x-citadel-key': sessionKey } }
            );
            if (res.ok) {
                const data = await res.json();
                setKeys(data.keys || []);
            }
        } catch (err) {
            console.error('Failed to fetch tenant keys:', err);
        } finally {
            setIsLoading(false);
        }
    }, [tenantId, sessionKey]);

    const fetchConnectStatus = useCallback(async () => {
        try {
            const configRes = await fetch('/api/workspace/config', {
                headers: { 
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                }
            });
            if (configRes.ok) {
                const configData = await configRes.json();
                setOrionStatus(configData.orion_id_token_status || null);
            }

            const statusRes = await fetch('/api/status', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                }
            });
            if (statusRes.ok) {
                const statusData = await statusRes.json();
                setTunnelUrl(statusData.tunnel_url || null);
            }
        } catch (err) {
            console.error('Error fetching Aegis Connect status:', err);
        }
    }, [tenantId, sessionKey]);

    useEffect(() => { 
        fetchKeys(); 
        fetchConnectStatus();
    }, [fetchKeys, fetchConnectStatus]);

    const handleToggle = async (keyId: string, newActive: boolean) => {
        setKeys(prev => prev.map(k =>
            k.key_id === keyId ? { ...k, is_active: newActive } : k
        ));
        try {
            const res = await fetch(`/api/router/keys/tenant/${encodeURIComponent(keyId)}?tenant_id=${encodeURIComponent(tenantId)}`, {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ is_active: newActive }),
            });
            if (!res.ok) {
                setKeys(prev => prev.map(k =>
                    k.key_id === keyId ? { ...k, is_active: !newActive } : k
                ));
            }
        } catch {
            setKeys(prev => prev.map(k =>
                k.key_id === keyId ? { ...k, is_active: !newActive } : k
            ));
        }
    };

    const handleDelete = async (keyId: string) => {
        if (!confirm('¿Eliminar esta clave personal?')) return;
        try {
            await fetch(
                `/api/router/keys/tenant/${encodeURIComponent(keyId)}?tenant_id=${encodeURIComponent(tenantId)}`,
                { 
                    method: 'DELETE',
                    headers: { 'x-citadel-key': sessionKey }
                }
            );
            await fetchKeys();
        } catch (err) {
            console.error('Failed to delete key:', err);
        }
    };

    const handleSaveOrionToken = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSavingOrion(true);
        setOrionError(null);
        setOrionSuccess(false);

        if (orionToken.trim() && !orionToken.trim().startsWith('orion_id_tok_live_aegis_')) {
            setOrionError('Token inválido. Debe comenzar con orion_id_tok_live_aegis_');
            setIsSavingOrion(false);
            return;
        }

        try {
            const res = await fetch('/api/workspace/config', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    key: 'orion_id_token',
                    value: orionToken.trim()
                })
            });

            if (res.ok) {
                setOrionSuccess(true);
                setOrionToken('');
                setTimeout(() => setOrionSuccess(false), 3000);
                await fetchConnectStatus();
            } else {
                const errData = await res.json();
                setOrionError(errData.error || 'Error al guardar el token');
            }
        } catch (err) {
            setOrionError('Error al conectar con la API');
        } finally {
            setIsSavingOrion(false);
        }
    };

    const handleDisconnectOrion = async () => {
        if (!confirm('¿Desconectar este nodo de Orion ID? El túnel público se desactivará.')) return;
        setIsSavingOrion(true);
        setOrionError(null);

        try {
            const res = await fetch('/api/workspace/config', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    key: 'orion_id_token',
                    value: ''
                })
            });

            if (res.ok) {
                await fetchConnectStatus();
            } else {
                setOrionError('Error al desconectar');
            }
        } catch (err) {
            setOrionError('Error de red');
        } finally {
            setIsSavingOrion(false);
        }
    };

    const getRateLimitText = (until: string | undefined): string => {
        if (!until) return '';
        const ms = new Date(until).getTime() - Date.now();
        if (ms <= 0) return '';
        const minutes = Math.ceil(ms / 60000);
        return t('rate_limited_until', { minutes: minutes.toString() });
    };

    return (
        <div className="glass p-6 rounded-2xl border border-white/10 bg-white/[0.01]">
            {/* Tab Navigation */}
            <div className="flex gap-4 border-b border-white/10 pb-4 mb-6">
                <button
                    type="button"
                    onClick={() => setActiveTab('keys')}
                    className={`pb-2 px-1 text-xs font-mono font-bold tracking-widest uppercase transition-all relative ${
                        activeTab === 'keys' ? 'text-aegis-cyan' : 'text-white/40 hover:text-white'
                    }`}
                >
                    {t('my_keys')}
                    {activeTab === 'keys' && (
                        <motion.div layoutId="activeTabUnderline" className="absolute bottom-0 left-0 right-0 h-0.5 bg-aegis-cyan" />
                    )}
                </button>
                <button
                    type="button"
                    onClick={() => setActiveTab('connect')}
                    className={`pb-2 px-1 text-xs font-mono font-bold tracking-widest uppercase transition-all relative flex items-center gap-2 ${
                        activeTab === 'connect' ? 'text-aegis-cyan' : 'text-white/40 hover:text-white'
                    }`}
                >
                    <Cloud className="w-3.5 h-3.5" />
                    Aegis Connect
                    {orionStatus === 'configured' && (
                        <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                    )}
                    {activeTab === 'connect' && (
                        <motion.div layoutId="activeTabUnderline" className="absolute bottom-0 left-0 right-0 h-0.5 bg-aegis-cyan" />
                    )}
                </button>
            </div>

            <AnimatePresence mode="wait">
                {activeTab === 'keys' ? (
                    <motion.div
                        key="keysTab"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -10 }}
                        className="space-y-6"
                    >
                        <div className="flex items-start gap-3 p-3 bg-white/5 border border-white/10 rounded-lg">
                            <Info className="w-4 h-4 text-aegis-cyan mt-0.5 shrink-0" />
                            <p className="text-[10px] font-mono text-white/40 leading-relaxed uppercase tracking-wider">
                                {t('global_keys_usage_info')}
                            </p>
                        </div>

                        <div className="flex items-center justify-between">
                            <div className="flex items-center gap-3">
                                <Key className="w-5 h-5 text-aegis-cyan" />
                                <h3 className="text-sm font-mono font-bold tracking-widest uppercase text-white">{t('my_keys')}</h3>
                            </div>
                            <div className="flex gap-2">
                                <button type="button" onClick={fetchKeys} className="p-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors">
                                    <RefreshCw className={`w-4 h-4 text-white/40 ${isLoading ? 'animate-spin' : ''}`} />
                                </button>
                                <button
                                    type="button"
                                    onClick={() => { setEditingKey(null); setShowModal(true); }}
                                    className="flex items-center gap-2 px-3 py-2 bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-lg hover:bg-aegis-cyan/20 transition-colors text-xs font-mono text-aegis-cyan font-bold"
                                >
                                    <Plus className="w-4 h-4" /> {t('add_key')}
                                </button>
                            </div>
                        </div>

                        {isLoading && keys.length === 0 ? (
                            <div className="text-center py-8 text-white/30 text-xs font-mono uppercase tracking-widest animate-pulse">{t('syncing')}</div>
                        ) : keys.length === 0 ? (
                            <div className="text-center py-8 text-white/30 text-xs font-mono uppercase tracking-widest border border-dashed border-white/10 rounded-xl bg-white/2">{t('personal_keys_notice')}</div>
                        ) : (
                            <div className="overflow-x-auto">
                                <table className="w-full text-xs font-mono">
                                    <thead>
                                        <tr className="text-white/30 uppercase tracking-widest border-b border-white/5">
                                            <th className="text-left py-2 pr-4">Identificador</th>
                                            <th className="text-left py-2 pr-4">Proveedor</th>
                                            <th className="text-left py-2 pr-4">Clasificación</th>
                                            <th className="text-left py-2 pr-4">Modelos Habilitados</th>
                                            <th className="text-left py-2 pr-4">Estado</th>
                                            <th className="text-right py-2">Acciones</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {keys.map((k) => {
                                            const rateLimitText = getRateLimitText(k.rate_limited_until);
                                            return (
                                                <tr key={k.key_id} className="border-b border-white/5 hover:bg-white/[0.02]">
                                                    <td className="py-3 pr-4 text-white font-bold">{k.label || '—'}</td>
                                                    <td className="py-3 pr-4 text-aegis-cyan uppercase">{k.provider}</td>
                                                    <td className="py-3 pr-4">
                                                        <span className={`px-2 py-0.5 rounded text-[9px] font-bold border uppercase ${k.is_free_tier ? 'text-emerald-400 border-emerald-500/30 bg-emerald-500/10' : 'text-purple-400 border-purple-500/30 bg-purple-500/10'}`}>
                                                            {k.is_free_tier ? 'FREE' : 'PAID'}
                                                        </span>
                                                    </td>
                                                    <td className="py-3 pr-4 text-white/40 truncate max-w-[200px]" title={k.active_models?.join(', ')}>
                                                        {k.active_models && k.active_models.length > 0 ? (
                                                            <>
                                                                {k.active_models.slice(0, 2).join(', ')}
                                                                {k.active_models.length > 2 && ` (+${k.active_models.length - 2})`}
                                                            </>
                                                        ) : 'Auto'}
                                                    </td>
                                                    <td className="py-3 pr-4">
                                                        {rateLimitText ? (
                                                            <span className="text-yellow-400 uppercase">{rateLimitText}</span>
                                                        ) : (
                                                            <button
                                                                type="button"
                                                                onClick={() => handleToggle(k.key_id, !k.is_active)}
                                                                className={`relative w-10 h-5 rounded-full transition-colors duration-300
                                                                    ${k.is_active ? 'bg-green-500/40 border-green-500/50' : 'bg-white/10 border-white/20'}
                                                                    border hover:opacity-80`}
                                                                title={k.is_active ? 'Activo' : 'Inactivo'}
                                                            >
                                                                <div className={`absolute top-0.5 w-4 h-4 rounded-full transition-all duration-300
                                                                    ${k.is_active ? 'left-5 bg-green-400' : 'left-0.5 bg-white/30'}`}
                                                                />
                                                            </button>
                                                        )}
                                                    </td>
                                                    <td className="py-3 text-right">
                                                        <div className="flex items-center justify-end gap-2">
                                                            <button
                                                                type="button"
                                                                onClick={() => { setEditingKey(k); setShowModal(true); }}
                                                                className="p-1.5 border border-white/10 rounded hover:bg-white/5 transition-colors group"
                                                                title="Editar modelos/clave"
                                                            >
                                                                <Settings className="w-3.5 h-3.5 text-white/30 group-hover:text-white" />
                                                            </button>
                                                            <button
                                                                type="button"
                                                                onClick={() => handleDelete(k.key_id)}
                                                                className="p-1.5 border border-red-500/20 rounded hover:bg-red-500/10 transition-colors group"
                                                                title="Eliminar"
                                                            >
                                                                <Trash2 className="w-3.5 h-3.5 text-red-400 group-hover:text-red-500" />
                                                            </button>
                                                        </div>
                                                    </td>
                                                </tr>
                                            );
                                        })}
                                    </tbody>
                                </table>
                            </div>
                        )}
                    </motion.div>
                ) : (
                    <motion.div
                        key="connectTab"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -10 }}
                        className="space-y-6"
                    >
                        <div className="flex items-start gap-3 p-3 bg-white/5 border border-white/10 rounded-lg">
                            <Cloud className="w-4 h-4 text-aegis-cyan mt-0.5 shrink-0 animate-pulse" />
                            <div>
                                <p className="text-[10px] font-mono text-white/80 leading-relaxed uppercase tracking-wider font-bold">
                                    Vincular con Orion ID
                                </p>
                                <p className="text-[9px] font-mono text-white/40 leading-relaxed uppercase tracking-widest mt-1">
                                    Conectá tu nodo de Aegis para habilitar un túnel HTTPS persistente. Vas a poder acceder de forma remota a tu terminal a través de tu slug de Orion ID.
                                </p>
                            </div>
                        </div>

                        {orionStatus === 'configured' ? (
                            <div className="p-6 rounded-2xl border border-emerald-500/20 bg-emerald-500/[0.02] space-y-4">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3">
                                        <span className="relative flex h-3 w-3">
                                            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                                            <span className="relative inline-flex rounded-full h-3 w-3 bg-emerald-500"></span>
                                        </span>
                                        <h4 className="text-xs font-mono font-bold tracking-widest uppercase text-emerald-400">
                                            Túnel Activo con Orion ID
                                        </h4>
                                    </div>
                                    <span className="text-[9px] font-mono px-2 py-0.5 border border-emerald-500/30 rounded text-emerald-400 bg-emerald-500/10 uppercase tracking-widest font-bold">
                                        Persistente
                                    </span>
                                </div>

                                <div className="space-y-1.5">
                                    <label className="block text-[9px] font-mono text-white/30 uppercase tracking-widest">
                                        URL de Acceso Remoto Seguro
                                    </label>
                                    <div className="flex gap-2">
                                        <div className="flex-1 bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 text-xs font-mono text-aegis-cyan truncate select-all">
                                            {tunnelUrl || `https://aegistest.orioncrea.com/u/${tenantId}`}
                                        </div>
                                        <button
                                            type="button"
                                            onClick={() => {
                                                navigator.clipboard.writeText(tunnelUrl || `https://aegistest.orioncrea.com/u/${tenantId}`);
                                                alert('¡URL copiado al portapapeles!');
                                            }}
                                            className="px-4 border border-white/10 rounded-xl hover:bg-white/5 transition-colors text-[10px] font-mono text-white/60 hover:text-white uppercase tracking-wider font-bold"
                                        >
                                            Copiar
                                        </button>
                                        <a
                                            href={tunnelUrl || `https://aegistest.orioncrea.com/u/${tenantId}`}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            className="px-4 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-xl hover:bg-aegis-cyan/30 transition-colors text-[10px] font-mono text-aegis-cyan uppercase tracking-wider font-bold flex items-center justify-center"
                                        >
                                            Acceder
                                        </a>
                                    </div>
                                </div>

                                <div className="pt-2 border-t border-white/5 flex justify-between items-center">
                                    <span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">
                                        Aegis Connect v1.0.0
                                    </span>
                                    <button
                                        type="button"
                                        disabled={isSavingOrion}
                                        onClick={handleDisconnectOrion}
                                        className="px-4 py-2 border border-red-500/20 hover:border-red-500/40 rounded-xl hover:bg-red-500/10 transition-all text-[9px] font-mono text-red-400 hover:text-red-500 uppercase tracking-widest font-bold flex items-center gap-1.5"
                                    >
                                        {isSavingOrion ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <X className="w-3.5 h-3.5" />}
                                        Desconectar
                                    </button>
                                </div>
                            </div>
                        ) : (
                            <form onSubmit={handleSaveOrionToken} className="space-y-4">
                                <div className="space-y-1.5">
                                    <label className="block text-[10px] font-mono text-white/40 uppercase tracking-widest ml-1">
                                        Token de Acceso Aegis Connect
                                    </label>
                                    <input
                                        type="password"
                                        value={orionToken}
                                        onChange={(e) => setOrionToken(e.target.value)}
                                        placeholder="orion_id_tok_live_aegis_..."
                                        className="w-full bg-black/40 border border-white/10 rounded-xl py-3 px-4 text-xs font-mono focus:border-aegis-cyan/50 outline-none text-white placeholder:text-white/10"
                                        required
                                    />
                                    <p className="text-[8px] font-mono text-white/20 uppercase tracking-wider ml-1 mt-1">
                                        Pegá el token generado desde tu panel web de Orion ID para registrar este nodo.
                                    </p>
                                </div>

                                {orionError && (
                                    <div className="bg-red-500/10 border border-red-500/30 p-3 rounded-xl flex items-center gap-3">
                                        <Terminal className="w-4 h-4 text-red-500" />
                                        <p className="text-[9px] font-mono text-red-400 uppercase tracking-tight">{orionError}</p>
                                    </div>
                                )}

                                {orionSuccess && (
                                    <div className="bg-emerald-500/10 border border-emerald-500/30 p-3 rounded-xl flex items-center gap-3">
                                        <Check className="w-4 h-4 text-emerald-400" />
                                        <p className="text-[9px] font-mono text-emerald-400 uppercase tracking-tight">¡Token vinculado con éxito! Conectando túnel...</p>
                                    </div>
                                )}

                                <button
                                    type="submit"
                                    disabled={isSavingOrion || !orionToken.trim()}
                                    className={`w-full group relative overflow-hidden rounded-xl py-3.5 transition-all duration-500 ${
                                        isSavingOrion || !orionToken.trim()
                                            ? "bg-white/5 border border-white/10 cursor-not-allowed opacity-50"
                                            : "bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30 text-aegis-cyan"
                                    }`}
                                >
                                    <div className="relative z-10 flex items-center justify-center gap-3">
                                        {isSavingOrion ? <Loader2 className="w-4 h-4 animate-spin" /> : <Shield className="w-4 h-4" />}
                                        <span className="text-[10px] font-mono font-bold tracking-[0.3em] uppercase">
                                            {isSavingOrion ? 'Vinculando...' : 'Vincular con Orion ID'}
                                        </span>
                                    </div>
                                </button>
                            </form>
                        )}
                    </motion.div>
                )}
            </AnimatePresence>

            <AnimatePresence>
                {showModal && (
                    <KeyModal 
                        tenantId={tenantId}
                        sessionKey={sessionKey}
                        initialKey={editingKey || undefined}
                        onClose={() => setShowModal(false)}
                        onSaved={() => { fetchKeys(); setShowModal(false); }}
                    />
                )}
            </AnimatePresence>
        </div>
    );
};

export default TenantKeyManager;
