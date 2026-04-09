import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Cpu, Globe, Zap, Box, Server, ShieldCheck, Eye, EyeOff, Terminal, Check, ArrowRight, Activity, AlertTriangle } from 'lucide-react';
import { useAegisStore, TaskTypeValue } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import { PROVIDER_PRESETS, ProviderType } from '../constants/enginePresets';

interface TaskTypeOption {
    id: TaskTypeValue;
    icon: string;
    label: string;
    desc: string;
}

const TASK_TYPES = (t: (key: string) => string): TaskTypeOption[] => [
    { id: 'chat', icon: 'C', label: 'Chat', desc: t('chat_desc') },
    { id: 'coding', icon: 'K', label: 'Coding', desc: t('coding_desc') },
    { id: 'planning', icon: 'P', label: 'Planning', desc: t('planning_desc') },
    { id: 'analysis', icon: 'A', label: 'Analysis', desc: t('analysis_desc') },
    { id: 'summarization', icon: 'S', label: 'Summarization', desc: t('summarization_desc') },
];

interface GlobalEngine {
    configured: boolean;
    provider?: string;
    model_name?: string;
    api_url?: string;
}

const EngineSetupWizard: React.FC = () => {
    const { t } = useTranslation();
    const { system_metrics, configureEngine, tenantId, taskType, setTaskType, setEngineConfigured } = useAegisStore();
    const taskTypeList = TASK_TYPES(t);
    const [globalEngine, setGlobalEngine] = useState<GlobalEngine | null>(null);
    const [showCustom, setShowCustom] = useState(false);
    const [showTaskTypeStep, setShowTaskTypeStep] = useState(false);

    const [selectedProvider, setSelectedProvider] = useState<ProviderType>('openai');
    const [apiUrl, setApiUrl] = useState<string>(PROVIDER_PRESETS.openai.url);
    const [model, setModel] = useState<string>(PROVIDER_PRESETS.openai.model);
    const [apiKey, setApiKey] = useState('');
    const [showKey, setShowKey] = useState(false);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const vramMb = system_metrics?.vram_total_mb ?? 0;
    const hasLowVRAM = vramMb < 4000;

    useEffect(() => {
        const checkGlobal = async () => {
            try {
                const res = await fetch('/api/engine/status');
                if (res.ok) {
                    const data = await res.json();
                    setGlobalEngine(data);
                    if (!data.configured) setShowCustom(true);
                }
            } catch (err) {
                console.error('Failed to check global engine:', err);
                setShowCustom(true);
            }
        };
        checkGlobal();
    }, []);

    const handleSelectProvider = (key: ProviderType) => {
        setSelectedProvider(key);
        const preset = PROVIDER_PRESETS[key];
        setApiUrl(preset.url);
        setModel(preset.model);
        if (key === 'ollama') setApiKey('');
    };

    const handleUseGlobal = async () => {
        if (!globalEngine?.configured) return;
        setIsSubmitting(true);
        setError(null);

        // Usar la URL y modelo reales del engine global — no hardcodear
        const engineApiUrl = globalEngine.api_url
            ?? PROVIDER_PRESETS[globalEngine.provider as ProviderType]?.url
            ?? '';
        const engineModel = globalEngine.model_name ?? '';
        const engineProvider = globalEngine.provider ?? 'custom';

        const success = await configureEngine(engineApiUrl, engineModel, '', engineProvider);
        if (success) {
            setShowTaskTypeStep(true);
        } else {
            setIsSubmitting(false);
            setError(t('global_engine_link_error'));
        }
    };

    const handleSubmitCustom = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);
        if (selectedProvider !== 'ollama' && selectedProvider !== 'custom' && !apiKey) {
            setError(t('api_key_required'));
            return;
        }

        setIsSubmitting(true);
        const success = await configureEngine(apiUrl, model, apiKey, selectedProvider);
        if (success) {
            setShowTaskTypeStep(true);
        } else {
            setIsSubmitting(false);
            setError(t('engine_config_error'));
        }
    };

    if (showTaskTypeStep) {
        return (
            <div className="min-h-screen bg-black flex items-center justify-center p-4 text-white relative">
                <motion.div
                    initial={{ opacity: 0, scale: 0.95 }}
                    animate={{ opacity: 1, scale: 1 }}
                    className="w-full max-w-lg z-10 bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-8"
                >
                    <div className="flex items-center gap-3 mb-2">
                        <Check className="w-6 h-6 text-green-400" />
                        <h2 className="text-lg tracking-[0.2em] font-bold text-white uppercase">{t('engine_activated')}</h2>
                    </div>
                    <p className="text-xs font-mono text-white/40 mb-8 uppercase tracking-widest">
                        {t('select_task_type_desc')}
                    </p>
                    <div className="grid grid-cols-1 gap-3 mb-8">
                        {taskTypeList.map((tt) => (
                            <button
                                key={tt.id}
                                onClick={() => setTaskType(tt.id)}
                                className={`flex items-center gap-4 p-4 rounded-xl border transition-all duration-200 text-left ${
                                    taskType === tt.id
                                        ? 'bg-aegis-cyan/20 border-aegis-cyan text-aegis-cyan'
                                        : 'bg-white/5 border-white/10 hover:border-white/30 text-white/60'
                                }`}
                            >
                                <span className="w-8 h-8 flex items-center justify-center rounded-lg bg-black/40 font-mono font-bold text-sm">
                                    {tt.icon}
                                </span>
                                <div>
                                    <div className="text-xs font-mono font-bold uppercase tracking-widest">{tt.label}</div>
                                    <div className="text-[10px] font-mono opacity-60 mt-0.5">{tt.desc}</div>
                                </div>
                                {taskType === tt.id && <Check className="w-4 h-4 ml-auto" />}
                            </button>
                        ))}
                    </div>
                    <button
                        onClick={() => setEngineConfigured(true)}
                        className="w-full flex items-center justify-center gap-2 bg-gradient-to-r from-aegis-cyan/80 to-aegis-purple/80 hover:from-aegis-cyan hover:to-aegis-purple text-white font-bold py-3 px-4 rounded-lg uppercase tracking-[0.2em] text-[10px] transition-all"
                    >
                        {t('start_aegis_shell')} <ArrowRight className="w-3 h-3" />
                    </button>
                </motion.div>
                <div className="fixed bottom-6 flex items-center gap-2 text-[8px] font-mono text-white/20 uppercase tracking-[0.3em]">
                    <ShieldCheck className="w-3 h-3" />
                    <span>Ring-0 Citadel Interface — All credentials encrypted locally</span>
                </div>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-black flex items-center justify-center p-4 text-white relative">
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-5xl z-10 grid grid-cols-1 md:grid-cols-3 gap-6 bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl p-8"
            >
                {/* Sidebar */}
                <div className="md:col-span-1 border-r border-white/5 pr-6 flex flex-col justify-center">
                    <div className="flex items-center gap-3 mb-6">
                        <Cpu className="w-8 h-8 text-aegis-cyan" />
                        <div>
                            <h2 className="text-2xl tracking-[0.2em] font-bold text-white uppercase">Cognitive Setup</h2>
                            <p className="text-xs font-mono text-aegis-cyan/60 leading-none mt-1">Enclave: {tenantId}</p>
                        </div>
                    </div>

                    <p className="text-xs text-white/50 mb-8 leading-relaxed font-mono uppercase tracking-widest">
                        Aegis Neural Kernel requiere una red neuronal activa para procesar misiones y ejecutar protocolos.
                    </p>

                    <div className="space-y-4 mb-8">
                        <div className="flex justify-between items-center text-[10px] font-mono border-b border-white/10 pb-2 uppercase tracking-widest">
                            <span className="text-white/40">Hardware Profile</span>
                            <span className={hasLowVRAM ? 'text-yellow-500' : vramMb > 0 ? 'text-green-500' : 'text-white/30'}>
                                {vramMb > 0 ? (hasLowVRAM ? 'Standard CPU' : 'Optimized GPU') : 'Cloud Mode'}
                            </span>
                        </div>
                        {vramMb > 0 && (
                            <div className="flex justify-between items-center text-[10px] font-mono border-b border-white/10 pb-2 uppercase tracking-widest">
                                <span className="text-white/40">{t('vram_detected')}</span>
                                <span className={hasLowVRAM ? 'text-yellow-500' : 'text-green-500'}>
                                    {vramMb} MB
                                </span>
                            </div>
                        )}
                    </div>

                    {hasLowVRAM && vramMb > 0 && (
                        <motion.div
                            initial={{ opacity: 0, x: -20 }}
                            animate={{ opacity: 1, x: 0 }}
                            className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-4 flex items-start gap-3"
                        >
                            <AlertTriangle className="w-4 h-4 text-yellow-500 flex-shrink-0 mt-0.5" />
                            <div className="text-[10px] text-yellow-500/90 leading-relaxed font-mono uppercase tracking-tight">
                                <strong>{t('vram_limited')}</strong>
                                <br />{t('low_vram_warning')}
                            </div>
                        </motion.div>
                    )}
                </div>

                {/* Main Content */}
                <div className="md:col-span-2 space-y-6">
                    <AnimatePresence mode="wait">
                        {!showCustom && globalEngine?.configured ? (
                            <motion.div
                                key="global"
                                initial={{ opacity: 0, x: 20 }}
                                animate={{ opacity: 1, x: 0 }}
                                exit={{ opacity: 0, x: -20 }}
                                className="flex flex-col items-center justify-center h-full py-8 text-center"
                            >
                                <div className="p-4 bg-aegis-cyan/10 rounded-full border border-aegis-cyan/30 mb-6">
                                    <Globe className="w-12 h-12 text-aegis-cyan" />
                                </div>
                                <h3 className="text-xl font-bold tracking-widest uppercase mb-2">{t('system_engine_configured')}</h3>
                                <p className="text-sm text-white/40 font-mono mb-8 uppercase tracking-widest italic leading-relaxed max-w-xs mx-auto">
                                    {t('global_engine_desc')}
                                </p>

                                <button
                                    onClick={handleUseGlobal}
                                    disabled={isSubmitting}
                                    className="w-full max-w-sm flex items-center justify-center gap-3 bg-aegis-cyan hover:bg-aegis-cyan/80 text-black font-bold py-4 px-6 rounded-xl uppercase tracking-[0.2em] text-xs transition-all transform hover:scale-[1.02] active:scale-[0.98] shadow-[0_0_30px_rgba(0,186,211,0.2)] disabled:opacity-50"
                                >
                                    {isSubmitting ? t('connecting') : t('use_system_engine')}
                                    <Check className="w-4 h-4" />
                                </button>

                                {error && (
                                    <div className="mt-4 bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-2 max-w-sm w-full">
                                        <Terminal className="w-4 h-4 text-red-500 flex-shrink-0" />
                                        <span className="text-[10px] font-mono text-red-400 uppercase leading-tight">{error}</span>
                                    </div>
                                )}

                                <button
                                    onClick={() => setShowCustom(true)}
                                    className="mt-6 text-[10px] font-mono text-white/20 hover:text-aegis-cyan transition-colors uppercase tracking-[0.3em]"
                                >
                                    {t('configure_custom_engine')}
                                </button>
                            </motion.div>
                        ) : (
                            <motion.div
                                key="custom"
                                initial={{ opacity: 0, x: 20 }}
                                animate={{ opacity: 1, x: 0 }}
                                exit={{ opacity: 0, x: -20 }}
                                className="space-y-6"
                            >
                                <div className="flex items-center justify-between mb-4">
                                    <h3 className="text-sm font-bold tracking-widest uppercase">{t('provider_selection')}</h3>
                                    {globalEngine?.configured && (
                                        <button
                                            onClick={() => setShowCustom(false)}
                                            className="text-[9px] font-mono text-white/30 hover:text-white transition-colors uppercase"
                                        >
                                            {t('back_to_global')}
                                        </button>
                                    )}
                                </div>

                                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                                    {(Object.keys(PROVIDER_PRESETS) as ProviderType[]).map((key) => {
                                        const preset = PROVIDER_PRESETS[key];
                                        const isSelected = selectedProvider === key;
                                        return (
                                            <button
                                                key={key}
                                                onClick={() => handleSelectProvider(key)}
                                                className={`p-3 rounded-xl border transition-all duration-300 flex flex-col items-center gap-2 group ${isSelected
                                                    ? 'bg-aegis-cyan/20 border-aegis-cyan'
                                                    : 'bg-white/5 border-white/10 hover:border-white/30'
                                                    }`}
                                            >
                                                <div className={`p-2 rounded-lg ${isSelected ? 'bg-aegis-cyan/30' : 'bg-white/5 group-hover:bg-white/10'}`}>
                                                    {key === 'openai' && <Globe className="w-4 h-4" />}
                                                    {key === 'anthropic' && <Zap className="w-4 h-4" />}
                                                    {key === 'groq' && <Activity className="w-4 h-4" />}
                                                    {key === 'grok' && <Box className="w-4 h-4" />}
                                                    {key === 'openrouter' && <Globe className="w-4 h-4" />}
                                                    {key === 'ollama' && <Server className="w-4 h-4" />}
                                                    {key === 'gemini' && <ShieldCheck className="w-4 h-4" />}
                                                    {key === 'custom' && <Terminal className="w-4 h-4" />}
                                                </div>
                                                <span className={`text-[8px] font-mono uppercase font-bold tracking-widest ${isSelected ? 'text-aegis-cyan' : 'text-white/30'}`}>
                                                    {preset.label}
                                                </span>
                                            </button>
                                        );
                                    })}
                                </div>

                                <form onSubmit={handleSubmitCustom} className="space-y-4 pt-4 border-t border-white/5">
                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                        <div className="space-y-1">
                                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">API URL</label>
                                            <input
                                                type="url"
                                                value={apiUrl}
                                                onChange={(e) => setApiUrl(e.target.value)}
                                                className="w-full bg-black/60 border border-white/10 rounded-lg py-3 px-4 text-xs font-mono focus:border-aegis-cyan/50"
                                                required
                                            />
                                        </div>
                                        <div className="space-y-1">
                                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('model')}</label>
                                            <input
                                                type="text"
                                                value={model}
                                                onChange={(e) => setModel(e.target.value)}
                                                className="w-full bg-black/60 border border-white/10 rounded-lg py-3 px-4 text-xs font-mono focus:border-aegis-cyan/50"
                                                required
                                            />
                                        </div>
                                    </div>

                                    {(selectedProvider !== 'ollama' && selectedProvider !== 'custom') && (
                                        <div className="space-y-2">
                                            <div className="flex justify-between items-center px-1">
                                                <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest">API Key</label>
                                                {PROVIDER_PRESETS[selectedProvider].keyLink && (
                                                    <a
                                                        href={PROVIDER_PRESETS[selectedProvider].keyLink!}
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        className="text-[9px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan transition-colors"
                                                    >
                                                        {t('get_key')}
                                                    </a>
                                                )}
                                            </div>
                                            <div className="relative">
                                                <input
                                                    type={showKey ? 'text' : 'password'}
                                                    value={apiKey}
                                                    onChange={(e) => setApiKey(e.target.value)}
                                                    className="w-full bg-black/60 border border-white/10 rounded-lg py-3 px-4 pr-10 text-xs font-mono focus:border-aegis-cyan/50 tracking-widest"
                                                    required
                                                />
                                                <button
                                                    type="button"
                                                    onClick={() => setShowKey(!showKey)}
                                                    className="absolute right-3 top-1/2 -translate-y-1/2 text-white/20 hover:text-white/50"
                                                >
                                                    {showKey ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                                                </button>
                                            </div>
                                        </div>
                                    )}

                                    {error && (
                                        <div className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-2">
                                            <Terminal className="w-4 h-4 text-red-500 flex-shrink-0" />
                                            <span className="text-[10px] font-mono text-red-400 uppercase leading-tight">{error}</span>
                                        </div>
                                    )}

                                    <button
                                        type="submit"
                                        disabled={isSubmitting}
                                        className="w-full flex items-center justify-center gap-2 bg-gradient-to-r from-aegis-cyan/80 to-aegis-purple/80 hover:from-aegis-cyan hover:to-aegis-purple text-white font-bold py-4 px-4 rounded-xl uppercase tracking-[0.2em] text-[10px] transition-all transform active:scale-[0.98] disabled:opacity-50 mt-4 shadow-lg shadow-aegis-cyan/10"
                                    >
                                        {isSubmitting ? t('starting_handshake') : t('activate_my_engine')}
                                        <ArrowRight className="w-3 h-3" />
                                    </button>
                                </form>
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>
            </motion.div>

            <div className="fixed bottom-6 flex items-center gap-2 text-[8px] font-mono text-white/20 uppercase tracking-[0.3em]">
                <ShieldCheck className="w-3 h-3" />
                <span>Ring-0 Citadel Interface — All credentials encrypted locally</span>
            </div>
        </div>
    );
};

export default EngineSetupWizard;
