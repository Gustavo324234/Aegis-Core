import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Plus, Shield, Globe, Zap, Box, Server, Activity, Terminal, Eye, EyeOff, Cpu, X, Loader2, Check, Search, Trash2, Settings } from 'lucide-react';
import { useTranslation } from '../i18n';
import { PROVIDER_PRESETS, ProviderType } from '../constants/enginePresets';

interface ProviderEntry {
    key_id: string;
    provider: string;
    label?: string;
    is_active: boolean;
    rate_limited_until?: string;
    active_models?: string[]; // modelos tildados por el usuario
}

const ProviderCard: React.FC<{
    provider: ProviderEntry;
    onDelete: (keyId: string) => void;
    onEdit: (provider: ProviderEntry) => void;
}> = ({ provider, onDelete, onEdit }) => {
    const { t } = useTranslation();
    const isRateLimited = provider.rate_limited_until && new Date(provider.rate_limited_until) > new Date();
    const status = !provider.is_active ? 'inactive' : isRateLimited ? 'limited' : 'active';

    const statusColors = {
        active: 'text-green-400 border-green-500/30 bg-green-500/10',
        limited: 'text-yellow-400 border-yellow-500/30 bg-yellow-500/10',
        inactive: 'text-white/30 border-white/10 bg-white/5',
    };

    const providerLabel = PROVIDER_PRESETS[provider.provider as ProviderType]?.label ?? provider.provider;

    return (
        <div className="glass p-5 rounded-2xl border border-white/10 hover:border-white/20 transition-all group/card">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    {/* Icono del provider */}
                    <div className="p-2 rounded-lg bg-white/5">
                        <Globe className="w-5 h-5 text-aegis-cyan" />
                    </div>
                    <div>
                        <h3 className="text-sm font-mono font-bold uppercase tracking-widest text-white/90">
                            {provider.label || providerLabel}
                        </h3>
                        {provider.active_models && provider.active_models.length > 0 && (
                            <p className="text-[10px] font-mono text-white/30 mt-0.5">
                                Models: {provider.active_models.slice(0, 2).join(', ')}
                                {provider.active_models.length > 2 && ` +${provider.active_models.length - 2}`}
                            </p>
                        )}
                    </div>
                </div>

                <div className="flex items-center gap-3">
                    {/* Badge de estado */}
                    <span className={`px-2 py-0.5 rounded text-[9px] font-mono uppercase border ${statusColors[status]}`}>
                        {status === 'active' ? t('status_active') : status === 'limited' ? t('status_limited') : t('status_inactive')}
                    </span>

                    {/* Acciones */}
                    <button onClick={() => onEdit(provider)} className="p-1.5 rounded hover:bg-white/10 transition-colors">
                        <Settings className="w-4 h-4 text-white/30 hover:text-white/70" />
                    </button>
                    <button 
                        onClick={() => {
                            if (window.confirm(t('confirm_delete_provider', { name: provider.label || providerLabel }))) {
                                onDelete(provider.key_id);
                            }
                        }} 
                        className="p-1.5 rounded hover:bg-red-500/10 transition-colors group"
                    >
                        <Trash2 className="w-4 h-4 text-white/30 group-hover:text-red-500" />
                    </button>
                </div>
            </div>
        </div>
    );
};

interface AddProviderPanelProps {
    onClose: () => void;
    onSaved: (provider: ProviderEntry) => void;
    tenantId: string;
    sessionKey: string;
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
                    {t('models_detected', { selected: selectedModels.length.toString(), total: models.length.toString() })}
                </label>
                <div className="flex gap-3">
                    <button onClick={() => onChange(filtered)} className="text-[9px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan uppercase tracking-widest">{t('all')}</button>
                    <button onClick={() => onChange([])} className="text-[9px] font-mono text-white/20 hover:text-white/40 uppercase tracking-widest">{t('none')}</button>
                </div>
            </div>

            <div className="relative">
                <input 
                    type="text" 
                    value={search} 
                    onChange={e => setSearch(e.target.value)} 
                    placeholder={t('filter_models')} 
                    className="w-full bg-black/40 border border-white/10 rounded-xl py-2.5 px-4 pl-10 text-[11px] font-mono focus:border-aegis-cyan/50 placeholder:text-white/10"
                />
                <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-white/20" />
            </div>

            <div className="max-h-60 overflow-y-auto space-y-1 pr-2 custom-scrollbar">
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

const ProviderModal: React.FC<{
    onClose: () => void;
    onSaved: () => void;
    tenantId: string;
    sessionKey: string;
    initialProvider?: ProviderEntry;
}> = ({ onClose, onSaved, tenantId, sessionKey, initialProvider }) => {
    const isEdit = !!initialProvider;
    const { t } = useTranslation();
    const [selectedProvider, setSelectedProvider] = useState<ProviderType>(
        (initialProvider?.provider as ProviderType) || 'openai'
    );
    const [apiKey, setApiKey] = useState('');
    const [showKey, setShowKey] = useState(false);
    const [isVerifying, setIsVerifying] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [models, setModels] = useState<string[]>([]);
    const [selectedModels, setSelectedModels] = useState<string[]>(initialProvider?.active_models || []);
    const [verifyError, setVerifyError] = useState<string | null>(null);
    const [step, setStep] = useState<'config' | 'models'>(isEdit ? 'models' : 'config');

    // Fetch models if in edit mode and models are empty
    React.useEffect(() => {
        if (isEdit && models.length === 0) {
            handleVerify();
        }
    }, [isEdit]);

    const handleVerify = async () => {
        setIsVerifying(true);
        setVerifyError(null);
        try {
            const res = await fetch('/api/providers/models', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId!,
                    'x-citadel-key': sessionKey!
                },
                body: JSON.stringify({
                    provider: selectedProvider,
                    api_key: apiKey || 'redacted', // Use placeholder if editing and key not changed
                    api_url: PROVIDER_PRESETS[selectedProvider].url
                })
            });
            if (res.ok) {
                const data = await res.json();
                setModels(data.models);
                if (!isEdit) setSelectedModels(data.models); 
                setStep('models');
            } else {
                const errData = await res.json();
                setVerifyError(errData.detail || t('invalid_api_key_error'));
            }
        } catch (err) {
            setVerifyError(t('provider_connection_error'));
        } finally {
            setIsVerifying(false);
        }
    };

    const handleSave = async () => {
        setIsSaving(true);
        try {
            const url = isEdit 
                ? `/api/router/keys/global/${initialProvider.key_id}`
                : '/api/router/keys/global';
            
            const res = await fetch(url, {
                method: isEdit ? 'PUT' : 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId!,
                    'x-citadel-key': sessionKey!
                },
                body: JSON.stringify({
                    provider: selectedProvider,
                    api_key: apiKey,
                    api_url: PROVIDER_PRESETS[selectedProvider].url,
                    models: selectedModels
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
            <div className="glass w-full max-w-2xl rounded-3xl border border-white/10 shadow-2xl overflow-hidden relative">
                <button 
                    onClick={onClose}
                    className="absolute top-6 right-6 p-2 rounded-full hover:bg-white/5 text-white/30 hover:text-white transition-colors"
                >
                    <X className="w-5 h-5" />
                </button>

                <div className="p-8">
                    <div className="flex items-center gap-4 mb-8">
                        <div className="p-3 rounded-2xl bg-aegis-cyan/10 border border-aegis-cyan/20">
                            {isEdit ? <Settings className="w-6 h-6 text-aegis-cyan" /> : <Plus className="w-6 h-6 text-aegis-cyan" />}
                        </div>
                        <div>
                            <h2 className="text-xl font-bold tracking-[0.2em] uppercase text-white">
                                {isEdit ? t('edit_provider') : t('link_provider')}
                            </h2>
                            <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest mt-1">
                                {isEdit ? initialProvider.label || selectedProvider : t('secure_neural_link')}
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
                                className="space-y-8"
                            >
                                <div className="grid grid-cols-4 sm:grid-cols-8 gap-3">
                                    {(Object.keys(PROVIDER_PRESETS) as ProviderType[]).map((key) => {
                                        const preset = PROVIDER_PRESETS[key];
                                        const isSelected = selectedProvider === key;
                                        return (
                                            <button 
                                                key={key} 
                                                disabled={isEdit}
                                                onClick={() => {
                                                    setSelectedProvider(key);
                                                    setVerifyError(null);
                                                }} 
                                                className={`p-3 rounded-xl border transition-all duration-300 flex flex-col items-center gap-2 group ${isSelected ? 'bg-aegis-cyan/20 border-aegis-cyan shadow-[0_0_15px_rgba(0,186,211,0.2)] scale-110' : 'bg-white/5 border-white/10 hover:border-white/30 hover:bg-white/10 opacity-60 hover:opacity-100'} ${isEdit && !isSelected ? 'hidden' : ''}`}
                                                title={preset.label}
                                            >
                                                <div className={`p-1.5 rounded-lg ${isSelected ? 'text-aegis-cyan' : 'text-white/40 group-hover:text-white/70'}`}>
                                                    {key === 'openai' && <Globe className="w-4 h-4" />}
                                                    {key === 'anthropic' && <Zap className="w-4 h-4" />}
                                                    {key === 'groq' && <Activity className="w-4 h-4" />}
                                                    {key === 'grok' && <Box className="w-4 h-4" />}
                                                    {key === 'openrouter' && <Globe className="w-4 h-4" />}
                                                    {key === 'ollama' && <Server className="w-4 h-4" />}
                                                    {key === 'gemini' && <Shield className="w-4 h-4" />}
                                                    {key === 'custom' && <Terminal className="w-4 h-4" />}
                                                </div>
                                            </button>
                                        );
                                    })}
                                </div>

                                <div className="space-y-6">
                                    <div className="space-y-2">
                                        <div className="flex justify-between items-center ml-1">
                                            <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest">{t('api_key_of', { name: PROVIDER_PRESETS[selectedProvider].label })}</label>
                                            {PROVIDER_PRESETS[selectedProvider].keyLink && (
                                                <a href={PROVIDER_PRESETS[selectedProvider].keyLink!} target="_blank" rel="noopener noreferrer" className="text-[9px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan flex items-center gap-1 transition-colors uppercase">
                                                    {t('get')} <Plus className="w-2.5 h-2.5" />
                                                </a>
                                            )}
                                        </div>
                                        <div className="relative">
                                            <input 
                                                type={showKey ? 'text' : 'password'} 
                                                value={apiKey} 
                                                onChange={(e) => setApiKey(e.target.value)} 
                                                placeholder={isEdit ? "••••••••••••••••" : "sk-..."} 
                                                className="w-full bg-black/40 border border-white/10 rounded-xl py-3 px-4 pr-10 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10 tracking-widest" 
                                            />
                                            <button 
                                                type="button" 
                                                onClick={() => setShowKey(!showKey)} 
                                                className="absolute right-3 top-1/2 -translate-y-1/2 text-white/20 hover:text-white/50 transition-colors"
                                            >
                                                {showKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                            </button>
                                        </div>
                                        {isEdit && <p className="text-[9px] font-mono text-white/20 uppercase ml-1 italic">{t('leave_empty_to_keep')}</p>}
                                    </div>

                                    {verifyError && (
                                        <div className="bg-red-500/10 border border-red-500/30 p-3 rounded-xl flex items-center gap-3">
                                            <Terminal className="w-4 h-4 text-red-500" />
                                            <p className="text-[10px] font-mono text-red-400 uppercase tracking-tight">{verifyError}</p>
                                        </div>
                                    )}

                                    <button 
                                        onClick={handleVerify}
                                        disabled={isVerifying || (!apiKey && !isEdit)}
                                        className={`w-full group relative overflow-hidden rounded-xl py-4 transition-all duration-500 ${isVerifying ? "bg-aegis-cyan/20 cursor-wait" : "bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30"}`}
                                    >
                                        <div className="relative z-10 flex items-center justify-center gap-3">
                                            {isVerifying ? <Loader2 className="w-4 h-4 text-aegis-cyan animate-spin" /> : <Shield className="w-4 h-4 text-aegis-cyan" />}
                                            <span className="text-[10px] font-mono font-bold tracking-[0.3em] uppercase text-aegis-cyan">
                                                {isVerifying ? t('verifying') : t('verify_load_models')}
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
                                className="space-y-6"
                            >
                                <ModelSelector models={models} selectedModels={selectedModels} onChange={setSelectedModels} />

                                <div className="flex gap-4 pt-4 border-t border-white/10">
                                    <button 
                                        onClick={() => setStep('config')}
                                        disabled={isSaving}
                                        className="flex-1 px-4 py-3 border border-white/10 rounded-xl text-[10px] font-mono text-white/40 hover:bg-white/5 transition-colors uppercase tracking-widest"
                                    >
                                        ← {isEdit ? t('edit_key') : t('cancel')}
                                    </button>
                                    <button 
                                        onClick={handleSave}
                                        disabled={isSaving || selectedModels.length === 0}
                                        className="flex-2 px-8 py-3 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-xl text-[10px] font-mono text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors uppercase tracking-widest font-bold flex items-center justify-center gap-2"
                                    >
                                        {isSaving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
                                        {isSaving ? t('saving') : isEdit ? t('update_provider') : t('activate_provider')}
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

const ProvidersTab: React.FC<{ tenantId: string | null; sessionKey: string | null }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [showModal, setShowModal] = useState(false);
    const [editingProvider, setEditingProvider] = useState<ProviderEntry | null>(null);
    const [providers, setProviders] = useState<ProviderEntry[]>([]);
    const [loading, setLoading] = useState(true);

    const fetchProviders = React.useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoading(true);
        try {
            const res = await fetch(`/api/router/keys/global`, {
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId!,
                    'x-citadel-key': sessionKey!
                }
            });
            if (res.ok) {
                const data = await res.json();
                setProviders(data.keys || []);
            }
        } catch (err) {
            console.error("Error fetching providers:", err);
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey]);

    const handleDelete = async (keyId: string) => {
        try {
            const res = await fetch(`/api/router/keys/global/${keyId}`, {
                method: 'DELETE',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId!,
                    'x-citadel-key': sessionKey!
                }
            });
            if (res.ok) {
                setProviders(prev => prev.filter(p => p.key_id !== keyId));
            }
        } catch (err) {
            console.error("Error deleting provider:", err);
        }
    };

    React.useEffect(() => {
        fetchProviders();
    }, [fetchProviders]);

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div className="flex justify-between items-center bg-white/2 p-6 rounded-2xl border border-white/5">
                <div className="flex items-center gap-4">
                    <div className="p-3 rounded-2xl bg-aegis-purple/10 border border-aegis-purple/20">
                        <Cpu className="w-6 h-6 text-aegis-purple" />
                    </div>
                    <div>
                        <h2 className="text-xl font-bold tracking-[0.2em] uppercase text-white">IA Providers</h2>
                        <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest mt-1">{t('keypool_pool')} {loading ? `(${t('loading')})` : providers.length > 0 ? `(${providers.length} ${t('linked')})` : t('empty')}</p>
                    </div>
                </div>
                <button 
                    onClick={() => {
                        setEditingProvider(null);
                        setShowModal(true);
                    }}
                    className="flex items-center gap-2 px-5 py-2.5 bg-aegis-cyan text-black rounded-xl font-bold text-[10px] uppercase tracking-widest hover:bg-aegis-cyan/80 transition-all hover:shadow-[0_0_20px_rgba(0,186,211,0.4)] active:scale-95"
                >
                    <Plus className="w-4 h-4" /> {t('add_provider')}
                </button>
            </div>

            {providers.length === 0 ? (
                <div className="glass p-20 rounded-3xl border border-dashed border-white/10 flex flex-col items-center justify-center text-center group transition-all duration-700 hover:border-aegis-cyan/30">
                    <div className="p-6 rounded-full bg-white/2 border border-white/5 mb-6 group-hover:scale-110 transition-transform duration-500">
                        <Shield className="w-12 h-12 text-white/10 group-hover:text-aegis-cyan/40 transition-colors" />
                    </div>
                    <h3 className="text-sm font-mono font-bold tracking-[0.3em] uppercase text-white/30 group-hover:text-white/50 transition-colors">{t('no_active_links')}</h3>
                    <p className="text-[10px] font-mono text-white/10 uppercase tracking-widest mt-4 leading-loose max-w-sm">
                        {t('add_provider_desc')}
                    </p>
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    {providers.map(provider => (
                        <ProviderCard 
                            key={provider.key_id} 
                            provider={provider} 
                            onDelete={handleDelete}
                            onEdit={(p) => {
                                setEditingProvider(p);
                                setShowModal(true);
                            }}
                        />
                    ))}
                    <button 
                        onClick={() => {
                            setEditingProvider(null);
                            setShowModal(true);
                        }}
                        className="glass p-5 rounded-2xl border border-dashed border-white/10 hover:border-aegis-cyan/30 hover:bg-aegis-cyan/5 transition-all flex flex-col items-center justify-center gap-3 group"
                    >
                        <div className="p-2 rounded-full bg-white/5 group-hover:bg-aegis-cyan/10 text-white/20 group-hover:text-aegis-cyan">
                            <Plus className="w-5 h-5" />
                        </div>
                        <span className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/30 group-hover:text-aegis-cyan/70">{t('link_more')}</span>
                    </button>
                </div>
            )}

            <AnimatePresence>
                {showModal && (
                    <ProviderModal 
                        onClose={() => setShowModal(false)} 
                        tenantId={tenantId!}
                        sessionKey={sessionKey!}
                        initialProvider={editingProvider || undefined}
                        onSaved={() => {
                            fetchProviders();
                            setShowModal(false);
                        }} 
                    />
                )}
            </AnimatePresence>
        </div>
    );
};

export default ProvidersTab;
