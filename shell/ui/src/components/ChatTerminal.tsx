import React, { useRef, useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { motion, AnimatePresence } from 'framer-motion';
import { Send, Settings, AlertCircle, Cpu, Mic, Paperclip, Loader2, LogOut } from 'lucide-react';
import { useAegisStore, Message } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import { AegisLogo } from './AegisLogo';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import TelemetryDashboard from './TelemetryDashboard';
import TenantKeyManager from './RouterConfig/TenantKeyManager';
import UserPasswordChange from './UserPasswordChange';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

// ─── Status Badge ──────────────────────────────────────────────────────────

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
            <span className={cn(
                "text-[9px] font-bold font-mono uppercase tracking-tighter transition-all group-hover:text-white",
                status === 'error' ? "text-red-400" : "text-white/50"
            )}>
                {current.label}
            </span>
        </div>
    );
};

// ─── Message Item ──────────────────────────────────────────────────────────

const MessageItem: React.FC<{ message: Message }> = ({ message }) => {
    const isAssistant = message.role === 'assistant';
    const isUser = message.role === 'user';
    const isSystem = message.role === 'system';

    return (
        <motion.div
            initial={{ opacity: 0, x: isUser ? 10 : -10, y: 5 }}
            animate={{ opacity: 1, x: 0, y: 0 }}
            className={cn(
                "flex w-full gap-4 px-2",
                isUser ? "justify-end" : "justify-start"
            )}
        >
            <div className={cn(
                "max-w-[85%] flex flex-col gap-1.5",
                isUser ? "items-end" : "items-start"
            )}>
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
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                {message.content}
                            </ReactMarkdown>
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

// ─── Chat Terminal ─────────────────────────────────────────────────────────

const ChatTerminal: React.FC = () => {
    const { t } = useTranslation();
    const { messages, sendMessage, status, isRecording, startSirenStream, stopSirenStream, tenantId, sessionKey, addSystemMessage, logout } = useAegisStore();
    const scrollRef = useRef<HTMLDivElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const isAtBottom = useRef(true);
    const [input, setInput] = useState('');
    const [voiceError, setVoiceError] = useState<string | null>(null);
    const [isUploading, setIsUploading] = useState(false);
    const [showSettings, setShowSettings] = useState(false);

    const scrollToBottom = (behavior: ScrollBehavior = 'smooth') => {
        if (scrollRef.current) {
            scrollRef.current.scrollTo({
                top: scrollRef.current.scrollHeight,
                behavior,
            });
        }
    };

    const handleScroll = () => {
        if (scrollRef.current) {
            const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
            const distanceToBottom = scrollHeight - scrollTop - clientHeight;
            isAtBottom.current = distanceToBottom < 100;
        }
    };

    useEffect(() => {
        if (isAtBottom.current) {
            scrollToBottom('smooth');
        }
    }, [messages]);

    const handleSend = () => {
        if (!input.trim()) return;
        sendMessage(input);
        setInput('');
        setTimeout(() => {
            isAtBottom.current = true;
            scrollToBottom('auto');
        }, 10);
    };

    const handleKeyPress = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleToggleMic = async () => {
        if (isRecording) {
            stopSirenStream();
        } else {
            try {
                setVoiceError(null);
                await startSirenStream();
            } catch (err: unknown) {
                console.error("🎤 Mic Toggle Error:", err);
                const domError = err as { name?: string };
                setVoiceError(domError.name === 'NotAllowedError' ? 'Microphone access denied' : 'Hardware error');
                setTimeout(() => setVoiceError(null), 5000);
            }
        }
    };

    const handleFileUploadChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file || !tenantId || !sessionKey) return;

        setIsUploading(true);
        try {
            const formData = new FormData();
            formData.append('file', file);

            const response = await fetch('/api/workspace/upload', {
                method: 'POST',
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: formData,
            });

            if (!response.ok) {
                const errData = await response.json().catch(() => ({})) as { detail?: string };
                throw new Error(errData.detail || 'Upload failed');
            }

            addSystemMessage(t('file_injected_success', { name: file.name }));
        } catch (err: unknown) {
            console.error('File Upload Error:', err);
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
        if (!file || !tenantId || !sessionKey || isUploading) return;

        setIsUploading(true);
        try {
            const formData = new FormData();
            formData.append('file', file);

            const response = await fetch('/api/workspace/upload', {
                method: 'POST',
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: formData,
            });

            if (!response.ok) {
                const errData = await response.json().catch(() => ({})) as { detail?: string };
                throw new Error(errData.detail || 'Upload failed');
            }

            addSystemMessage(t('file_injected_success', { name: file.name }));
        } catch (err: unknown) {
            console.error('File Upload Error:', err);
            const message = err instanceof Error ? err.message : 'Unknown error';
            addSystemMessage(t('file_injection_error', { error: message }));
        } finally {
            setIsUploading(false);
        }
    };

    const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => e.preventDefault();

    return (
        <div
            className="flex flex-col h-screen bg-black text-white overflow-hidden font-sans"
            onDrop={handleDrop}
            onDragOver={handleDragOver}
        >
            <TelemetryDashboard />

            <div className="flex-1 flex flex-col min-w-0 h-full">
                <header className="h-16 shrink-0 border-b border-white/5 flex items-center justify-between px-8 bg-black/40 backdrop-blur-3xl z-50">
                    <div className="flex items-center gap-4">
                        <AegisLogo variant="icon" className="w-5 h-5 text-aegis-cyan drop-shadow-[0_0_8px_rgba(0,242,254,0.4)]" />
                        <div className="flex flex-col">
                            <h1 className="text-[10px] font-mono tracking-[0.4em] text-white font-bold uppercase">Aegis Shell v0.1.0</h1>
                        </div>
                    </div>
                    <div className="flex items-center gap-4">
                        <StatusBadge status={status} />
                        <div className="h-4 w-px bg-white/10" />
                        <div className="flex items-center gap-2">
                            <span className="text-[8px] font-mono text-white/40 uppercase tracking-widest">{tenantId} // Active Domain</span>
                            <button 
                                onClick={logout}
                                className="p-1.5 rounded-md hover:bg-white/5 text-white/20 hover:text-red-400 transition-colors"
                                title="Disconnect"
                            >
                                <LogOut className="w-3 h-3" />
                            </button>
                        </div>
                    </div>
                </header>

                <main 
                    ref={scrollRef}
                    onScroll={handleScroll}
                    className="flex-1 overflow-y-auto px-6 py-8 space-y-8 scrollbar-hide relative"
                >
                    <AnimatePresence initial={false}>
                        {messages.length === 0 ? (
                            <motion.div 
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                className="h-full flex flex-col items-center justify-center gap-6"
                            >
                                <AegisLogo variant="full" className="w-24 h-24 text-white/5 opacity-20" />
                                <p className="text-[10px] font-mono text-white/20 uppercase tracking-[0.4em]">Standby for instruction...</p>
                            </motion.div>
                        ) : (
                            messages.map((msg, index) => (
                                <MessageItem key={msg.id + index} message={msg} />
                            ))
                        )}
                    </AnimatePresence>

                    {status === 'thinking' && (
                        <motion.div
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="flex items-center gap-3 text-aegis-purple/70 px-8 py-5 bg-aegis-purple/5 rounded-2xl border border-aegis-purple/10 max-w-fit mx-auto shadow-[0_0_30px_rgba(191,0,255,0.05)]"
                        >
                            <Cpu className="w-5 h-5 animate-pulse text-aegis-purple" />
                            <span className="text-[11px] font-mono italic tracking-widest uppercase">ANK is processing payload...</span>
                        </motion.div>
                    )}
                    <div className="h-12" />
                </main>

                <div className="p-6 shrink-0 bg-gradient-to-t from-black via-black/90 to-transparent border-t border-white/5">
                    {voiceError && (
                        <motion.div
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="max-w-4xl mx-auto mb-2 px-4 py-2 bg-red-500/10 border border-red-500/20 rounded-lg flex items-center gap-2 text-red-500 text-xs font-mono"
                        >
                            <AlertCircle className="w-3 h-3" />
                            <span>Siren Error: {voiceError}</span>
                        </motion.div>
                    )}
                    <div className="max-w-4xl mx-auto relative">
                        <div className="glass rounded-xl border border-white/10 flex items-end p-2 gap-2 focus-within:border-aegis-cyan/30 transition-all shadow-2xl">
                            <textarea
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={handleKeyPress}
                                placeholder="Inject command to Ring 0..."
                                className="w-full bg-transparent border-none focus:ring-0 text-sm py-2 px-3 resize-none max-h-32 min-h-[40px] font-mono placeholder:text-white/20"
                                rows={1}
                            />

                            <input
                                type="file"
                                className="hidden"
                                ref={fileInputRef}
                                onChange={handleFileUploadChange}
                            />

                            <button
                                onClick={() => fileInputRef.current?.click()}
                                disabled={isUploading}
                                className={cn(
                                    "p-2 rounded-lg transition-all",
                                    isUploading ? "text-aegis-cyan animate-pulse" : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                                )}
                                title="Inject File to Workspace"
                            >
                                {isUploading ? <Loader2 className="w-5 h-5 animate-spin" /> : <Paperclip className="w-5 h-5" />}
                            </button>

                            <button
                                onClick={handleToggleMic}
                                className={cn(
                                    "p-2 rounded-lg transition-all",
                                    status === 'listening' 
                                        ? "bg-green-500 text-white shadow-[0_0_15px_rgba(34,197,94,0.5)] animate-pulse"
                                        : status === 'transcribing'
                                            ? "bg-yellow-500 text-black shadow-[0_0_15px_rgba(234,179,8,0.5)]"
                                            : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                                )}
                                title={isRecording ? "Stop Listening" : "Start Voice Interaction"}
                            >
                                {status === 'transcribing' ? (
                                    <Loader2 className="w-5 h-5 animate-spin" />
                                ) : (
                                    <Mic className="w-5 h-5" />
                                )}
                            </button>

                            <button
                                onClick={() => setShowSettings(true)}
                                className="p-2 rounded-lg bg-white/5 text-white/40 hover:text-aegis-cyan hover:bg-aegis-cyan/10 transition-all"
                                title="Engine Configuration"
                            >
                                <Settings className="w-5 h-5" />
                            </button>

                            <button
                                onClick={handleSend}
                                disabled={!input.trim() || status === 'thinking'}
                                className={cn(
                                    "p-2 rounded-lg transition-all",
                                    input.trim() ? "bg-aegis-cyan text-black hover:scale-105" : "bg-white/5 text-white/20"
                                )}
                            >
                                <Send className="w-5 h-5" />
                            </button>
                        </div>
                        <div className="mt-2 flex justify-center">
                            <span className="text-[9px] font-mono text-white/20 uppercase tracking-[0.2em]">Citadel Protocol Active</span>
                        </div>
                    </div>
                </div>

                <AnimatePresence>
                    {showSettings && (
                        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-md">
                            <motion.div 
                                initial={{ opacity: 0, scale: 0.9 }}
                                animate={{ opacity: 1, scale: 1 }}
                                exit={{ opacity: 0, scale: 0.9 }}
                                className="w-full max-w-xl"
                            >
                                <div className="bg-aegis-steel border border-white/10 rounded-2xl overflow-hidden shadow-2xl relative max-h-[90vh] overflow-y-auto">
                                    <div className="p-6 border-b border-white/5 flex justify-between items-center bg-black/40 sticky top-0 z-10">
                                        <h3 className="text-sm font-mono font-bold tracking-widest text-aegis-cyan uppercase">{t('tenant_security')}</h3>
                                        <div className="flex items-center gap-4">
                                            <button 
                                                onClick={() => {
                                                    const newLang = localStorage.getItem('aegis_language') === 'en' ? 'es' : 'en';
                                                    localStorage.setItem('aegis_language', newLang);
                                                    window.dispatchEvent(new Event('storage'));
                                                    window.location.reload(); 
                                                }}
                                                className="text-[10px] font-mono text-white/40 hover:text-white uppercase px-2 py-1 border border-white/10 rounded"
                                            >
                                                Lang: {localStorage.getItem('aegis_language') === 'en' ? 'EN' : 'ES'}
                                            </button>
                                            <button onClick={() => setShowSettings(false)} className="text-white/20 hover:text-white">&times;</button>
                                        </div>
                                    </div>
                                    <div className="p-8 space-y-6">
                                        <div>
                                            <p className="text-[10px] font-mono text-white/40 uppercase mb-4 tracking-widest">{t('personal_keys_notice')}</p>
                                            <TenantKeyManager tenantId={tenantId!} sessionKey={sessionKey!} />
                                        </div>
                                        <UserPasswordChange />
                                    </div>
                                </div>
                            </motion.div>
                        </div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
};

export default ChatTerminal;
