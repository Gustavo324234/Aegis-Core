import React, { useState, useEffect, useCallback } from 'react';
import { motion } from 'framer-motion';
import {
    Mic,
    Check,
    Eye,
    EyeOff,
    Save,
    RefreshCw,
    Volume2,
    VolumeX,
    Sparkles,
    Play,
    Square,
    ChevronDown,
    ChevronUp,
    Sliders,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import {
    getScoredVoices,
    loadVoicePrefs,
    saveVoicePrefs,
    speakNaturalText,
    stopSpeaking,
    ScoredVoice,
    VoiceGender,
    VOICE_STYLE_PRESETS,
} from '../audio/naturalVoice';

const MIC_LANGUAGES = [
    { id: 'es-AR', label: 'Español (Argentina / Rioplatense)' },
    { id: 'es-MX', label: 'Español (Latinoamérica / México)' },
    { id: 'es-ES', label: 'Español (España)' },
    { id: 'en-US', label: 'English (US)' },
];

const SirenConfigTab: React.FC = () => {
    const { tenantId, sessionKey, voiceEnabled, setVoiceEnabled, fetchSirenConfig } = useAegisStore();

    const initialPrefs = loadVoicePrefs();
    const [voiceURI, setVoiceURI] = useState<string>(initialPrefs.voiceURI);
    const [voiceGender, setVoiceGender] = useState<VoiceGender>(initialPrefs.voiceGender || 'female');
    const [voicePreset, setVoicePreset] = useState<string>(initialPrefs.voicePreset || 'female-warm');
    const [rate, setRate] = useState<number>(initialPrefs.rate);
    const [pitch, setPitch] = useState<number>(initialPrefs.pitch);
    const [micLang, setMicLang] = useState<string>(initialPrefs.lang || 'es-AR');
    const [autoContinueMic, setAutoContinueMic] = useState<boolean>(initialPrefs.autoContinueMic);
    const [availableVoices, setAvailableVoices] = useState<ScoredVoice[]>([]);
    const [isTestingVoice, setIsTestingVoice] = useState(false);
    const [savedToast, setSavedToast] = useState(false);

    // Opciones avanzadas cloud (plegadas por defecto)
    const [showAdvancedCloud, setShowAdvancedCloud] = useState(false);
    const [provider, setProvider] = useState('browser');
    const [apiKey, setApiKey] = useState('');
    const [sttProvider, setSttProvider] = useState('browser');
    const [sttApiKey, setSttApiKey] = useState('');
    const [showKey, setShowKey] = useState(false);
    const [showSttKey, setShowSttKey] = useState(false);
    const [isSavingCloud, setIsSavingCloud] = useState(false);

    const refreshBrowserVoices = useCallback((genderOverride?: VoiceGender) => {
        const g = genderOverride ?? loadVoicePrefs().voiceGender ?? 'female';
        const sorted = getScoredVoices('es', g);
        setAvailableVoices(sorted);
    }, []);

    useEffect(() => {
        refreshBrowserVoices(voiceGender);
        if (typeof window !== 'undefined' && window.speechSynthesis) {
            window.speechSynthesis.onvoiceschanged = () => refreshBrowserVoices();
        }
        return () => {
            if (typeof window !== 'undefined' && window.speechSynthesis) {
                window.speechSynthesis.onvoiceschanged = null;
            }
        };
    }, [refreshBrowserVoices, voiceGender]);

    useEffect(() => {
        if (!tenantId || !sessionKey) return;
        fetch('/api/siren/config', {
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        })
            .then((r) => (r.ok ? r.json() : null))
            .then((data) => {
                if (data) {
                    setProvider(data.provider || 'browser');
                    setApiKey(data.api_key || '');
                    setSttProvider(data.stt_provider || 'browser');
                    setSttApiKey(data.stt_api_key || '');
                }
            })
            .catch(() => {});
    }, [tenantId, sessionKey]);

    const handleUpdatePrefs = (
        patch: Partial<{
            voiceURI: string;
            voiceGender: VoiceGender;
            voicePreset: string;
            rate: number;
            pitch: number;
            lang: string;
            autoContinueMic: boolean;
        }>
    ) => {
        const next = saveVoicePrefs(patch);
        setVoiceURI(next.voiceURI);
        setVoiceGender(next.voiceGender);
        setVoicePreset(next.voicePreset);
        setRate(next.rate);
        setPitch(next.pitch);
        setMicLang(next.lang);
        setAutoContinueMic(next.autoContinueMic);
        refreshBrowserVoices(next.voiceGender);
        setSavedToast(true);
        setTimeout(() => setSavedToast(false), 2000);
    };

    const handleSelectPreset = (presetId: string) => {
        const found = VOICE_STYLE_PRESETS.find((p) => p.id === presetId);
        if (!found) return;
        handleUpdatePrefs({
            voicePreset: found.id,
            voiceGender: found.gender,
            voiceURI: '', // Selección automática de la mejor voz para ese género
            rate: found.rate,
            pitch: found.pitch,
        });
    };

    const handleTestVoice = () => {
        if (isTestingVoice) {
            stopSpeaking();
            setIsTestingVoice(false);
            return;
        }
        setIsTestingVoice(true);
        const sampleText = micLang.startsWith('en')
            ? "Hi! I'm your personal assistant. How can I help you today?"
            : voiceGender === 'male'
            ? '¡Hola! Soy tu asistente personal. Configuré mi voz masculina para sonar clara, natural y sin tono robótico.'
            : '¡Hola! Soy tu asistente personal. Configuré mi voz femenina para sonar cálida, fluida y natural.';
        speakNaturalText(sampleText, () => {
            setIsTestingVoice(false);
        });
    };

    const handleSaveCloudConfig = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!tenantId || !sessionKey) return;
        setIsSavingCloud(true);
        try {
            const response = await fetch('/api/siren/config', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({
                    provider,
                    api_key: apiKey,
                    voice_id: '',
                    stt_provider: sttProvider,
                    stt_api_key: sttApiKey,
                }),
            });
            if (response.ok) {
                await fetchSirenConfig();
                setSavedToast(true);
                setTimeout(() => setSavedToast(false), 2500);
            }
        } finally {
            setIsSavingCloud(false);
        }
    };

    const topVoice = availableVoices[0];
    const hasNativeMaleVoice = availableVoices.some((v) => v.gender === 'male');

    return (
        <div className="space-y-6 max-h-[70vh] overflow-y-auto pr-2 custom-scrollbar">
            {/* Tarjeta Principal: Voz Natural Simplificada */}
            <div className="glass p-6 rounded-2xl border border-white/10 shadow-2xl relative overflow-hidden space-y-6">
                <div className="flex flex-wrap items-center justify-between gap-4 border-b border-white/10 pb-5">
                    <div className="flex items-center gap-3.5">
                        <div className="p-3 rounded-xl bg-aegis-cyan/10 border border-aegis-cyan/30">
                            <Sparkles className="w-6 h-6 text-aegis-cyan" />
                        </div>
                        <div>
                            <h2 className="text-base sm:text-lg font-bold tracking-wider uppercase text-white">
                                Tipo de Voz y Micrófono
                            </h2>
                            <p className="text-xs text-white/40 mt-0.5">
                                Elegí entre voz femenina o masculina, estilo de entonación y conversación manos libres
                            </p>
                        </div>
                    </div>

                    <button
                        type="button"
                        onClick={() => setVoiceEnabled(!voiceEnabled)}
                        className={`flex items-center gap-2.5 px-4 py-2.5 rounded-xl border text-xs font-mono font-bold uppercase tracking-wider transition-all ${
                            voiceEnabled
                                ? 'bg-aegis-cyan/20 border-aegis-cyan text-aegis-cyan shadow-[0_0_20px_rgba(0,242,254,0.15)]'
                                : 'bg-white/5 border-white/10 text-white/50 hover:text-white hover:bg-white/10'
                        }`}
                    >
                        {voiceEnabled ? <Volume2 className="w-4 h-4" /> : <VolumeX className="w-4 h-4" />}
                        {voiceEnabled ? 'Voz Activada' : 'Voz Silenciada'}
                    </button>
                </div>

                {/* Perfiles Rápidos de Tipo de Voz (Femenina / Masculina) */}
                <div className="space-y-2.5">
                    <label className="text-xs font-mono text-white/60 uppercase tracking-wider block">
                        Estilo y Timbre de Voz
                    </label>
                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
                        {VOICE_STYLE_PRESETS.map((preset) => {
                            const isSelected = voicePreset === preset.id;
                            return (
                                <button
                                    key={preset.id}
                                    type="button"
                                    onClick={() => handleSelectPreset(preset.id)}
                                    className={`p-3.5 rounded-xl border text-left transition-all flex flex-col justify-between gap-1.5 ${
                                        isSelected
                                            ? 'bg-aegis-cyan/15 border-aegis-cyan text-white shadow-[0_0_20px_rgba(0,242,254,0.12)]'
                                            : 'bg-white/[0.03] border-white/10 text-white/60 hover:bg-white/[0.06] hover:text-white'
                                    }`}
                                >
                                    <div className="flex items-center justify-between">
                                        <span className="text-xs font-mono font-bold uppercase tracking-wide flex items-center gap-1.5">
                                            <span className={preset.gender === 'male' ? 'text-sky-400' : 'text-amber-300'}>
                                                {preset.gender === 'male' ? '♂' : '♀'}
                                            </span>
                                            {preset.label}
                                        </span>
                                        {isSelected && <Check className="w-3.5 h-3.5 text-aegis-cyan" />}
                                    </div>
                                    <p className="text-[11px] text-white/40 leading-snug">{preset.desc}</p>
                                </button>
                            );
                        })}
                    </div>
                </div>

                {/* Selector específico de Voz del Sistema + Botón Probar */}
                <div className="space-y-2">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                        <label className="text-xs font-mono text-white/60 uppercase tracking-wider">
                            Motor de Voz Específico ({voiceGender === 'male' ? '♂ Priorizando Masculinas' : '♀ Priorizando Femeninas'})
                        </label>
                        {topVoice && !voiceURI && (
                            <span className="text-[11px] font-mono text-aegis-cyan flex items-center gap-1">
                                <Sparkles className="w-3 h-3" /> Activa: {topVoice.label}
                            </span>
                        )}
                    </div>

                    <div className="flex flex-col sm:flex-row gap-3">
                        <select
                            value={voiceURI}
                            onChange={(e) => handleUpdatePrefs({ voiceURI: e.target.value, voicePreset: 'custom' })}
                            className="flex-1 bg-black/50 border border-white/15 rounded-xl py-3 px-4 text-sm text-white focus:border-aegis-cyan/60 focus:outline-none transition-all cursor-pointer"
                        >
                            <option value="">
                                ✨ Automática ({voiceGender === 'male' ? 'Mejor voz masculina disponible' : 'Mejor voz femenina disponible'})
                            </option>
                            {availableVoices.map((item) => {
                                const genderTag = item.gender === 'male' ? '♂ ' : item.gender === 'female' ? '♀ ' : '';
                                return (
                                    <option key={item.voice.voiceURI} value={item.voice.voiceURI}>
                                        {item.isNatural ? '✨ ' : ''}
                                        {genderTag}
                                        {item.label} [{item.voice.lang}]
                                    </option>
                                );
                            })}
                        </select>

                        <button
                            type="button"
                            onClick={handleTestVoice}
                            className={`px-5 py-3 rounded-xl border font-mono text-xs font-bold uppercase tracking-wider flex items-center justify-center gap-2 transition-all shrink-0 ${
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
                                    <Play className="w-3.5 h-3.5 fill-current" /> Probar Voz
                                </>
                            )}
                        </button>
                    </div>

                    {voiceGender === 'male' && !hasNativeMaleVoice && (
                        <p className="text-[11px] text-amber-300/80 bg-amber-500/10 border border-amber-500/20 rounded-lg px-3 py-2">
                            ℹ️ Tu navegador actual no reporta una voz masculina nativa instalada, por lo que aplicamos automáticamente un timbre grave (<em>pitch {pitch.toFixed(2)}</em>). Para voces masculinas neurales nativas (como <strong>Tomás - Argentina</strong> o <strong>Álvaro - España</strong>), abrí Aegis en <strong>Microsoft Edge</strong> o agregá el paquete de voz Español en Windows.
                        </p>
                    )}
                </div>

                {/* Sliders de Velocidad y Tono */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-6 pt-2">
                    <div className="bg-white/[0.03] border border-white/5 rounded-xl p-4 space-y-2">
                        <div className="flex justify-between text-xs font-mono">
                            <span className="text-white/60 uppercase">Velocidad de lectura</span>
                            <span className="text-aegis-cyan font-bold">{rate.toFixed(2)}x</span>
                        </div>
                        <input
                            type="range"
                            min="0.80"
                            max="1.25"
                            step="0.05"
                            value={rate}
                            onChange={(e) => handleUpdatePrefs({ rate: parseFloat(e.target.value), voicePreset: 'custom' })}
                            className="w-full accent-cyan-400 cursor-pointer"
                        />
                        <div className="flex justify-between text-[10px] font-mono text-white/25">
                            <span>Pausada (0.85x)</span>
                            <span>Natural (1.00x)</span>
                            <span>Rápida (1.25x)</span>
                        </div>
                    </div>

                    <div className="bg-white/[0.03] border border-white/5 rounded-xl p-4 space-y-2">
                        <div className="flex justify-between text-xs font-mono">
                            <span className="text-white/60 uppercase">Timbre / Tono</span>
                            <span className="text-aegis-cyan font-bold">{pitch.toFixed(2)}</span>
                        </div>
                        <input
                            type="range"
                            min="0.75"
                            max="1.20"
                            step="0.05"
                            value={pitch}
                            onChange={(e) => handleUpdatePrefs({ pitch: parseFloat(e.target.value), voicePreset: 'custom' })}
                            className="w-full accent-cyan-400 cursor-pointer"
                        />
                        <div className="flex justify-between text-[10px] font-mono text-white/25">
                            <span>Grave / Masculino (0.80)</span>
                            <span>Neutro (1.00)</span>
                            <span>Brillante (1.15)</span>
                        </div>
                    </div>
                </div>

                {/* Configuración de Micrófono y Manos Libres */}
                <div className="pt-4 border-t border-white/10 grid grid-cols-1 sm:grid-cols-2 gap-6 items-center">
                    <div className="space-y-2">
                        <label className="text-xs font-mono text-white/60 uppercase tracking-wider flex items-center gap-2">
                            <Mic className="w-3.5 h-3.5 text-aegis-cyan" /> Idioma del Micrófono
                        </label>
                        <select
                            value={micLang}
                            onChange={(e) => handleUpdatePrefs({ lang: e.target.value })}
                            className="w-full bg-black/50 border border-white/15 rounded-xl py-3 px-4 text-sm text-white focus:border-aegis-cyan/60 focus:outline-none transition-all cursor-pointer"
                        >
                            {MIC_LANGUAGES.map((l) => (
                                <option key={l.id} value={l.id}>
                                    {l.label}
                                </option>
                            ))}
                        </select>
                    </div>

                    <div className="bg-white/[0.03] border border-white/10 rounded-xl p-4 flex items-center justify-between gap-4">
                        <div>
                            <p className="text-xs font-mono font-bold uppercase text-white">
                                Conversación Continua (Manos Libres)
                            </p>
                            <p className="text-[11px] text-white/40 mt-0.5">
                                Reactiva el micrófono automáticamente cuando el asistente termina de hablar.
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={() => handleUpdatePrefs({ autoContinueMic: !autoContinueMic })}
                            className={`px-3.5 py-2 rounded-lg font-mono text-xs font-bold uppercase transition-all shrink-0 ${
                                autoContinueMic
                                    ? 'bg-green-500/20 border border-green-500/40 text-green-300'
                                    : 'bg-white/5 border border-white/10 text-white/40 hover:text-white'
                            }`}
                        >
                            {autoContinueMic ? 'Activo' : 'Inactivo'}
                        </button>
                    </div>
                </div>

                {savedToast && (
                    <motion.div
                        initial={{ opacity: 0, y: 4 }}
                        animate={{ opacity: 1, y: 0 }}
                        className="bg-green-500/10 border border-green-500/30 p-3 rounded-xl flex items-center gap-2.5 text-xs font-mono text-green-400"
                    >
                        <Check className="w-4 h-4" /> Preferencias de voz actualizadas.
                    </motion.div>
                )}
            </div>

            {/* Opciones Cloud Avanzadas (Opcional, plegado por defecto) */}
            <div className="glass rounded-2xl border border-white/10 overflow-hidden">
                <button
                    type="button"
                    onClick={() => setShowAdvancedCloud(!showAdvancedCloud)}
                    className="w-full p-5 flex items-center justify-between text-left hover:bg-white/[0.02] transition-colors"
                >
                    <div className="flex items-center gap-3">
                        <Sliders className="w-4 h-4 text-white/40" />
                        <div>
                            <span className="text-xs font-mono font-bold uppercase tracking-wider text-white/60">
                                Opciones Avanzadas Cloud (Opcional — ElevenLabs / Groq Whisper)
                            </span>
                            <p className="text-[11px] text-white/30">
                                Solo si querés usar APIs externas pagas en lugar de la voz integrada del navegador
                            </p>
                        </div>
                    </div>
                    {showAdvancedCloud ? (
                        <ChevronUp className="w-4 h-4 text-white/40" />
                    ) : (
                        <ChevronDown className="w-4 h-4 text-white/40" />
                    )}
                </button>

                {showAdvancedCloud && (
                    <form onSubmit={handleSaveCloudConfig} className="p-6 pt-2 border-t border-white/5 space-y-4">
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <div className="space-y-1.5">
                                <label className="text-[11px] font-mono text-white/50 uppercase">
                                    Proveedor de Transcripción (STT)
                                </label>
                                <select
                                    value={sttProvider}
                                    onChange={(e) => setSttProvider(e.target.value)}
                                    className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 text-xs font-mono text-white"
                                >
                                    <option value="browser">Navegador integrado (Recomendado — Gratis)</option>
                                    <option value="groq">Groq Whisper Cloud (Requiere API Key)</option>
                                </select>
                            </div>

                            {sttProvider === 'groq' && (
                                <div className="space-y-1.5">
                                    <label className="text-[11px] font-mono text-white/50 uppercase">
                                        Groq API Key
                                    </label>
                                    <div className="relative">
                                        <input
                                            type={showSttKey ? 'text' : 'password'}
                                            value={sttApiKey}
                                            onChange={(e) => setSttApiKey(e.target.value)}
                                            placeholder="gsk_..."
                                            className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 pr-9 text-xs font-mono text-white"
                                        />
                                        <button
                                            type="button"
                                            onClick={() => setShowSttKey(!showSttKey)}
                                            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-white/30 hover:text-white"
                                        >
                                            {showSttKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                                        </button>
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <div className="space-y-1.5">
                                <label className="text-[11px] font-mono text-white/50 uppercase">
                                    Proveedor TTS Externo
                                </label>
                                <select
                                    value={provider}
                                    onChange={(e) => setProvider(e.target.value)}
                                    className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 text-xs font-mono text-white"
                                >
                                    <option value="browser">Voz Natural del Navegador (Recomendado)</option>
                                    <option value="elevenlabs">ElevenLabs Cloud (Requiere API Key)</option>
                                </select>
                            </div>

                            {provider === 'elevenlabs' && (
                                <div className="space-y-1.5">
                                    <label className="text-[11px] font-mono text-white/50 uppercase">
                                        ElevenLabs API Key
                                    </label>
                                    <div className="relative">
                                        <input
                                            type={showKey ? 'text' : 'password'}
                                            value={apiKey}
                                            onChange={(e) => setApiKey(e.target.value)}
                                            placeholder="sk_..."
                                            className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 pr-9 text-xs font-mono text-white"
                                        />
                                        <button
                                            type="button"
                                            onClick={() => setShowKey(!showKey)}
                                            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-white/30 hover:text-white"
                                        >
                                            {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                                        </button>
                                    </div>
                                </div>
                            )}
                        </div>

                        <button
                            type="submit"
                            disabled={isSavingCloud}
                            className="px-5 py-2.5 rounded-xl bg-white/10 hover:bg-white/15 border border-white/15 text-xs font-mono uppercase tracking-wider text-white flex items-center gap-2"
                        >
                            {isSavingCloud ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <Save className="w-3.5 h-3.5" />}
                            Guardar Opciones Cloud
                        </button>
                    </form>
                )}
            </div>
        </div>
    );
};

export default SirenConfigTab;
