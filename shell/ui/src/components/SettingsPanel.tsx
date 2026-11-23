import React, { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Settings, X, Save, Cpu, Volume2, Globe, Check, Loader2, Sparkles, Shield, Server, Zap, Box, Cloud, Terminal, Activity, Info, Search, RefreshCw, Copy, FileText, Trash2, TrendingUp, Play, Square, Upload } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import { PROVIDER_PRESETS, ProviderType } from '../constants/enginePresets';
import TenantKeyManager from './RouterConfig/TenantKeyManager';
import UserPasswordChange from './UserPasswordChange';
import ConnectedAccountsTab from './ConnectedAccountsTab';
import TrainingPanel from './TrainingPanel';
import SirenConfigTab from './SirenConfigTab';
import { AssistantAvatar } from './AssistantAvatar';
import { AVATAR_PRESETS } from '../constants/avatarPresets';
import {
    VOICE_STYLE_PRESETS,
    loadVoicePrefs,
    saveVoicePrefs,
    speakNaturalText,
    stopSpeaking,
    getScoredVoices,
    ScoredVoice,
    VoiceGender,
} from '../audio/naturalVoice';

interface SettingsPanelProps {
    onClose: () => void;
    tenantId: string;
    sessionKey: string;
}

type TabId = 'perfil' | 'motor' | 'voz' | 'logs' | 'cuenta' | 'entrenamiento';

const TABS = (t: (key: string) => string): { id: TabId; label: string; icon: React.ReactNode }[] => [
    { id: 'perfil', label: t('tab_persona'), icon: <Sparkles className="w-4 h-4" /> },
    { id: 'motor', label: t('tab_motor'), icon: <Cpu className="w-4 h-4" /> },
    { id: 'entrenamiento', label: 'Entrenamiento', icon: <TrendingUp className="w-4 h-4" /> },
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

const PERSONALITY_CHIPS = [
    {
        label: 'Cariñosa y sarcástica',
        prompt: 'Tenés una personalidad cariñosa, cercana y cómplice, pero con un toque de sarcasmo inteligente y humor sutil. Hablás en español rioplatense natural.',
    },
    {
        label: 'Profesional y preciso',
        prompt: 'Sos un asistente ejecutivo, analítico, preciso y estructurado. Priorizás respuestas claras, accionables y bien fundamentadas.',
    },
    {
        label: 'Casual y amigable',
        prompt: 'Sos un asistente cálido, relajado y empático. Explicás todo de forma simple y conversacional, como un buen amigo.',
    },
    {
        label: 'Directo y sin rodeos',
        prompt: 'Sos ultra conciso y vas directo al grano sin introducciones innecesarias. Priorizás velocidad y brevedad.',
    },
    {
        label: 'Curioso y creativo',
        prompt: 'Tenés un enfoque creativo, entusiasta y explorador. Proponés ideas originales y miradas alternativas.',
    },
];

const PersonaTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const fetchAgentName = useAegisStore((s) => s.fetchAgentName);
    const [persona, setPersona] = useState<PersonaData>({ prompt: '', name: '' });
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

    // Estado de Avatar y Tipo de Voz
    const initialPrefs = loadVoicePrefs();
    const [avatarId, setAvatarId] = useState<string>(initialPrefs.avatarId || 'orb-cyan');
    const [customAvatarUrl, setCustomAvatarUrl] = useState<string>(initialPrefs.customAvatarUrl || '');
    const [voicePreset, setVoicePreset] = useState<string>(initialPrefs.voicePreset || 'female-warm');
    const [voiceGender, setVoiceGender] = useState<VoiceGender>(initialPrefs.voiceGender || 'female');
    const [voiceURI, setVoiceURI] = useState<string>(initialPrefs.voiceURI || '');
    const [availableVoices, setAvailableVoices] = useState<ScoredVoice[]>([]);
    const [isTestingVoice, setIsTestingVoice] = useState(false);
    const avatarInputRef = useRef<HTMLInputElement>(null);

    const refreshVoices = useCallback((genderOverride?: VoiceGender) => {
        const g = genderOverride ?? loadVoicePrefs().voiceGender ?? 'female';
        setAvailableVoices(getScoredVoices('es', g));
    }, []);

    useEffect(() => {
        refreshVoices(voiceGender);
        if (typeof window !== 'undefined' && window.speechSynthesis) {
            window.speechSynthesis.onvoiceschanged = () => refreshVoices();
        }
        return () => {
            if (typeof window !== 'undefined' && window.speechSynthesis) {
                window.speechSynthesis.onvoiceschanged = null;
            }
        };
    }, [refreshVoices, voiceGender]);

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

    const handleSelectAvatar = (id: string) => {
        setAvatarId(id);
        saveVoicePrefs({ avatarId: id });
    };

    const handleCustomAvatarUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = () => {
            const img = new Image();
            img.onload = () => {
                const canvas = document.createElement('canvas');
                const maxDim = 160;
                canvas.width = maxDim;
                canvas.height = maxDim;
                const ctx = canvas.getContext('2d');
                if (ctx) {
                    const minSide = Math.min(img.width, img.height);
                    const sx = (img.width - minSide) / 2;
                    const sy = (img.height - minSide) / 2;
                    ctx.drawImage(img, sx, sy, minSide, minSide, 0, 0, maxDim, maxDim);
                    const dataUrl = canvas.toDataURL('image/webp', 0.85);
                    setCustomAvatarUrl(dataUrl);
                    setAvatarId('custom');
                    saveVoicePrefs({ avatarId: 'custom', customAvatarUrl: dataUrl });
                }
            };
            img.src = reader.result as string;
        };
        reader.readAsDataURL(file);
        if (avatarInputRef.current) avatarInputRef.current.value = '';
    };

    const handleSelectVoicePreset = (presetId: string) => {
        const found = VOICE_STYLE_PRESETS.find((p) => p.id === presetId);
        if (!found) return;
        const next = saveVoicePrefs({
            voicePreset: found.id,
            voiceGender: found.gender,
            voiceURI: '',
            rate: found.rate,
            pitch: found.pitch,
        });
        setVoicePreset(next.voicePreset);
        setVoiceGender(next.voiceGender);
        setVoiceURI(next.voiceURI);
        refreshVoices(next.voiceGender);
    };

    const handleSelectSpecificVoice = (uri: string) => {
        const next = saveVoicePrefs({ voiceURI: uri, voicePreset: 'custom' });
        setVoiceURI(next.voiceURI);
        setVoicePreset(next.voicePreset);
    };

    const handleTestAssistantVoice = () => {
        if (isTestingVoice) {
            stopSpeaking();
            setIsTestingVoice(false);
            return;
        }
        setIsTestingVoice(true);
        const displayName = persona.name.trim() || 'tu asistente';
        const sample =
            voiceGender === 'male'
                ? `Hola, soy ${displayName}. Así suena mi voz masculina configurada para acompañarte todos los días.`
                : `Hola, soy ${displayName}. Así suena mi voz natural configurada para acompañarte todos los días.`;
        speakNaturalText(sample, () => setIsTestingVoice(false));
    };

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
                await fetchAgentName();
                setMessage({ type: 'success', text: '¡Personalización del asistente guardada!' });
            } else {
                const err = await res.json();
                setMessage({ type: 'error', text: err.detail || t('persona_save_error') });
            }
        } catch {
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
                await fetchAgentName();
                setMessage({ type: 'success', text: t('persona_reset') });
            } else {
                setMessage({ type: 'error', text: t('persona_save_error') });
            }
        } catch {
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

    const activePresetObj = VOICE_STYLE_PRESETS.find((p) => p.id === voicePreset);

    return (
        <div className="space-y-6">
            {/* Tarjeta de Vista Previa del Asistente */}
            <div className="p-4 sm:p-5 bg-gradient-to-r from-aegis-cyan/10 via-white/[0.03] to-purple-500/10 border border-white/15 rounded-2xl flex flex-wrap items-center justify-between gap-4">
                <div className="flex items-center gap-4">
                    <AssistantAvatar avatarId={avatarId} customAvatarUrl={customAvatarUrl} size="lg" />
                    <div>
                        <div className="flex items-center gap-2">
                            <h4 className="text-base font-bold text-white tracking-wide">
                                {persona.name.trim() || 'Aegis'}
                            </h4>
                            <span className="px-2 py-0.5 rounded-full text-[10px] font-mono bg-aegis-cyan/15 text-aegis-cyan border border-aegis-cyan/30">
                                {voiceGender === 'male' ? '♂ Voz Masculina' : '♀ Voz Femenina'}
                            </span>
                        </div>
                        <p className="text-xs text-white/50 mt-0.5 line-clamp-1">
                            {persona.prompt.trim() || 'Personalizá su nombre, avatar, timbre de voz y carácter'}
                        </p>
                    </div>
                </div>

                <button
                    type="button"
                    onClick={handleTestAssistantVoice}
                    className={`px-4 py-2.5 rounded-xl border font-mono text-xs font-bold uppercase tracking-wider flex items-center gap-2 transition-all ${
                        isTestingVoice
                            ? 'bg-red-500/20 border-red-500/40 text-red-300'
                            : 'bg-aegis-cyan/15 hover:bg-aegis-cyan/25 border-aegis-cyan/40 text-aegis-cyan'
                    }`}
                >
                    {isTestingVoice ? (
                        <>
                            <Square className="w-3.5 h-3.5 fill-current" /> Detener
                        </>
                    ) : (
                        <>
                            <Play className="w-3.5 h-3.5 fill-current" /> Escuchar Voz
                        </>
                    )}
                </button>
            </div>

            {/* 1. Nombre del Asistente */}
            <div>
                <label className="text-[11px] font-mono text-white/60 uppercase tracking-wider block mb-2">
                    1. Nombre del Asistente
                </label>
                <input
                    type="text"
                    value={persona.name || ''}
                    onChange={(e) => setPersona({ ...persona, name: e.target.value })}
                    placeholder="Ej. Aegis, Orion, Atlas, Nova..."
                    className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-4 text-sm text-white focus:border-aegis-cyan/60 focus:outline-none transition-all placeholder:text-white/25"
                />
            </div>

            {/* 2. Selector de Avatar */}
            <div className="space-y-2.5">
                <div className="flex items-center justify-between">
                    <label className="text-[11px] font-mono text-white/60 uppercase tracking-wider">
                        2. Avatar del Asistente
                    </label>
                    <input
                        ref={avatarInputRef}
                        type="file"
                        accept="image/*"
                        onChange={handleCustomAvatarUpload}
                        className="hidden"
                    />
                    <button
                        type="button"
                        onClick={() => avatarInputRef.current?.click()}
                        className="text-[11px] font-mono text-aegis-cyan hover:underline flex items-center gap-1.5"
                    >
                        <Upload className="w-3.5 h-3.5" /> Subir imagen propia
                    </button>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
                    {AVATAR_PRESETS.map((preset) => {
                        const selected = avatarId === preset.id;
                        return (
                            <button
                                key={preset.id}
                                type="button"
                                onClick={() => handleSelectAvatar(preset.id)}
                                className={`p-3 rounded-xl border text-left transition-all flex items-center gap-3 ${
                                    selected
                                        ? 'bg-white/10 border-aegis-cyan shadow-[0_0_15px_rgba(0,242,254,0.15)]'
                                        : 'bg-white/[0.02] border-white/10 hover:bg-white/[0.06]'
                                }`}
                            >
                                <AssistantAvatar avatarId={preset.id} size="sm" />
                                <div className="min-w-0 flex-1">
                                    <p className="text-xs font-mono font-bold text-white truncate">{preset.name}</p>
                                    <p className="text-[10px] text-white/40 truncate">{preset.desc}</p>
                                </div>
                            </button>
                        );
                    })}
                    {customAvatarUrl && (
                        <button
                            type="button"
                            onClick={() => handleSelectAvatar('custom')}
                            className={`p-3 rounded-xl border text-left transition-all flex items-center gap-3 ${
                                avatarId === 'custom'
                                    ? 'bg-white/10 border-aegis-cyan shadow-[0_0_15px_rgba(0,242,254,0.15)]'
                                    : 'bg-white/[0.02] border-white/10 hover:bg-white/[0.06]'
                            }`}
                        >
                            <AssistantAvatar avatarId="custom" customAvatarUrl={customAvatarUrl} size="sm" />
                            <div className="min-w-0 flex-1">
                                <p className="text-xs font-mono font-bold text-white truncate">Mi Imagen</p>
                                <p className="text-[10px] text-white/40 truncate">Avatar personalizado</p>
                            </div>
                        </button>
                    )}
                </div>
            </div>

            {/* 3. Tipo de Voz (Femenina / Masculina) */}
            <div className="space-y-2.5">
                <div className="flex items-center justify-between">
                    <label className="text-[11px] font-mono text-white/60 uppercase tracking-wider">
                        3. Tipo de Voz (Femenina / Masculina)
                    </label>
                    {activePresetObj && (
                        <span className="text-[11px] font-mono text-aegis-cyan">
                            {activePresetObj.label}
                        </span>
                    )}
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
                    {VOICE_STYLE_PRESETS.map((preset) => {
                        const isSelected = voicePreset === preset.id;
                        return (
                            <button
                                key={preset.id}
                                type="button"
                                onClick={() => handleSelectVoicePreset(preset.id)}
                                className={`p-3 rounded-xl border text-left transition-all flex flex-col gap-1 ${
                                    isSelected
                                        ? 'bg-aegis-cyan/15 border-aegis-cyan text-white'
                                        : 'bg-white/[0.02] border-white/10 text-white/60 hover:bg-white/[0.06] hover:text-white'
                                }`}
                            >
                                <span className="text-xs font-mono font-bold flex items-center gap-1">
                                    <span className={preset.gender === 'male' ? 'text-sky-400' : 'text-amber-300'}>
                                        {preset.gender === 'male' ? '♂' : '♀'}
                                    </span>
                                    {preset.label}
                                </span>
                                <span className="text-[10px] text-white/40 leading-tight">{preset.desc}</span>
                            </button>
                        );
                    })}
                </div>

                <div className="pt-1">
                    <select
                        value={voiceURI}
                        onChange={(e) => handleSelectSpecificVoice(e.target.value)}
                        className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 text-xs text-white/80 focus:border-aegis-cyan/60 focus:outline-none cursor-pointer"
                    >
                        <option value="">
                            ✨ Selección automática ({voiceGender === 'male' ? 'Mejor voz masculina neural' : 'Mejor voz femenina neural'})
                        </option>
                        {availableVoices.map((item) => {
                            const gTag = item.gender === 'male' ? '♂ ' : item.gender === 'female' ? '♀ ' : '';
                            return (
                                <option key={item.voice.voiceURI} value={item.voice.voiceURI}>
                                    {item.isNatural ? '✨ ' : ''}
                                    {gTag}
                                    {item.label} [{item.voice.lang}]
                                </option>
                            );
                        })}
                    </select>
                </div>
            </div>

            {/* 4. Personalidad */}
            <div className="space-y-2.5">
                <div className="flex justify-between items-center">
                    <label className="text-[11px] font-mono text-white/60 uppercase tracking-wider">
                        4. Personalidad y Estilo de Respuesta
                    </label>
                    <span className="text-[10px] font-mono text-white/25">
                        {persona.prompt.length} / 4000
                    </span>
                </div>

                <div className="flex flex-wrap gap-1.5">
                    {PERSONALITY_CHIPS.map((chip) => (
                        <button
                            key={chip.label}
                            type="button"
                            onClick={() => setPersona({ ...persona, prompt: chip.prompt })}
                            className="px-2.5 py-1 rounded-lg bg-white/5 hover:bg-aegis-cyan/15 border border-white/10 hover:border-aegis-cyan/40 text-[11px] text-white/70 hover:text-aegis-cyan transition-all"
                        >
                            + {chip.label}
                        </button>
                    ))}
                </div>

                <textarea
                    value={persona.prompt}
                    onChange={(e) => setPersona({ ...persona, prompt: e.target.value.slice(0, 4000) })}
                    placeholder="Describí cómo querés que te hable y se comporte tu asistente..."
                    rows={4}
                    className="w-full bg-black/50 border border-white/15 rounded-xl py-3 px-4 text-sm text-white focus:border-aegis-cyan/60 focus:outline-none transition-all placeholder:text-white/25 resize-none"
                />
            </div>

            {message && (
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    className={`p-3 rounded-xl flex items-center gap-2 text-xs font-mono ${
                        message.type === 'success'
                            ? 'bg-green-500/10 border border-green-500/30 text-green-400'
                            : 'bg-red-500/10 border border-red-500/30 text-red-400'
                    }`}
                >
                    {message.type === 'success' && <Check className="w-4 h-4" />}
                    {message.text}
                </motion.div>
            )}

            <div className="flex gap-3 pt-3 border-t border-white/10">
                <button
                    onClick={handleReset}
                    disabled={isSaving || (!persona.prompt && !persona.name)}
                    className="px-4 py-2.5 border border-white/10 rounded-xl text-[10px] font-mono text-white/40 hover:text-white hover:bg-white/5 transition-colors uppercase tracking-widest disabled:opacity-30"
                >
                    Restablecer
                </button>
                <button
                    onClick={handleSave}
                    disabled={isSaving}
                    className="flex-1 flex items-center justify-center gap-2 bg-aegis-cyan/20 border border-aegis-cyan/40 rounded-xl py-3 text-xs font-mono font-bold text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors uppercase tracking-widest disabled:opacity-50"
                >
                    {isSaving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
                    Guardar Personalización
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
    const { lastRoutingInfo, lastError } = useAegisStore(
        useShallow((s) => ({ lastRoutingInfo: s.lastRoutingInfo, lastError: s.lastError })),
    );
    const [globalProviders, setGlobalProviders] = useState<GlobalProviderEntry[]>([]);
    const [agyStatus, setAgyStatus] = useState<{
        sdk_installed: boolean;
        version: string;
        mode: string;
        use_antigravity_sdk: boolean;
        pending_outbox_count: number;
    } | null>(null);
    const [isTogglingAgy, setIsTogglingAgy] = useState(false);
    const [isLoading, setIsLoading] = useState(true);

    const getHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    }), [tenantId, sessionKey]);

    useEffect(() => {
        const fetchMotorData = async () => {
            try {
                const [provRes, agyRes] = await Promise.all([
                    fetch('/api/router/keys/global', { headers: getHeaders() }),
                    fetch('/api/antigravity/status', { headers: getHeaders() }),
                ]);
                if (provRes.ok) {
                    const data = await provRes.json();
                    setGlobalProviders(data.keys || []);
                }
                if (agyRes.ok) {
                    const agyData = await agyRes.json();
                    setAgyStatus(agyData);
                }
            } catch (err) {
                console.error('Fetch motor data error:', err);
            } finally {
                setIsLoading(false);
            }
        };
        fetchMotorData();
    }, [getHeaders]);

    const handleToggleAgySdk = async () => {
        if (!agyStatus || isTogglingAgy) return;
        const nextVal = !agyStatus.use_antigravity_sdk;
        setIsTogglingAgy(true);
        try {
            const res = await fetch('/api/antigravity/config', {
                method: 'POST',
                headers: getHeaders(),
                body: JSON.stringify({ use_antigravity_sdk: nextVal }),
            });
            if (res.ok) {
                setAgyStatus((prev) => (prev ? { ...prev, use_antigravity_sdk: nextVal } : prev));
            }
        } catch (err) {
            console.error('Toggle Antigravity SDK error:', err);
        } finally {
            setIsTogglingAgy(false);
        }
    };

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

            {/* CORE-386: Google Antigravity SDK Card */}
            <div className="p-4 rounded-xl border border-aegis-purple/30 bg-aegis-purple/10 space-y-3">
                <div className="flex items-start justify-between gap-4">
                    <div className="space-y-1">
                        <div className="flex items-center gap-2">
                            <Zap className="w-4 h-4 text-aegis-cyan" />
                            <h3 className="text-xs font-mono font-bold uppercase tracking-wider text-white">
                                Google Antigravity SDK (Subagentes y Tareas Complejas)
                            </h3>
                            {agyStatus?.sdk_installed ? (
                                <span className="px-2 py-0.5 rounded-full text-[9px] font-mono bg-emerald-500/15 border border-emerald-500/30 text-emerald-300">
                                    SDK v{agyStatus.version} Detectado
                                </span>
                            ) : (
                                <span className="px-2 py-0.5 rounded-full text-[9px] font-mono bg-amber-500/15 border border-amber-500/30 text-amber-300">
                                    No detectado
                                </span>
                            )}
                        </div>
                        <p className="text-[11px] text-white/60 leading-relaxed">
                            Cuando está activo, tu asistente delega proyectos y subagentes directamente a{' '}
                            <span className="text-aegis-cyan font-mono">google-antigravity</span>, aprovechando los créditos de tu cuenta de Antigravity en lugar de consumir tokens de tu API de chat.
                        </p>
                    </div>

                    <button
                        type="button"
                        onClick={handleToggleAgySdk}
                        disabled={isTogglingAgy}
                        className={`px-3.5 py-2 rounded-xl border text-xs font-mono font-bold uppercase tracking-wider transition-all shrink-0 ${
                            agyStatus?.use_antigravity_sdk
                                ? 'bg-aegis-cyan/20 border-aegis-cyan text-aegis-cyan shadow-[0_0_12px_rgba(0,242,254,0.2)]'
                                : 'bg-white/5 border-white/15 text-white/50 hover:text-white'
                        }`}
                    >
                        {agyStatus?.use_antigravity_sdk ? '⚡ Activo' : 'Desactivado'}
                    </button>
                </div>
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

interface ChatMessage {
    timestamp: string;
    role: string;
    content: string;
}

const LogsTab: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const { clearHistory, clearAgentTraces } = useAegisStore(
        useShallow((s) => ({
            clearHistory: s.clearHistory,
            clearAgentTraces: s.clearAgentTraces,
        })),
    );
    const [subTab, setSubTab] = useState<'history' | 'traces'>('history');
    const [logs, setLogs] = useState<(ChatMessage | string)[]>([]);
    const [searchTerm, setSearchTerm] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [copied, setCopied] = useState(false);

    const handleClear = async () => {
        if (!window.confirm(t('logs_clear_confirm'))) return;
        setIsLoading(true);
        if (subTab === 'history') {
            await clearHistory();
        } else {
            await clearAgentTraces();
        }
        await fetchLogs();
    };

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
            textToCopy = (logs as ChatMessage[])
                .map((msg) => `[${msg.timestamp}] ${msg.role.toUpperCase()}: ${msg.content}`)
                .join('\n');
        } else {
            textToCopy = (logs as string[]).join('\n');
        }
        navigator.clipboard.writeText(textToCopy);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const filteredLogs = logs.filter((log) => {
        if (!searchTerm) return true;
        const term = searchTerm.toLowerCase();
        if (typeof log === 'string') {
            return log.toLowerCase().includes(term);
        } else {
            return (
                log.content.toLowerCase().includes(term) ||
                log.role.toLowerCase().includes(term) ||
                log.timestamp.toLowerCase().includes(term)
            );
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
                    <button
                        onClick={handleClear}
                        disabled={isLoading || logs.length === 0}
                        className="flex items-center gap-2 px-3 py-2 bg-red-500/10 border border-red-500/20 hover:bg-red-500/20 disabled:opacity-50 text-red-400 hover:text-red-300 rounded-lg transition-all text-xs font-mono uppercase tracking-wider"
                    >
                        <Trash2 className="w-3.5 h-3.5" />
                        <span>{t('logs_clear')}</span>
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
                            (filteredLogs as ChatMessage[]).map((msg, idx) => (
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
                            className="h-full"
                        >
                            {activeTab === 'perfil' && <PersonaTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'motor' && <MotorTab tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'entrenamiento' && <TrainingPanel tenantId={tenantId} sessionKey={sessionKey} />}
                            {activeTab === 'voz' && <SirenConfigTab />}
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