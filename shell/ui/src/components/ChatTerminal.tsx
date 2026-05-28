import React, { useRef, useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { motion, AnimatePresence } from 'framer-motion';
import { Send, Settings, AlertCircle, Mic, MicOff, Paperclip, Loader2, LogOut, LayoutDashboard, Volume2, VolumeX, ChevronDown, Check, X, AlertTriangle } from 'lucide-react';
import { useAegisStore, Message } from '../store/useAegisStore';
import { AgentBadge } from './AgentBadge';
import { InputModeSelector } from './InputModeSelector';
import { useTranslation } from '../i18n';
import { AegisLogo } from './AegisLogo';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import TelemetryDashboard from './TelemetryDashboard';
import SettingsPanel from './SettingsPanel';
import MusicPlayer from './MusicPlayer';
import { QrCode } from 'lucide-react';
import { ConnectionQR } from './ConnectionQR';
import { AgentActivityPanel } from './AgentActivityPanel';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface CatalogModel {
    model_id: string;
    provider: string;
    display_name?: string;
    cost_input_per_mtok: number;
}

const StatusBadge: React.FC<{ status: string }> = ({ status }) => {
    const { lastError } = useAegisStore();
    const config: Record<string, { color: string, label: string }> = {
        idle: { color: 'bg-green-500', label: 'Idle' },
        thinking: { color: 'bg-aegis-purple animate-pulse shadow-[0_0_8px_rgba(191,0,255,0.6)]', label: 'Processing' },
        executing_syscall: { color: 'bg-aegis-cyan animate-pulse', label: 'Syscall' },
        disconnected: { color: 'bg-red-500', label: 'Offline' },
        connecting: { color: 'bg-yellow-500 animate-bounce', label: 'Linking' },
        error: { color: 'bg-red-600 shadow-[0_0_10px_rgba(255,0,0,0.5)]', label: lastError || 'Kernel Panic' },
        listening: { color: 'bg-blue-500 animate-pulse shadow-[0_0_10px_rgba(59,130,246,0.8)]', label: 'Listening' },
        transcribing: { color: 'bg-pink-500 animate-pulse shadow-[0_0_10px_rgba(236,72,153,0.8)]', label: 'Transcribing' },
    };
    const current = config[status] || config.disconnected;
    return (
        <div className="flex items-center gap-2 group cursor-help" title={lastError || undefined}>
            <div className={cn("w-2 h-2 rounded-full", current.color)} />
            <span className={cn("text-[9px] font-bold font-mono uppercase tracking-tighter transition-all group-hover:text-white", status === 'error' ? "text-red-400" : "text-white/50")}>
                {current.label}
            </span>
        </div>
    );
};

const MessageItem: React.FC<{ message: Message }> = ({ message }) => {
    const isAssistant = message.role === 'assistant';
    const isUser = message.role === 'user';
    const isSystem = message.role === 'system';

    return (
        <motion.div
            initial={{ opacity: 0, x: isUser ? 10 : -10, y: 5 }}
            animate={{ opacity: 1, x: 0, y: 0 }}
            className={cn("flex w-full gap-4 px-2", isUser ? "justify-end" : "justify-start")}
        >
            <div className={cn("max-w-[85%] flex flex-col gap-1.5", isUser ? "items-end" : "items-start")}>
                <div className="flex items-center gap-2 px-1">
                    {isUser && <span className="text-[10px] font-mono text-white/40 uppercase">Operator</span>}
                    {isAssistant && <span className="text-[10px] font-mono text-aegis-cyan/60 uppercase">ANK Kernel</span>}
                    {isSystem && <span className="text-[10px] font-mono text-aegis-purple/60 uppercase">System log</span>}
                </div>
                <div className={cn(
                    "rounded-2xl px-4 py-3 text-sm transition-all shadow-lg",
                    isUser && "bg-aegis-cyan/10 border border-aegis-cyan/20 text-white rounded-tr-none",
                    isAssistant && message.type === 'text' && "bg-white/5 border border-white/10 text-white/90 rounded-tl-none",
                    isAssistant && message.type === 'thought' && "bg-aegis-purple/5 border border-aegis-purple/10 text-aegis-purple/60 text-xs italic font-mono rounded-tl-none",
                    isSystem && "bg-black/50 border border-aegis-cyan/40 text-aegis-cyan flex items-center gap-3",
                    message.type === 'error' && "bg-red-500/10 border border-red-500/30 text-red-500 italic"
                )}>
                    {isSystem && <Settings className="w-4 h-4" />}
                    {message.type === 'error' && <AlertCircle className="w-4 h-4 inline-block mr-2" />}
                    <div className="prose prose-invert prose-sm max-w-none prose-p:leading-relaxed prose-code:text-aegis-cyan prose-code:bg-aegis-cyan/5 prose-code:px-1 prose-code:rounded prose-pre:bg-white/5 prose-pre:border prose-pre:border-white/10">
                        {message.type === 'thought' ? (
                            <p>{message.content}</p>
                        ) : (
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.content}</ReactMarkdown>
                        )}
                    </div>
                </div>
                <span className="text-[9px] font-mono text-white/10 px-1 mt-0.5">
                    {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                </span>
            </div>
        </motion.div>
    );
};

const ChatTerminal: React.FC = () => {
    const { t } = useTranslation();
    const { messages, sendMessage, status, isRecording, sttAvailable, startSirenStream, stopSirenStream, tenantId, sessionKey, addSystemMessage, logout, fetchSirenConfig, inputMode, lastRoutingInfo, voiceEnabled, setVoiceEnabled } = useAegisStore();
    const scrollRef = useRef<HTMLDivElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const modelPickerRef = useRef<HTMLDivElement>(null);
    const isAtBottom = useRef(true);
    const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const processingBannerTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const [input, setInput] = useState('');
    const [showProcessingBanner, setShowProcessingBanner] = useState(false);
    const [voiceError, setVoiceError] = useState<string | null>(null);
    const [isUploading, setIsUploading] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [showQR, setShowQR] = useState(false);
    const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
    const [availableModels, setAvailableModels] = useState<CatalogModel[]>([]);
    const [showModelPicker, setShowModelPicker] = useState(false);
    const [showLocalHistoryWarning, setShowLocalHistoryWarning] = useState(() => {
        if (typeof window === 'undefined') return false;
        const dismissed = localStorage.getItem('aegis_dismiss_http_warn') === 'true';
        const isInsecure = window.location.protocol === 'http:' &&
            !['localhost', '127.0.0.1'].includes(window.location.hostname);
        return isInsecure && !dismissed;
    });

    const scrollToBottom = (behavior: ScrollBehavior = 'smooth') => {
        if (scrollRef.current) {
            scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior });
        }
    };

    const handleScroll = () => {
        if (scrollRef.current) {
            const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
            isAtBottom.current = scrollHeight - scrollTop - clientHeight < 100;
        }
    };

    useEffect(() => { if (isAtBottom.current) scrollToBottom('smooth'); }, [messages]);
    useEffect(() => { if (tenantId && sessionKey) fetchSirenConfig(); }, [tenantId, sessionKey, fetchSirenConfig]);
    useEffect(() => {
        if (status !== 'thinking') {
            if (thinkingTimeoutRef.current) {
                clearTimeout(thinkingTimeoutRef.current);
                thinkingTimeoutRef.current = null;
            }
            if (processingBannerTimeoutRef.current) {
                clearTimeout(processingBannerTimeoutRef.current);
                processingBannerTimeoutRef.current = null;
            }
            setShowProcessingBanner(false);
        }
    }, [status]);

    useEffect(() => {
        if (!tenantId || !sessionKey) return;
        fetch('/api/router/models', {
            headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
        })
            .then(r => r.ok ? r.json() : null)
            .then(data => { if (data?.models) setAvailableModels(data.models); })
            .catch(() => {});
    }, [tenantId, sessionKey]);

    useEffect(() => {
        if (!showModelPicker) return;
        const handleClickOutside = (e: MouseEvent) => {
            if (modelPickerRef.current && !modelPickerRef.current.contains(e.target as Node)) {
                setShowModelPicker(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [showModelPicker]);

    const handleSend = () => {
        if (!input.trim()) return;
        sendMessage(input, selectedModelId);
        setInput('');
        setTimeout(() => { isAtBottom.current = true; scrollToBottom('auto'); }, 10);

        if (thinkingTimeoutRef.current) clearTimeout(thinkingTimeoutRef.current);
        if (processingBannerTimeoutRef.current) clearTimeout(processingBannerTimeoutRef.current);
        setShowProcessingBanner(false);

        processingBannerTimeoutRef.current = setTimeout(() => {
            if (useAegisStore.getState().status === 'thinking') {
                setShowProcessingBanner(true);
            }
            processingBannerTimeoutRef.current = null;
        }, 5_000);

        thinkingTimeoutRef.current = setTimeout(() => {
            if (useAegisStore.getState().status === 'thinking') {
                useAegisStore.getState().addSystemMessage('El motor tardó demasiado en responder. Podés intentar de nuevo.');
                useAegisStore.getState().setStatus('idle');
            }
            thinkingTimeoutRef.current = null;
        }, 120_000);
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); }
    };

    const handleToggleMic = async () => {
        if (isRecording) {
            stopSirenStream();
        } else {
            try {
                setVoiceError(null);
                await startSirenStream();
            } catch (err: unknown) {
                const error = err as Error;
                let msg: string;
                if (error.message?.startsWith('INSECURE_CONTEXT')) {
                    msg = 'El micrófono requiere HTTPS. Accedé via el link de Cloudflare.';
                } else if (error.name === 'NotAllowedError') {
                    msg = 'El navegador bloqueó el micrófono. Verificá los permisos de esta página.';
                } else {
                    msg = error.message || 'Error de hardware';
                }
                setVoiceError(msg);
                setTimeout(() => setVoiceError(null), 8000);
            }
        }
    };

    const handleVoiceToggle = () => {
        const isInsecure = window.location.protocol === 'http:' &&
            !['localhost', '127.0.0.1'].includes(window.location.hostname);
        if (!voiceEnabled && isInsecure) {
            alert('La voz requiere HTTPS. Configurá tu dominio con HTTPS para habilitar esta función.');
            return;
        }
        setVoiceEnabled(!voiceEnabled);
    };

    const handleFileUpload = async (file: File) => {
        if (!file || !tenantId || !sessionKey) return;
        setIsUploading(true);
        try {
            const formData = new FormData();
            formData.append('file', file);
            const response = await fetch('/api/workspace/upload', {
                method: 'POST',
                headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                body: formData,
            });
            if (!response.ok) {
                const errData = await response.json().catch(() => ({})) as { detail?: string };
                throw new Error(errData.detail || 'Upload failed');
            }
            addSystemMessage(t('file_injected_success', { name: file.name }));
        } catch (err: unknown) {
            const message = err instanceof Error ? err.message : 'Unknown error';
            addSystemMessage(t('file_injection_error', { error: message }));
        } finally {
            setIsUploading(false);
            if (fileInputRef.current) fileInputRef.current.value = '';
        }
    };

    const handleDrop = async (e: React.DragEvent<HTMLDivElement>) => {
        e.preventDefault();
        const file = e.dataTransfer.files?.[0];
        if (file && !isUploading) await handleFileUpload(file);
    };

    return (
        // CORE-126: aegis-screen usa height:100% del body que ya tiene 100svh
        // Esto evita el overflow causado por h-screen o h-dvh con zoom del browser
        <div className="aegis-screen bg-black text-white font-sans" onDrop={handleDrop} onDragOver={(e) => e.preventDefault()}>

            {/* Telemetría — shrink-0: toma su espacio fijo */}
            <TelemetryDashboard />

            {/* Chat — flex-1 + min-h-0: ocupa el resto sin desbordar */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, overflow: 'hidden' }}>

                {/* Header */}
                <header className="shrink-0 border-b border-white/5 flex items-center justify-between px-4 sm:px-8 bg-black/40 backdrop-blur-3xl z-50" style={{ height: '56px' }}>
                    <div className="flex items-center gap-4 sm:gap-6">
                        <div className="flex items-center gap-2 sm:gap-4">
                            <AegisLogo variant="icon" className="w-5 h-5 text-aegis-cyan drop-shadow-[0_0_8px_rgba(0,242,254,0.4)]" />
                            <h1 className="text-[10px] font-mono tracking-[0.4em] text-white font-bold uppercase hidden sm:block">Aegis Shell v0.1.15</h1>
                        </div>
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <button 
                            onClick={() => useAegisStore.getState().setCurrentView('dashboard')}
                            className="group flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
                        >
                            <LayoutDashboard className="w-3.5 h-3.5 group-hover:scale-110 transition-transform" />
                            <span className="text-[10px] font-mono uppercase tracking-[0.2em] hidden sm:block">Dashboard</span>
                        </button>
                    </div>
                    <div className="flex items-center gap-2 sm:gap-4">
                        <AgentBadge />
                        <StatusBadge status={status} />
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <div className="flex items-center gap-2">
                            <span className="text-[8px] font-mono text-white/40 uppercase tracking-widest hidden md:block">{tenantId} // Active Domain</span>
                            <button onClick={() => setShowQR(true)} className="p-1.5 rounded-md hover:bg-white/5 text-white/20 hover:text-aegis-cyan transition-colors hidden sm:block" title="Mobile Connection">
                                <QrCode className="w-3.5 h-3.5" />
                            </button>
                            <button onClick={logout} className="p-1.5 rounded-md hover:bg-white/5 text-white/20 hover:text-red-400 transition-colors" title="Disconnect">
                                <LogOut className="w-3 h-3" />
                            </button>
                        </div>
                    </div>
                </header>

                <AnimatePresence>
                    {showLocalHistoryWarning && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            transition={{ duration: 0.2 }}
                            className="shrink-0 bg-amber-500/10 border-b border-amber-500/20 px-8 py-3 flex items-center justify-between gap-4 text-[10px] font-mono text-amber-300 uppercase tracking-widest leading-relaxed overflow-hidden"
                        >
                            <div className="flex items-center gap-3">
                                <AlertTriangle className="w-4 h-4 text-amber-400 flex-shrink-0" />
                                <span>
                                    ⚠️ Acceso local (HTTP). El historial de Cloudflare no está disponible aquí. Para historial unificado, usá siempre el link de Cloudflare.
                                </span>
                            </div>
                            <button
                                onClick={() => {
                                    localStorage.setItem('aegis_dismiss_http_warn', 'true');
                                    setShowLocalHistoryWarning(false);
                                }}
                                className="text-white/20 hover:text-white transition-colors p-1"
                                title="Ocultar advertencia"
                            >
                                <X className="w-3.5 h-3.5" />
                            </button>
                        </motion.div>
                    )}
                </AnimatePresence>

                {/* Mensajes — flex-1 + overflow-y-auto */}
                <main ref={scrollRef} onScroll={handleScroll} style={{ flex: 1, overflowY: 'auto', minHeight: 0 }} className="px-6 py-8 space-y-8 scrollbar-hide relative">
                    <AnimatePresence initial={false}>
                        {messages.length === 0 ? (
                            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="h-full flex flex-col items-center justify-center gap-6">
                                <AegisLogo variant="full" className="w-24 h-24 text-white/5 opacity-20" />
                                <p className="text-[10px] font-mono text-white/20 uppercase tracking-[0.4em]">Standby for instruction...</p>
                            </motion.div>
                        ) : (
                            messages.map((msg, index) => <MessageItem key={msg.id + index} message={msg} />)
                        )}
                    </AnimatePresence>
                    <div className="h-12" />
                </main>

                {/* Agent Activity — CORE-202 */}
                <AgentActivityPanel />

                {/* CORE-248/299: banner de estado enriquecido — aparece tras 5s de espera */}
                {status === 'thinking' && showProcessingBanner && (
                    <motion.div
                        initial={{ opacity: 0, y: 4 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        className="shrink-0 flex items-center gap-3 px-6 py-3 bg-white/5 border-t border-white/5 text-white/50 text-xs font-mono"
                    >
                        <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan" />
                        <span>
                            {lastRoutingInfo
                                ? `${lastRoutingInfo.provider} / ${lastRoutingInfo.model_id}`
                                : 'Procesando...'}
                        </span>
                        {lastRoutingInfo?.latency_ms != null && (
                            <span className="text-white/20 ml-auto">{lastRoutingInfo.latency_ms}ms</span>
                        )}
                    </motion.div>
                )}

                {/* Input */}
                <div className="shrink-0 p-6 bg-gradient-to-t from-black via-black/90 to-transparent border-t border-white/5">
                    {voiceError && (
                        <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} className="max-w-4xl mx-auto mb-2 px-4 py-2 bg-red-500/10 border border-red-500/20 rounded-lg flex items-center gap-2 text-red-500 text-xs font-mono">
                            <AlertCircle className="w-3 h-3" />
                            <span>Siren Error: {voiceError}</span>
                        </motion.div>
                    )}
                    <div className="max-w-4xl mx-auto relative">
                        <div className="glass rounded-xl border border-white/10 flex flex-col sm:flex-row items-stretch sm:items-end p-2 gap-2 focus-within:border-aegis-cyan/30 transition-all shadow-2xl">
                            <textarea
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={handleKeyPress}
                                placeholder="Inject command to Ring 0..."
                                className="w-full bg-transparent border-none focus:ring-0 text-sm py-2 px-3 resize-none max-h-32 min-h-[40px] font-mono placeholder:text-white/20"
                                rows={1}
                            />
                            <div className="flex flex-wrap items-center justify-end gap-1.5 px-2 pb-1 sm:pb-0 sm:px-0 sm:flex-nowrap sm:gap-2">
                                <input type="file" className="hidden" ref={fileInputRef} onChange={(e) => { const f = e.target.files?.[0]; if (f) handleFileUpload(f); }} />
                                <button onClick={() => fileInputRef.current?.click()} disabled={isUploading} className={cn("p-2 rounded-lg transition-all", isUploading ? "text-aegis-cyan animate-pulse" : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10")} title="Inject File">
                                    {isUploading ? <Loader2 className="w-5 h-5 animate-spin" /> : <Paperclip className="w-5 h-5" />}
                                </button>
                                <InputModeSelector />
                                <button
                                    onClick={handleVoiceToggle}
                                    className={cn(
                                        "p-2 rounded-lg transition-all",
                                        voiceEnabled
                                            ? "bg-aegis-cyan/20 text-aegis-cyan hover:bg-aegis-cyan/30"
                                            : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                                    )}
                                    title={voiceEnabled ? "Desactivar voz" : "Activar voz"}
                                >
                                    {voiceEnabled ? <Volume2 className="w-5 h-5" /> : <VolumeX className="w-5 h-5" />}
                                </button>
                                {(inputMode === 'audio' || inputMode === 'conversation') && (
                                    <button
                                        onClick={handleToggleMic}
                                        disabled={!sttAvailable}
                                        className={cn(
                                            "p-2 rounded-lg transition-all",
                                            status === 'listening'    && "bg-green-500 text-white animate-pulse shadow-[0_0_15px_rgba(34,197,94,0.5)]",
                                            status === 'transcribing' && "bg-yellow-500 text-black",
                                            isRecording && status !== 'listening' && status !== 'transcribing' && "bg-red-500/20 text-red-400",
                                            !isRecording && status !== 'listening' && status !== 'transcribing' && "bg-white/5 text-white/40 hover:text-white hover:bg-white/10",
                                            !sttAvailable && "opacity-30 cursor-not-allowed"
                                        )}
                                        title={isRecording ? "Stop" : "Voice"}
                                    >
                                        {status === 'transcribing'
                                            ? <Loader2 className="w-5 h-5 animate-spin" />
                                            : isRecording
                                                ? <MicOff className="w-5 h-5" />
                                                : <Mic className="w-5 h-5" />
                                        }
                                    </button>
                                )}
                                {/* CORE-300: model selector */}
                                <div className="relative shrink-0" ref={modelPickerRef}>
                                    <button
                                        onClick={() => setShowModelPicker(v => !v)}
                                        className={cn(
                                            "flex items-center gap-1 px-2 py-1.5 rounded-lg transition-all text-[10px] font-mono max-w-[160px]",
                                            selectedModelId
                                                ? "bg-aegis-cyan/10 text-aegis-cyan border border-aegis-cyan/20"
                                                : "bg-white/5 text-white/40 hover:text-aegis-cyan hover:bg-aegis-cyan/10"
                                        )}
                                        title="Seleccionar modelo"
                                    >
                                        <span className="truncate">
                                            {selectedModelId
                                                ? (availableModels.find(m => m.model_id === selectedModelId)?.display_name
                                                    || selectedModelId.split('/').slice(-1)[0])
                                                : '⚡ Auto'}
                                        </span>
                                        <ChevronDown className="w-3 h-3 flex-shrink-0 opacity-60" />
                                    </button>
                                    {showModelPicker && (
                                        <div className="absolute bottom-full right-0 mb-2 w-64 bg-[#0d1117] border border-white/10 rounded-xl shadow-2xl overflow-hidden z-50">
                                            <div className="max-h-72 overflow-y-auto">
                                                <button
                                                    onClick={() => { setSelectedModelId(null); setShowModelPicker(false); }}
                                                    className="w-full flex items-center justify-between px-3 py-2.5 text-[10px] font-mono hover:bg-white/5 transition-colors"
                                                >
                                                    <span className="text-aegis-cyan font-bold">⚡ Auto (CMR)</span>
                                                    {selectedModelId === null && <Check className="w-3 h-3 text-aegis-cyan" />}
                                                </button>
                                                {availableModels.length > 0 && (
                                                    <>
                                                        <div className="border-t border-white/10" />
                                                        {Object.entries(
                                                            availableModels.reduce<Record<string, CatalogModel[]>>((acc, m) => {
                                                                (acc[m.provider] = acc[m.provider] || []).push(m);
                                                                return acc;
                                                            }, {})
                                                        ).map(([provider, providerModels]) => (
                                                            <div key={provider}>
                                                                <div className="px-3 py-1.5 text-[9px] font-mono text-white/30 uppercase tracking-widest bg-white/2">
                                                                    {provider}
                                                                </div>
                                                                {providerModels.map(m => (
                                                                    <button
                                                                        key={m.model_id}
                                                                        onClick={() => { setSelectedModelId(m.model_id); setShowModelPicker(false); }}
                                                                        className="w-full flex items-center justify-between px-3 py-2 text-[10px] font-mono hover:bg-white/5 transition-colors"
                                                                    >
                                                                        <span className="text-white/70 truncate flex-1 text-left">
                                                                            {m.display_name || m.model_id.split('/').slice(-1)[0] || m.model_id}
                                                                            {m.cost_input_per_mtok === 0 && (
                                                                                <span className="ml-1 text-green-400/60">(free)</span>
                                                                            )}
                                                                        </span>
                                                                        {selectedModelId === m.model_id && (
                                                                            <Check className="w-3 h-3 text-aegis-cyan flex-shrink-0 ml-1" />
                                                                        )}
                                                                    </button>
                                                                ))}
                                                            </div>
                                                        ))}
                                                    </>
                                                )}
                                            </div>
                                        </div>
                                    )}
                                </div>
                                <button onClick={() => setShowSettings(true)} className="p-2 rounded-lg bg-white/5 text-white/40 hover:text-aegis-cyan hover:bg-aegis-cyan/10 transition-all" title="Settings">
                                    <Settings className="w-5 h-5" />
                                </button>
                                <button onClick={handleSend} disabled={!input.trim() || status === 'thinking'} className={cn("p-2 rounded-lg transition-all", input.trim() ? "bg-aegis-cyan text-black hover:scale-105" : "bg-white/5 text-white/20")}>
                                    <Send className="w-5 h-5" />
                                </button>
                            </div>
                        </div>
                        <div className="mt-2 flex justify-center">
                            <span className="text-[9px] font-mono text-white/20 uppercase tracking-[0.2em]">Citadel Protocol Active</span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Settings Panel */}
            <AnimatePresence>
                {showSettings && (
                    <SettingsPanel
                        tenantId={tenantId!}
                        sessionKey={sessionKey!}
                        onClose={() => setShowSettings(false)}
                    />
                )}
            </AnimatePresence>

            <MusicPlayer />
            <AnimatePresence>
                {showQR && <ConnectionQR onClose={() => setShowQR(false)} />}
            </AnimatePresence>
        </div>
    );
};

export default ChatTerminal;
