import React, { useState, useEffect, useCallback } from 'react';
import { motion } from 'framer-motion';
import { useShallow } from 'zustand/react/shallow';
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
    ShieldCheck,
    UserPlus,
    Trash2,
    Wand2,
    Activity,
    Download,
    Power,
    Key,
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
import { recordFixedSamplePcm16, playWakeChime } from '../audio/smartVadRecorder';

const MIC_LANGUAGES = [
    { id: 'es-AR', label: 'Español (Argentina / Rioplatense)' },
    { id: 'es-MX', label: 'Español (Latinoamérica / México)' },
    { id: 'es-ES', label: 'Español (España)' },
    { id: 'en-US', label: 'English (US)' },
];

const SirenConfigTab: React.FC = () => {
    const { tenantId, sessionKey, voiceEnabled, setVoiceEnabled, fetchSirenConfig } = useAegisStore(
        useShallow((s) => ({
            tenantId: s.tenantId,
            sessionKey: s.sessionKey,
            voiceEnabled: s.voiceEnabled,
            setVoiceEnabled: s.setVoiceEnabled,
            fetchSirenConfig: s.fetchSirenConfig,
        })),
    );

    const initialPrefs = loadVoicePrefs();
    const [voiceURI, setVoiceURI] = useState<string>(initialPrefs.voiceURI);
    const [voiceGender, setVoiceGender] = useState<VoiceGender>(initialPrefs.voiceGender || 'female');
    const [voicePreset, setVoicePreset] = useState<string>(initialPrefs.voicePreset || 'female-warm');
    const [rate, setRate] = useState<number>(initialPrefs.rate);
    const [pitch, setPitch] = useState<number>(initialPrefs.pitch);
    const [micLang, setMicLang] = useState<string>(initialPrefs.lang || 'es-AR');
    const [autoContinueMic, setAutoContinueMic] = useState<boolean>(initialPrefs.autoContinueMic);
    const [silenceHangoverMs, setSilenceHangoverMs] = useState<number>(initialPrefs.silenceHangoverMs ?? 2400);
    const [availableVoices, setAvailableVoices] = useState<ScoredVoice[]>([]);
    const [isTestingVoice, setIsTestingVoice] = useState(false);
    const [savedToast, setSavedToast] = useState(false);

    // Activación en tenant, frase libre y biometría multi-voz (EPIC 63)
    const [agentName, setAgentName] = useState('Aegis');
    const [wakePhraseInput, setWakePhraseInput] = useState('Aegis escuchame');
    const [savingWakePhrase, setSavingWakePhrase] = useState(false);
    const [alwaysOnEnabled, setAlwaysOnEnabled] = useState(true);
    const [togglingAlwaysOn, setTogglingAlwaysOn] = useState(false);
    const [daemonConnected, setDaemonConnected] = useState(false);
    const [downloadingInstaller, setDownloadingInstaller] = useState(false);
    const [sttReady, setSttReady] = useState(false);
    const [inlineGroqKey, setInlineGroqKey] = useState('');
    const [savingGroqKey, setSavingGroqKey] = useState(false);
    const [enrolledProfiles, setEnrolledProfiles] = useState<Array<{ id: string; label: string; sample_count: number; dims?: number }>>([]);
    const [gateEnabled, setGateEnabled] = useState(true);
    const [enrollLabel, setEnrollLabel] = useState('Mi voz');
    const [recordedSamples, setRecordedSamples] = useState<(string | null)[]>([null, null, null]);
    const [activeSampleIdx, setActiveSampleIdx] = useState<number | null>(null);
    const [enrollLevel, setEnrollLevel] = useState<number>(0);
    const [isEnrolling, setIsEnrolling] = useState(false);
    const [isSavingVoice, setIsSavingVoice] = useState(false);
    const [isTestingSpeaker, setIsTestingSpeaker] = useState(false);
    const [biometricStatusMsg, setBiometricStatusMsg] = useState<string | null>(null);

    const isInsecureBrowserHttp =
        typeof window !== 'undefined' &&
        window.location.protocol === 'http:' &&
        !['localhost', '127.0.0.1'].includes(window.location.hostname);

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

    const applyVoiceSettingsPayload = useCallback((data: {
        profiles?: Array<{ id: string; label: string; sample_count: number; dims?: number }>;
        gate_enabled?: boolean;
        agent_name?: string;
        wake_phrase?: string;
        always_on_enabled?: boolean;
        groq_key_detected?: boolean;
        stt_ready?: boolean;
        daemon_connected?: boolean;
    }) => {
        setEnrolledProfiles(data.profiles || []);
        setGateEnabled(data.gate_enabled !== false);
        if (data.agent_name) setAgentName(data.agent_name);
        if (typeof data.wake_phrase === 'string' && data.wake_phrase.trim()) {
            setWakePhraseInput(data.wake_phrase);
        }
        if (typeof data.always_on_enabled === 'boolean') {
            setAlwaysOnEnabled(data.always_on_enabled);
        }
        setSttReady(!!data.stt_ready || !!data.groq_key_detected);
        setDaemonConnected(!!data.daemon_connected);
    }, []);

    const loadBiometricsState = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch('/api/siren/voice-settings', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                const data = await res.json();
                applyVoiceSettingsPayload(data);
            }
        } catch {
            // best-effort
        }
    }, [tenantId, sessionKey, applyVoiceSettingsPayload]);

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
                    setSttReady(!!data.stt_ready);
                }
            })
            .catch(() => {});
        loadBiometricsState();

        const interval = setInterval(() => {
            loadBiometricsState();
        }, 8000);
        return () => clearInterval(interval);
    }, [tenantId, sessionKey, loadBiometricsState]);

    const handleUpdatePrefs = (
        patch: Partial<{
            voiceURI: string;
            voiceGender: VoiceGender;
            voicePreset: string;
            rate: number;
            pitch: number;
            lang: string;
            autoContinueMic: boolean;
            silenceHangoverMs: number;
            followupWindowSecs: number;
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
        setSilenceHangoverMs(next.silenceHangoverMs ?? 2400);
        refreshBrowserVoices(next.voiceGender);
        setSavedToast(true);
        setTimeout(() => setSavedToast(false), 2000);
    };

    const postVoiceSettings = async (payload: Record<string, unknown>) => {
        const res = await fetch('/api/siren/voice-settings', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-citadel-tenant': tenantId || '',
                'x-citadel-key': sessionKey || '',
            },
            body: JSON.stringify(payload),
        });
        const data = await res.json();
        if (!res.ok) {
            throw new Error(data.error || data.message || 'No se pudo guardar la configuración');
        }
        applyVoiceSettingsPayload(data);
        return data;
    };

    const handleToggleAlwaysOn = async () => {
        if (!tenantId || !sessionKey || togglingAlwaysOn) return;
        const next = !alwaysOnEnabled;
        setAlwaysOnEnabled(next);
        setTogglingAlwaysOn(true);
        try {
            await postVoiceSettings({ always_on_enabled: next });
            setBiometricStatusMsg(
                next
                    ? `✓ Escucha por frase activada ("${wakePhraseInput || `${agentName} escuchame`}").`
                    : '✓ Escucha continua pausada (Alt+Shift+A sigue funcionando cuando quieras).'
            );
        } catch (err) {
            setAlwaysOnEnabled(!next);
            setBiometricStatusMsg(`✕ ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setTogglingAlwaysOn(false);
        }
    };

    const handleSaveWakePhrase = async (phraseOverride?: string) => {
        if (!tenantId || !sessionKey) return;
        const phrase = (phraseOverride ?? wakePhraseInput).trim();
        if (!phrase) return;
        setWakePhraseInput(phrase);
        setSavingWakePhrase(true);
        try {
            await postVoiceSettings({ wake_phrase: phrase });
            setBiometricStatusMsg(`✓ Frase de activación guardada: "${phrase}".`);
        } catch (err) {
            setBiometricStatusMsg(`✕ ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setSavingWakePhrase(false);
        }
    };

    const handleSaveInlineGroqKey = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!tenantId || !sessionKey || !inlineGroqKey.trim()) return;
        setSavingGroqKey(true);
        try {
            await postVoiceSettings({ groq_api_key: inlineGroqKey.trim() });
            setInlineGroqKey('');
            await fetchSirenConfig();
            setBiometricStatusMsg('✓ Groq Whisper Turbo activado para este tenant.');
        } catch (err) {
            setBiometricStatusMsg(`✕ ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setSavingGroqKey(false);
        }
    };

    const handleDownloadPcActivator = async () => {
        if (!tenantId || !sessionKey || downloadingInstaller) return;
        setDownloadingInstaller(true);
        try {
            const res = await fetch('/api/siren/desktop-daemon/installer', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ base_url: window.location.origin }),
            });
            const data = await res.json();
            if (!res.ok || !data.script) {
                throw new Error(data.error || 'No se pudo generar el activador para PC');
            }
            const blob = new Blob([data.script], { type: 'application/x-bat;charset=utf-8' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = data.filename || `activar-voz-aegis-${tenantId}.bat`;
            document.body.appendChild(a);
            a.click();
            a.remove();
            URL.revokeObjectURL(url);
            setBiometricStatusMsg(
                `✓ Descargado "${a.download}". Ejecutalo con doble clic una vez y tu PC queda escuchando en segundo plano.`
            );
        } catch (err) {
            setBiometricStatusMsg(`✕ ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setDownloadingInstaller(false);
        }
    };

    const handleToggleGate = async () => {
        if (!tenantId || !sessionKey) return;
        const next = !gateEnabled;
        setGateEnabled(next);
        try {
            await postVoiceSettings({ gate_enabled: next });
        } catch {
            setGateEnabled(!next);
        }
    };

    const submitSamplesToServer = async (samples: string[]) => {
        if (!tenantId || !sessionKey || !enrollLabel.trim() || samples.length === 0) return;
        setIsSavingVoice(true);
        try {
            const res = await fetch('/api/siren/voices/enroll', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({
                    label: enrollLabel.trim(),
                    samples_pcm_b64: samples,
                    sample_rate: 16000,
                }),
            });
            const data = await res.json();
            if (res.ok && data.success) {
                playWakeChime('done');
                setRecordedSamples([null, null, null]);
                setBiometricStatusMsg(
                    `✓ Voz "${enrollLabel.trim()}" guardada con ${samples.length} muestra(s).`
                );
                await loadBiometricsState();
            } else {
                setBiometricStatusMsg(`✕ Error al registrar voz: ${data.error || data.message || 'Audio muy bajo'}`);
            }
        } finally {
            setIsSavingVoice(false);
        }
    };

    const handleRecordSingleSample = async (idx: number) => {
        if (!tenantId || !sessionKey || isEnrolling || isTestingSpeaker || isSavingVoice) return;
        setIsEnrolling(true);
        setActiveSampleIdx(idx);
        setBiometricStatusMsg(null);
        try {
            playWakeChime('ready');
            const rec = await recordFixedSamplePcm16(3000, (rms) => setEnrollLevel(Math.min(1, rms * 10)));
            setRecordedSamples((prev) => {
                const copy = [...prev];
                copy[idx] = rec.pcmB64;
                return copy;
            });
            playWakeChime('done');
            setBiometricStatusMsg(`✓ Muestra ${idx + 1}/3 grabada. Podés grabar las que falten o guardar.`);
        } catch (err) {
            setBiometricStatusMsg(`✕ No se pudo acceder al micrófono: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setIsEnrolling(false);
            setActiveSampleIdx(null);
            setEnrollLevel(0);
        }
    };

    const handleRecordAllThreeGuided = async () => {
        if (!tenantId || !sessionKey || !enrollLabel.trim() || isEnrolling || isSavingVoice) return;
        setIsEnrolling(true);
        setBiometricStatusMsg(null);
        try {
            const samples: string[] = [];
            for (let idx = 0; idx < 3; idx++) {
                setActiveSampleIdx(idx);
                playWakeChime('ready');
                const rec = await recordFixedSamplePcm16(3000, (rms) => setEnrollLevel(Math.min(1, rms * 10)));
                samples.push(rec.pcmB64);
                setRecordedSamples((prev) => {
                    const copy = [...prev];
                    copy[idx] = rec.pcmB64;
                    return copy;
                });
                if (idx < 2) {
                    await new Promise((r) => setTimeout(r, 1200));
                }
            }
            setActiveSampleIdx(null);
            setEnrollLevel(0);
            await submitSamplesToServer(samples);
        } catch (err) {
            setBiometricStatusMsg(`✕ No se pudo acceder al micrófono: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setIsEnrolling(false);
            setActiveSampleIdx(null);
            setEnrollLevel(0);
        }
    };

    const handleSubmitRecordedSamples = async () => {
        const valid = recordedSamples.filter((s): s is string => typeof s === 'string' && s.length > 0);
        if (valid.length === 0) return;
        try {
            await submitSamplesToServer(valid);
        } catch (err) {
            setBiometricStatusMsg(`✕ Error guardando muestras: ${err instanceof Error ? err.message : String(err)}`);
        }
    };

    const handleTestSpeakerRecognition = async () => {
        if (!tenantId || !sessionKey || isTestingSpeaker || isEnrolling) return;
        setIsTestingSpeaker(true);
        setBiometricStatusMsg('Hablá durante 3 segundos para probar tu reconocimiento...');
        try {
            playWakeChime('ready');
            const rec = await recordFixedSamplePcm16(3000, (rms) => setEnrollLevel(Math.min(1, rms * 10)));
            const res = await fetch('/api/siren/voices/verify-test', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ pcm_b64: rec.pcmB64, sample_rate: 16000 }),
            });
            const data = await res.json();
            const pct = Math.round((data.score ?? 0) * 100);
            if (data.accepted) {
                playWakeChime('done');
                setBiometricStatusMsg(
                    data.matched_label
                        ? `✓ Voz reconocida: "${data.matched_label}" (${pct}% similitud)`
                        : '✓ Sin filtro activo aún (grabá muestras para restringir accesos).'
                );
            } else {
                playWakeChime('rejected');
                setBiometricStatusMsg(`✕ Voz rechazada (${pct}% < ${Math.round((data.threshold ?? 0.78) * 100)}% requerido).`);
            }
        } catch (err) {
            setBiometricStatusMsg(`✕ Error probando micrófono: ${err instanceof Error ? err.message : String(err)}`);
        } finally {
            setIsTestingSpeaker(false);
            setEnrollLevel(0);
        }
    };

    const handleDeleteEnrolledVoice = async (id: string) => {
        if (!tenantId || !sessionKey) return;
        await fetch(`/api/siren/voices/enrolled/${encodeURIComponent(id)}`, {
            method: 'DELETE',
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        });
        await loadBiometricsState();
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
    const recordedCount = recordedSamples.filter(Boolean).length;
    const samplePrompts = [
        `"${wakePhraseInput || `${agentName} escuchame`}, revisá mis recordatorios"`,
        `"Anotá diez mil pesos de supermercado en gastos"`,
        `"Contame qué tengo agendado para mañana a la mañana"`,
    ];

    return (
        <div className="space-y-6 max-h-[70vh] overflow-y-auto pr-2 custom-scrollbar">
            {/* Tarjeta Principal 1: Activación en Tenant, Frase Personalizada y Muestras de Voz */}
            <div className="glass p-6 rounded-2xl border border-amber-500/25 shadow-2xl space-y-5">
                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-4">
                    <div className="flex items-center gap-3">
                        <div className="w-10 h-10 rounded-xl bg-amber-500/15 border border-amber-500/30 flex items-center justify-center">
                            <ShieldCheck className="w-5 h-5 text-amber-400" />
                        </div>
                        <div>
                            <h3 className="text-sm font-mono font-bold uppercase tracking-wider text-white">
                                Activación por Voz y Reconocimiento ({tenantId || 'tenant'})
                            </h3>
                            <p className="text-xs text-white/45 flex flex-wrap items-center gap-2 mt-0.5">
                                <span className={daemonConnected ? 'text-green-400' : 'text-white/45'}>
                                    {daemonConnected ? '● Activo en 2do plano en PC' : '○ PC en 2do plano aún no conectada'}
                                </span>
                                <span>·</span>
                                <span>{sttReady ? '✓ Transcripción lista' : '⚠️ Falta clave gratuita de Groq'}</span>
                            </p>
                        </div>
                    </div>

                    <div className="flex flex-wrap items-center gap-2">
                        <button
                            type="button"
                            onClick={handleToggleAlwaysOn}
                            disabled={togglingAlwaysOn}
                            className={`px-3.5 py-2 rounded-xl font-mono text-xs font-bold uppercase transition-all border flex items-center gap-1.5 ${
                                alwaysOnEnabled
                                    ? 'bg-green-500/15 border-green-500/40 text-green-300'
                                    : 'bg-white/5 border-white/10 text-white/50 hover:text-white'
                            }`}
                        >
                            <Power className="w-3.5 h-3.5" />
                            {alwaysOnEnabled ? 'Escucha Activada' : 'Escucha Pausada'}
                        </button>

                        <button
                            type="button"
                            onClick={handleDownloadPcActivator}
                            disabled={downloadingInstaller}
                            className="px-3.5 py-2 rounded-xl font-mono text-xs font-bold uppercase transition-all border bg-aegis-cyan/15 hover:bg-aegis-cyan/25 border-aegis-cyan/40 text-aegis-cyan flex items-center gap-1.5 disabled:opacity-50"
                        >
                            <Download className="w-3.5 h-3.5" />
                            {downloadingInstaller ? 'Preparando...' : 'Activar en esta PC (1 clic)'}
                        </button>
                    </div>
                </div>

                {/* Paso rápido STT si todavía no hay key de Groq */}
                {!sttReady && (
                    <form
                        onSubmit={handleSaveInlineGroqKey}
                        className="bg-amber-500/10 border border-amber-500/25 rounded-xl p-4 space-y-2.5"
                    >
                        <div className="flex items-center gap-2 text-xs font-mono font-bold text-amber-200">
                            <Key className="w-4 h-4 text-amber-400 shrink-0" />
                            Falta activar la transcripción de voz (Groq Whisper gratis)
                        </div>
                        <p className="text-[11px] text-white/60">
                            Pegá una API key gratuita de Groq (`gsk_...`) para que Aegis entienda tu voz desde esta PC y desde la app:
                        </p>
                        <div className="flex flex-col sm:flex-row gap-2">
                            <input
                                type="password"
                                value={inlineGroqKey}
                                onChange={(e) => setInlineGroqKey(e.target.value)}
                                placeholder="gsk_..."
                                className="flex-1 bg-black/50 border border-white/15 rounded-xl py-2 px-3 text-xs font-mono text-white focus:border-amber-400/60 focus:outline-none"
                            />
                            <button
                                type="submit"
                                disabled={savingGroqKey || !inlineGroqKey.trim()}
                                className="px-4 py-2 rounded-xl bg-amber-500/25 hover:bg-amber-500/35 border border-amber-500/40 text-xs font-mono font-bold text-amber-100 uppercase disabled:opacity-40"
                            >
                                {savingGroqKey ? 'Guardando...' : 'Activar Groq'}
                            </button>
                        </div>
                    </form>
                )}

                {/* Frase de activación personalizable */}
                <div className="space-y-2">
                    <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-end">
                        <div className="flex-1 space-y-1.5">
                            <label className="text-[11px] font-mono text-white/60 uppercase flex items-center gap-1.5">
                                <Wand2 className="w-3.5 h-3.5 text-amber-400" /> Frase para activar al asistente
                            </label>
                            <input
                                type="text"
                                maxLength={60}
                                value={wakePhraseInput}
                                onChange={(e) => setWakePhraseInput(e.target.value)}
                                placeholder={`${agentName} escuchame`}
                                className="w-full bg-black/50 border border-white/15 rounded-xl py-2.5 px-3 text-sm text-white focus:border-amber-400/60 focus:outline-none"
                            />
                        </div>
                        <button
                            type="button"
                            onClick={() => handleSaveWakePhrase()}
                            disabled={savingWakePhrase || !wakePhraseInput.trim()}
                            className="px-4 py-2.5 rounded-xl bg-amber-500/20 hover:bg-amber-500/30 border border-amber-500/40 text-xs font-mono font-bold text-amber-200 uppercase disabled:opacity-40"
                        >
                            {savingWakePhrase ? 'Guardando...' : 'Guardar Frase'}
                        </button>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                        {[
                            `${agentName} escuchame`,
                            `Oye ${agentName}`,
                            `Hola ${agentName}`,
                            'Che asistente',
                        ].map((preset) => (
                            <button
                                key={preset}
                                type="button"
                                onClick={() => handleSaveWakePhrase(preset)}
                                className={`px-2.5 py-1 rounded-lg border text-[11px] font-mono transition-colors ${
                                    wakePhraseInput.trim().toLowerCase() === preset.toLowerCase()
                                        ? 'bg-amber-500/20 border-amber-500/40 text-amber-200'
                                        : 'bg-white/[0.03] border-white/10 text-white/50 hover:text-white'
                                }`}
                            >
                                &ldquo;{preset}&rdquo;
                            </button>
                        ))}
                    </div>
                </div>

                {/* Muestras de reconocimiento vocal del tenant */}
                <div className="bg-black/30 border border-white/10 rounded-xl p-4 space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                        <span className="text-xs font-mono font-bold uppercase text-white/80">
                            Muestras de Reconocimiento de Voz
                        </span>
                        <button
                            type="button"
                            onClick={handleToggleGate}
                            className={`px-3 py-1.5 rounded-lg font-mono text-[11px] font-bold uppercase transition-all border ${
                                gateEnabled
                                    ? 'bg-green-500/15 border-green-500/40 text-green-300'
                                    : 'bg-white/5 border-white/10 text-white/40'
                            }`}
                        >
                            {gateEnabled ? '🔒 Solo Voces Registradas' : '🔓 Responder a Cualquier Voz'}
                        </button>
                    </div>

                    {isInsecureBrowserHttp && (
                        <p className="text-[11px] text-amber-300/90 bg-amber-500/10 border border-amber-500/20 rounded-lg px-3 py-2">
                            ℹ️ Estás entrando por IP en HTTP. Para grabar muestras desde este navegador abrí{' '}
                            <strong className="font-mono">http://localhost:8000</strong> en esta PC (o grabalas desde la app del celular).
                        </p>
                    )}

                    <div className="flex flex-col sm:flex-row gap-2.5 items-stretch sm:items-center">
                        <input
                            type="text"
                            value={enrollLabel}
                            onChange={(e) => setEnrollLabel(e.target.value)}
                            placeholder="Nombre de la voz (ej: Mi voz, Voz Celular)"
                            className="flex-1 bg-black/50 border border-white/15 rounded-xl py-2 px-3 text-xs text-white focus:border-aegis-cyan/60 focus:outline-none"
                        />
                        <button
                            type="button"
                            onClick={handleRecordAllThreeGuided}
                            disabled={isEnrolling || isTestingSpeaker || isSavingVoice || !enrollLabel.trim()}
                            className="px-4 py-2 rounded-xl bg-aegis-cyan/20 hover:bg-aegis-cyan/30 border border-aegis-cyan/40 text-xs font-mono font-bold text-aegis-cyan flex items-center justify-center gap-2 disabled:opacity-40"
                        >
                            <UserPlus className="w-3.5 h-3.5" />
                            {isEnrolling && activeSampleIdx !== null
                                ? `Grabando ${activeSampleIdx + 1}/3...`
                                : isSavingVoice
                                ? 'Guardando...'
                                : 'Grabar 3 guiadas'}
                        </button>
                        <button
                            type="button"
                            onClick={handleTestSpeakerRecognition}
                            disabled={isEnrolling || isTestingSpeaker || isSavingVoice}
                            className="px-3.5 py-2 rounded-xl bg-white/10 hover:bg-white/15 border border-white/15 text-xs font-mono text-white disabled:opacity-40"
                        >
                            {isTestingSpeaker ? 'Escuchando 3s...' : 'Probar Mi Voz'}
                        </button>
                    </div>

                    {/* Botones individuales por muestra */}
                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                        {[0, 1, 2].map((idx) => {
                            const hasSample = !!recordedSamples[idx];
                            const isRecThis = isEnrolling && activeSampleIdx === idx;
                            return (
                                <button
                                    key={idx}
                                    type="button"
                                    onClick={() => handleRecordSingleSample(idx)}
                                    disabled={isEnrolling || isTestingSpeaker || isSavingVoice}
                                    className={`p-2.5 rounded-xl border text-left transition-all ${
                                        isRecThis
                                            ? 'bg-red-500/20 border-red-400 text-red-200 animate-pulse'
                                            : hasSample
                                            ? 'bg-green-500/15 border-green-500/40 text-green-200'
                                            : 'bg-white/[0.02] border-white/10 text-white/60 hover:text-white'
                                    }`}
                                >
                                    <div className="flex items-center justify-between text-xs font-mono font-bold">
                                        <span>Muestra {idx + 1}/3</span>
                                        {hasSample ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Mic className="w-3.5 h-3.5" />}
                                    </div>
                                    <p className="text-[10px] text-white/45 mt-1 truncate">
                                        {isRecThis ? 'Hablá ahora (3s)...' : hasSample ? 'Grabada (clic para regrabar)' : samplePrompts[idx]}
                                    </p>
                                </button>
                            );
                        })}
                    </div>

                    {recordedCount > 0 && !isEnrolling && (
                        <div className="flex items-center justify-between gap-2 bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-xl px-3 py-2">
                            <span className="text-xs font-mono text-aegis-cyan">
                                {recordedCount}/3 muestras listas para &ldquo;{enrollLabel.trim() || 'Mi voz'}&rdquo;
                            </span>
                            <button
                                type="button"
                                onClick={handleSubmitRecordedSamples}
                                disabled={isSavingVoice || !enrollLabel.trim()}
                                className="px-3.5 py-1.5 rounded-lg bg-aegis-cyan text-black font-mono text-xs font-bold uppercase disabled:opacity-40"
                            >
                                {isSavingVoice ? 'Guardando...' : 'Guardar Voz Autorizada'}
                            </button>
                        </div>
                    )}

                    {(isEnrolling || isTestingSpeaker) && (
                        <div className="space-y-1.5">
                            <p className="text-[11px] font-mono text-amber-300">
                                {activeSampleIdx !== null
                                    ? `Muestra ${activeSampleIdx + 1}/3 — Decí: ${samplePrompts[activeSampleIdx]}`
                                    : 'Hablá naturalmente durante 3 segundos...'}
                            </p>
                            <div className="w-full h-1.5 bg-white/10 rounded-full overflow-hidden">
                                <div
                                    className="h-full bg-aegis-cyan transition-all duration-75"
                                    style={{ width: `${Math.round(enrollLevel * 100)}%` }}
                                />
                            </div>
                        </div>
                    )}

                    {biometricStatusMsg && (
                        <p className="text-xs font-mono text-white/85 bg-white/5 border border-white/10 rounded-lg px-3 py-2">
                            {biometricStatusMsg}
                        </p>
                    )}

                    {enrolledProfiles.length > 0 ? (
                        <div className="flex flex-wrap gap-2 pt-1">
                            {enrolledProfiles.map((p) => (
                                <div
                                    key={p.id}
                                    className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-white/5 border border-white/10 text-xs font-mono text-white/80"
                                >
                                    <span>{p.label}</span>
                                    <span className="text-[10px] text-white/40">({p.sample_count} muestras)</span>
                                    <button
                                        type="button"
                                        onClick={() => handleDeleteEnrolledVoice(p.id)}
                                        className="text-white/30 hover:text-red-400 transition-colors"
                                        title="Eliminar voz"
                                    >
                                        <Trash2 className="w-3.5 h-3.5" />
                                    </button>
                                </div>
                            ))}
                        </div>
                    ) : (
                        <p className="text-[11px] text-white/40">
                            Aún no cargaste muestras de tu voz. Grabá 3 muestras acá arriba para que solo responda a tu voz.
                        </p>
                    )}
                </div>
            </div>

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

                {/* Tolerancia a pausas para respirar */}
                <div className="pt-4 border-t border-white/10 space-y-2">
                    <label className="text-xs font-mono text-white/60 uppercase tracking-wider flex items-center gap-2">
                        <Activity className="w-3.5 h-3.5 text-aegis-cyan" /> Tolerancia a Pausas / Respiración al Hablar
                    </label>
                    <div className="grid grid-cols-3 gap-2">
                        {[
                            { ms: 1600, label: 'Ágil (1.6s)' },
                            { ms: 2400, label: 'Natural (2.4s)' },
                            { ms: 3400, label: 'Pensativo (3.4s)' },
                        ].map((opt) => (
                            <button
                                key={opt.ms}
                                type="button"
                                onClick={() => handleUpdatePrefs({ silenceHangoverMs: opt.ms })}
                                className={`py-2 px-3 rounded-xl border text-xs font-mono transition-all ${
                                    silenceHangoverMs === opt.ms
                                        ? 'bg-aegis-cyan/15 border-aegis-cyan/50 text-white font-bold'
                                        : 'bg-white/[0.02] border-white/10 text-white/50 hover:text-white'
                                }`}
                            >
                                {opt.label}
                            </button>
                        ))}
                    </div>
                    <p className="text-[11px] text-white/35">
                        Evita cortes cuando respirás o pensás a mitad de una frase.
                    </p>
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
