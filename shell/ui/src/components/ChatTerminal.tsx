import React, { useRef, useEffect, useLayoutEffect, useState, useCallback, Suspense, lazy } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { motion, AnimatePresence } from 'framer-motion';
import { Send, Settings, AlertCircle, Mic, MicOff, Paperclip, Loader2, LogOut, LayoutDashboard, Volume2, VolumeX, ChevronDown, Check, X, Brain, Eraser, ArrowUp, History, RefreshCw, Puzzle, QrCode } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore, Message } from '../store/useAegisStore';
import { AgentBadge } from './AgentBadge';
import { useTranslation } from '../i18n';
import { AegisLogo } from './AegisLogo';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { AssistantAvatar } from './AssistantAvatar';
import { loadVoicePrefs } from '../audio/naturalVoice';
import { extractVideoCardsFromMessage } from '../utils/videoCards';

const TelemetryDashboard = lazy(() => import('./TelemetryDashboard'));
const SettingsPanel = lazy(() => import('./SettingsPanel'));
const MusicPlayer = lazy(() => import('./MusicPlayer'));
const ConnectionQR = lazy(() =>
    import('./ConnectionQR').then((m) => ({ default: m.ConnectionQR })),
);
const AgentActivityPanel = lazy(() =>
    import('./AgentActivityPanel').then((m) => ({ default: m.AgentActivityPanel })),
);
const PersonalAssistantDrawer = lazy(() =>
    import('./PersonalAssistantDrawer').then((m) => ({ default: m.PersonalAssistantDrawer })),
);
const ModulesAppHub = lazy(() => import('./ModulesAppHub'));
const ChatVideoResultCard = lazy(() =>
    import('./video_studio/ChatVideoResultCard').then((m) => ({ default: m.ChatVideoResultCard })),
);

function renderTextWithLineBreaks(node: React.ReactNode): React.ReactNode {
    if (typeof node === 'string') {
        if (!/<br\s*\/?>/i.test(node)) return node;
        const parts = node.split(/<br\s*\/?>/gi);
        return parts.map((part, idx) => (
            <React.Fragment key={idx}>
                {part}
                {idx < parts.length - 1 && <br />}
            </React.Fragment>
        ));
    }
    if (Array.isArray(node)) {
        return node.map((child, idx) => (
            <React.Fragment key={idx}>{renderTextWithLineBreaks(child)}</React.Fragment>
        ));
    }
    if (React.isValidElement<{ children?: React.ReactNode }>(node) && node.props?.children != null) {
        if (node.type === 'code' || node.type === 'pre') {
            return node;
        }
        return React.cloneElement(node, undefined, renderTextWithLineBreaks(node.props.children));
    }
    return node;
}

const MARKDOWN_COMPONENTS: Components = {
    table: ({ children }) => (
        <div className="my-2.5 w-full overflow-x-auto rounded-xl border border-white/15 bg-black/40">
            <table className="w-full border-collapse text-left text-xs leading-relaxed">
                {children}
            </table>
        </div>
    ),
    thead: ({ children }) => (
        <thead className="bg-white/10 text-aegis-cyan font-semibold uppercase tracking-wider text-[11px]">
            {children}
        </thead>
    ),
    tbody: ({ children }) => (
        <tbody className="divide-y divide-white/10">{children}</tbody>
    ),
    tr: ({ children }) => (
        <tr className="transition-colors hover:bg-white/[0.04]">{children}</tr>
    ),
    th: ({ children, style }) => (
        <th style={style} className="border-b border-white/15 px-3 py-2 font-mono whitespace-nowrap">
            {renderTextWithLineBreaks(children)}
        </th>
    ),
    td: ({ children, style }) => (
        <td style={style} className="px-3 py-2 align-top text-white/85">
            {renderTextWithLineBreaks(children)}
        </td>
    ),
    p: ({ children }) => (
        <p className="my-1.5 leading-relaxed first:mt-0 last:mb-0">
            {renderTextWithLineBreaks(children)}
        </p>
    ),
    ul: ({ children }) => (
        <ul className="my-2 list-disc pl-5 space-y-1 text-white/90">{children}</ul>
    ),
    ol: ({ children }) => (
        <ol className="my-2 list-decimal pl-5 space-y-1 text-white/90">{children}</ol>
    ),
    li: ({ children }) => (
        <li className="leading-relaxed">{renderTextWithLineBreaks(children)}</li>
    ),
    blockquote: ({ children }) => (
        <blockquote className="my-2 border-l-2 border-aegis-cyan/60 bg-aegis-cyan/5 pl-3 pr-2 py-1.5 rounded-r-lg text-white/80 italic">
            {renderTextWithLineBreaks(children)}
        </blockquote>
    ),
    h1: ({ children }) => (
        <h1 className="mt-3 mb-1.5 text-base font-bold text-white first:mt-0">{children}</h1>
    ),
    h2: ({ children }) => (
        <h2 className="mt-2.5 mb-1.5 text-sm font-bold text-aegis-cyan first:mt-0">{children}</h2>
    ),
    h3: ({ children }) => (
        <h3 className="mt-2 mb-1 text-xs font-bold uppercase tracking-wider text-white/90 first:mt-0">{children}</h3>
    ),
    hr: () => <hr className="my-3 border-white/10" />,
    a: ({ href, children }) => (
        <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-aegis-cyan underline decoration-aegis-cyan/40 underline-offset-2 hover:decoration-aegis-cyan"
        >
            {children}
        </a>
    ),
    code: ({ className, children, ...props }) => {
        const isBlock = Boolean(className?.includes('language-'));
        if (isBlock) {
            return (
                <code className={`font-mono text-xs text-white/90 ${className ?? ''}`} {...props}>
                    {children}
                </code>
            );
        }
        return (
            <code
                className="rounded bg-aegis-cyan/10 px-1.5 py-0.5 font-mono text-[12px] text-aegis-cyan"
                {...props}
            >
                {children}
            </code>
        );
    },
    pre: ({ children }) => (
        <pre className="my-2 overflow-x-auto rounded-lg border border-white/10 bg-black/50 p-3 font-mono text-xs leading-relaxed [&_code]:bg-transparent [&_code]:p-0 [&_code]:text-white/90">
            {children}
        </pre>
    ),
};

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
    const lastError = useAegisStore((s) => s.lastError);
    const config: Record<string, { color: string, label: string }> = {
        idle: { color: 'bg-green-500', label: 'En línea' },
        thinking: { color: 'bg-aegis-purple animate-pulse shadow-[0_0_8px_rgba(191,0,255,0.6)]', label: 'Pensando...' },
        executing_syscall: { color: 'bg-aegis-cyan animate-pulse', label: 'Ejecutando acción...' },
        disconnected: { color: 'bg-red-500', label: 'Desconectado' },
        connecting: { color: 'bg-yellow-500 animate-bounce', label: 'Conectando...' },
        error: { color: 'bg-red-600 shadow-[0_0_10px_rgba(255,0,0,0.5)]', label: lastError || 'Error de conexión' },
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

const MessageItem: React.FC<{
    message: Message;
    tenantId?: string | null;
    sessionKey?: string | null;
    agentName?: string | null;
    avatarId?: string;
    customAvatarUrl?: string;
    skipInitialAnim?: boolean;
    onOpenVideoStudio?: (projectId: string) => void;
}> = ({ message, tenantId, sessionKey, agentName, avatarId, customAvatarUrl, skipInitialAnim, onOpenVideoStudio }) => {
    const isAssistant = message.role === 'assistant';
    const isUser = message.role === 'user';
    const isSystem = message.role === 'system';
    const { cleanContent, videoProjectIds } =
        message.type === 'thought'
            ? { cleanContent: message.content, videoProjectIds: [] as string[] }
            : extractVideoCardsFromMessage(message.content, isAssistant);
    const isWideMessage = cleanContent.length > 280 || videoProjectIds.length > 0;

    return (
        <motion.div
            initial={skipInitialAnim ? false : { opacity: 0, x: isUser ? 10 : -10, y: 5 }}
            animate={{ opacity: 1, x: 0, y: 0 }}
            className={cn("flex w-full gap-3 px-2", isUser ? "justify-end" : "justify-start")}
        >
            {isAssistant && (
                <div className="mt-5">
                    <AssistantAvatar avatarId={avatarId} customAvatarUrl={customAvatarUrl} size="sm" />
                </div>
            )}
            <div className={cn(
                "min-w-0 flex flex-col gap-1.5",
                isWideMessage ? "max-w-[95%] sm:max-w-[90%]" : "max-w-[85%]",
                isUser ? "items-end" : "items-start"
            )}>
                <div className="flex items-center gap-2 px-1">
                    {isUser && <span className="text-[10px] font-mono text-white/40 uppercase">{tenantId || "Vos"}</span>}
                    {isAssistant && <span className="text-[10px] font-mono text-aegis-cyan/80 font-bold uppercase">{agentName || "Aegis"}</span>}
                    {isSystem && <span className="text-[10px] font-mono text-aegis-purple/60 uppercase">System log</span>}
                </div>
                <div className={cn(
                    "max-w-full rounded-2xl px-4 py-3 text-sm transition-all shadow-lg",
                    isUser && "bg-aegis-cyan/10 border border-aegis-cyan/20 text-white rounded-tr-none",
                    isAssistant && message.type === 'text' && "bg-white/5 border border-white/10 text-white/90 rounded-tl-none",
                    isAssistant && message.type === 'thought' && "bg-aegis-purple/5 border border-aegis-purple/10 text-aegis-purple/60 text-xs italic font-mono rounded-tl-none",
                    isSystem && "bg-black/50 border border-aegis-cyan/40 text-aegis-cyan flex items-center gap-3",
                    message.type === 'error' && "bg-red-500/10 border border-red-500/30 text-red-500 italic"
                )}>
                    {isSystem && <Settings className="w-4 h-4" />}
                    {message.type === 'error' && <AlertCircle className="w-4 h-4 inline-block mr-2" />}
                    <div className="min-w-0 w-full text-sm">
                        {message.type === 'thought' ? (
                            <p>{cleanContent}</p>
                        ) : (
                            <ReactMarkdown remarkPlugins={[remarkGfm]} components={MARKDOWN_COMPONENTS}>{cleanContent}</ReactMarkdown>
                        )}
                        {videoProjectIds.map((pid) => (
                            <Suspense key={pid} fallback={null}>
                                <ChatVideoResultCard
                                    projectId={pid}
                                    tenantId={tenantId ?? null}
                                    sessionKey={sessionKey ?? null}
                                    onOpenInStudio={onOpenVideoStudio}
                                />
                            </Suspense>
                        ))}
                    </div>
                </div>
                <span className="text-[9px] font-mono text-white/10 px-1 mt-0.5">
                    {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                </span>
            </div>
        </motion.div>
    );
};

const HistoryLoadButton: React.FC<{
    variant: 'empty' | 'top';
    isLoading: boolean;
    remainingCount: number | null;
    onLoad: () => void;
}> = ({ variant, isLoading, remainingCount, onLoad }) => {
    const Icon = variant === 'empty' ? History : ArrowUp;
    const idleLabel = variant === 'empty'
        ? `Subí con la rueda o tocá para ver lo anterior (${remainingCount ?? 0})`
        : `Subí o tocá para cargar anteriores (${remainingCount ?? 0} guardados)`;

    return (
        <button
            onClick={onLoad}
            disabled={isLoading}
            className={cn(
                "inline-flex items-center gap-2 rounded-full border font-mono transition-all",
                variant === 'empty'
                    ? "px-4 py-2 bg-white/[0.04] hover:bg-aegis-cyan/10 border-white/10 hover:border-aegis-cyan/30 text-[11px] text-white/60 hover:text-aegis-cyan"
                    : "px-3.5 py-1.5 bg-white/[0.04] hover:bg-white/[0.08] border-white/10 text-[10px] text-white/50 hover:text-aegis-cyan"
            )}
        >
            {isLoading ? (
                <>
                    <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan" />
                    <span>Cargando conversación anterior...</span>
                </>
            ) : (
                <>
                    <Icon className="w-3.5 h-3.5 text-aegis-cyan" />
                    <span>{idleLabel}</span>
                </>
            )}
        </button>
    );
};

const ChatTerminal: React.FC = () => {
    const { t } = useTranslation();
    const {
        messages, sendMessage, status, isRecording, startSirenStream, stopSirenStream,
        tenantId, sessionKey, addSystemMessage, logout, fetchSirenConfig, lastRoutingInfo,
        voiceEnabled, setVoiceEnabled, agentName, fetchAgentName,
        nextHistoryBeforeIndex, hasMoreHistory, isLoadingHistory, cleanOnOpen, historyPrependedSeq,
        loadOlderHistory, clearChatView, setCleanOnOpen,
        pendingRetryPrompt, retryPendingMessage, dismissPendingRetry, liveToolProgress,
    } = useAegisStore(
        useShallow((s) => ({
            messages: s.messages,
            sendMessage: s.sendMessage,
            status: s.status,
            isRecording: s.isRecording,
            startSirenStream: s.startSirenStream,
            stopSirenStream: s.stopSirenStream,
            tenantId: s.tenantId,
            sessionKey: s.sessionKey,
            addSystemMessage: s.addSystemMessage,
            logout: s.logout,
            fetchSirenConfig: s.fetchSirenConfig,
            lastRoutingInfo: s.lastRoutingInfo,
            voiceEnabled: s.voiceEnabled,
            setVoiceEnabled: s.setVoiceEnabled,
            agentName: s.agentName,
            fetchAgentName: s.fetchAgentName,
            nextHistoryBeforeIndex: s.nextHistoryBeforeIndex,
            hasMoreHistory: s.hasMoreHistory,
            isLoadingHistory: s.isLoadingHistory,
            cleanOnOpen: s.cleanOnOpen,
            historyPrependedSeq: s.historyPrependedSeq,
            loadOlderHistory: s.loadOlderHistory,
            clearChatView: s.clearChatView,
            setCleanOnOpen: s.setCleanOnOpen,
            pendingRetryPrompt: s.pendingRetryPrompt,
            retryPendingMessage: s.retryPendingMessage,
            dismissPendingRetry: s.dismissPendingRetry,
            liveToolProgress: s.liveToolProgress,
        })),
    );
    const [avatarPrefs, setAvatarPrefs] = useState(() => loadVoicePrefs());
    const scrollRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);
    const modelPickerRef = useRef<HTMLDivElement>(null);
    const isAtBottom = useRef(true);
    const scrollAnchorRef = useRef<{ mode: 'prepend' | 'reveal-from-empty'; prevScrollHeight: number; prevScrollTop: number } | null>(null);
    const isAdjustingScrollRef = useRef(false);
    const lastLoadTriggerRef = useRef(0);
    const touchStartYRef = useRef<number | null>(null);
    const thinkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const processingBannerTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const [input, setInput] = useState('');
    const isLongInput = input.length > 85 || input.includes('\n');
    const [showProcessingBanner, setShowProcessingBanner] = useState(false);
    const [voiceError, setVoiceError] = useState<string | null>(null);
    const [isUploading, setIsUploading] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [showPersonalDrawer, setShowPersonalDrawer] = useState(false);
    const [showModulesHub, setShowModulesHub] = useState(false);
    const [modulesInitialView, setModulesInitialView] = useState<string>('store');
    const [targetVideoProjectId, setTargetVideoProjectId] = useState<string | null>(null);
    const [showQR, setShowQR] = useState(false);
    const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
    const [availableModels, setAvailableModels] = useState<CatalogModel[]>([]);
    const [showModelPicker, setShowModelPicker] = useState(false);

    const handleSelectModel = (modelId: string | null) => {
        setSelectedModelId(modelId);
        setShowModelPicker(false);
        if (pendingRetryPrompt && status !== 'thinking') {
            retryPendingMessage(modelId);
        }
    };

    const scrollToBottom = (behavior: ScrollBehavior = 'smooth') => {
        if (scrollRef.current) {
            scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior });
        }
    };

    const triggerLoadOlder = useCallback(async () => {
        const now = Date.now();
        if (isLoadingHistory || !hasMoreHistory || now - lastLoadTriggerRef.current < 350) return;
        const el = scrollRef.current;
        if (!el) return;
        lastLoadTriggerRef.current = now;
        scrollAnchorRef.current = {
            mode: messages.length === 0 ? 'reveal-from-empty' : 'prepend',
            prevScrollHeight: el.scrollHeight,
            prevScrollTop: el.scrollTop,
        };
        isAtBottom.current = false;
        const added = await loadOlderHistory(25);
        if (added === 0) {
            scrollAnchorRef.current = null;
        }
    }, [isLoadingHistory, hasMoreHistory, messages.length, loadOlderHistory]);

    const handleScroll = () => {
        if (!scrollRef.current || isAdjustingScrollRef.current) return;
        const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
        isAtBottom.current = scrollHeight - scrollTop - clientHeight < 100;
        if (scrollTop <= 60 && hasMoreHistory && !isLoadingHistory) {
            void triggerLoadOlder();
        }
    };

    const handleWheel = (e: React.WheelEvent<HTMLElement>) => {
        if (!scrollRef.current || e.ctrlKey) return;
        if (scrollRef.current.scrollTop <= 2 && e.deltaY < -8 && hasMoreHistory && !isLoadingHistory) {
            void triggerLoadOlder();
        }
    };

    const handleTouchStart = (e: React.TouchEvent<HTMLElement>) => {
        if (scrollRef.current && scrollRef.current.scrollTop <= 2) {
            touchStartYRef.current = e.touches[0]?.clientY ?? null;
        } else {
            touchStartYRef.current = null;
        }
    };

    const handleTouchEnd = (e: React.TouchEvent<HTMLElement>) => {
        const startY = touchStartYRef.current;
        touchStartYRef.current = null;
        if (startY == null || !scrollRef.current) return;
        const endY = e.changedTouches[0]?.clientY ?? startY;
        if (scrollRef.current.scrollTop <= 2 && endY - startY > 45 && hasMoreHistory && !isLoadingHistory) {
            void triggerLoadOlder();
        }
    };

    useLayoutEffect(() => {
        if (historyPrependedSeq === 0) return;
        const anchor = scrollAnchorRef.current;
        const el = scrollRef.current;
        if (!anchor || !el) return;
        scrollAnchorRef.current = null;
        isAdjustingScrollRef.current = true;
        lastLoadTriggerRef.current = Date.now();
        if (anchor.mode === 'prepend') {
            const delta = el.scrollHeight - anchor.prevScrollHeight;
            el.scrollTop = Math.max(0, anchor.prevScrollTop + delta);
        } else {
            el.scrollTop = el.scrollHeight;
        }
        requestAnimationFrame(() => {
            isAdjustingScrollRef.current = false;
        });
    }, [historyPrependedSeq]);

    useEffect(() => { if (isAtBottom.current && !scrollAnchorRef.current) scrollToBottom('smooth'); }, [messages]);
    useLayoutEffect(() => {
        const el = textareaRef.current;
        if (!el) return;
        const resize = () => {
            el.style.height = 'auto';
            const nextHeight = input ? Math.min(Math.max(el.scrollHeight, 44), 280) : 44;
            el.style.height = `${nextHeight}px`;
        };
        resize();
        window.addEventListener('resize', resize);
        return () => window.removeEventListener('resize', resize);
    }, [input, isLongInput]);
    useEffect(() => {
        if (tenantId && sessionKey) {
            fetchSirenConfig();
            fetchAgentName();
        }
    }, [tenantId, sessionKey, fetchSirenConfig, fetchAgentName]);
    useEffect(() => {
        const handlePrefsChanged = () => setAvatarPrefs(loadVoicePrefs());
        window.addEventListener('aegis-voice-prefs-changed', handlePrefsChanged);
        return () => window.removeEventListener('aegis-voice-prefs-changed', handlePrefsChanged);
    }, []);
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
        if (status === 'thinking' && liveToolProgress) {
            setShowProcessingBanner(true);
            if (thinkingTimeoutRef.current) clearTimeout(thinkingTimeoutRef.current);
            thinkingTimeoutRef.current = setTimeout(() => {
                if (useAegisStore.getState().status === 'thinking') {
                    useAegisStore.getState().addSystemMessage('El motor tardó demasiado en responder. Podés intentar de nuevo.');
                    useAegisStore.getState().setStatus('idle');
                }
                thinkingTimeoutRef.current = null;
            }, 180_000);
        }
    }, [status, liveToolProgress]);

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
        if (!input.trim() || useAegisStore.getState().status === 'thinking') return;
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
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            if (status !== 'thinking') {
                handleSend();
            }
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
                const error = err as Error;
                let msg: string;
                if (error.message?.startsWith('INSECURE_CONTEXT')) {
                    msg = 'El micrófono en navegador requiere http://localhost:8000 o HTTPS.';
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
            <Suspense fallback={null}>
                <TelemetryDashboard />
            </Suspense>

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
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <button 
                            onClick={() => setShowPersonalDrawer(true)}
                            className="group flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
                            title="Mi Memoria, Gastos y Segundo Plano"
                        >
                            <Brain className="w-3.5 h-3.5 text-aegis-cyan group-hover:scale-110 transition-transform" />
                            <span className="text-[10px] font-mono uppercase tracking-[0.2em] hidden sm:block text-aegis-cyan/90">Mi Asistente</span>
                        </button>
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <button
                            onClick={() => setShowModulesHub(true)}
                            className="group flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
                            title="Catálogo de Módulos y Mini-Apps (Creator Studio, Dev Studio, Negocio)"
                        >
                            <Puzzle className="w-3.5 h-3.5 text-pink-300 group-hover:scale-110 transition-transform" />
                            <span className="text-[10px] font-mono uppercase tracking-[0.2em] hidden sm:block text-white/80 group-hover:text-aegis-cyan">Módulos</span>
                        </button>
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <button
                            onClick={() => setShowSettings(true)}
                            className="group flex items-center gap-2 px-2.5 py-1 rounded-full bg-white/[0.04] hover:bg-white/[0.08] border border-white/10 transition-all"
                            title="Personalizar nombre, avatar, personalidad y voz"
                        >
                            <AssistantAvatar
                                avatarId={avatarPrefs.avatarId}
                                customAvatarUrl={avatarPrefs.customAvatarUrl}
                                size="xs"
                            />
                            <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-white/90 group-hover:text-aegis-cyan transition-colors">
                                {agentName || 'Aegis'}
                            </span>
                        </button>
                    </div>
                    <div className="flex items-center gap-2 sm:gap-4">
                        <AgentBadge />
                        <StatusBadge status={status} />
                        <div className="h-4 w-px bg-white/10 hidden sm:block" />
                        <div className="flex items-center gap-2">
                            <button
                                onClick={() => { void clearChatView(); }}
                                disabled={messages.length === 0}
                                className={cn(
                                    "flex items-center gap-1.5 px-2.5 py-1 rounded-md border text-[10px] font-mono uppercase tracking-wider transition-all",
                                    messages.length > 0
                                        ? "bg-white/[0.04] border-white/10 text-white/70 hover:text-aegis-cyan hover:border-aegis-cyan/30 hover:bg-aegis-cyan/10"
                                        : "bg-transparent border-white/5 text-white/20 cursor-default"
                                )}
                                title="Limpiar chat visualmente sin borrar el historial guardado"
                            >
                                <Eraser className="w-3.5 h-3.5" />
                                <span className="hidden md:inline">Limpiar vista</span>
                            </button>
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

                {/* Mensajes — flex-1 + overflow-y-auto */}
                <main
                    ref={scrollRef}
                    onScroll={handleScroll}
                    onWheel={handleWheel}
                    onTouchStart={handleTouchStart}
                    onTouchEnd={handleTouchEnd}
                    style={{ flex: 1, overflowY: 'auto', minHeight: 0 }}
                    className="px-6 py-8 space-y-8 scrollbar-hide relative"
                >
                    {messages.length > 0 && hasMoreHistory && (
                        <div className="flex justify-center pb-2">
                            <HistoryLoadButton
                                variant="top"
                                isLoading={isLoadingHistory}
                                remainingCount={nextHistoryBeforeIndex}
                                onLoad={() => { void triggerLoadOlder(); }}
                            />
                        </div>
                    )}
                    <AnimatePresence initial={false}>
                        {messages.length === 0 ? (
                            <motion.div key="empty-clean-state" initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="h-full flex flex-col items-center justify-center gap-5 text-center px-4">
                                <AegisLogo variant="full" className="w-24 h-24 text-white/5 opacity-20" />
                                <p className="text-[10px] font-mono text-white/25 uppercase tracking-[0.35em]">
                                    {hasMoreHistory ? 'Vista limpia · historial guardado' : 'Standby for instruction...'}
                                </p>
                                {hasMoreHistory && (
                                    <HistoryLoadButton
                                        variant="empty"
                                        isLoading={isLoadingHistory}
                                        remainingCount={nextHistoryBeforeIndex}
                                        onLoad={() => { void triggerLoadOlder(); }}
                                    />
                                )}
                                <button
                                    onClick={() => { void setCleanOnOpen(!cleanOnOpen); }}
                                    className="text-[10px] font-mono text-white/30 hover:text-white/60 transition-colors"
                                    title="Define si al abrir Aegis la vista inicia limpia ocultando mensajes anteriores"
                                >
                                    Al abrir: {cleanOnOpen ? 'Vista limpia activada (cambiar)' : 'Mostrar últimos mensajes (cambiar)'}
                                </button>
                            </motion.div>
                        ) : (
                            messages.map((msg) => (
                                <MessageItem
                                    key={msg.id}
                                    message={msg}
                                    tenantId={tenantId}
                                    sessionKey={sessionKey}
                                    agentName={agentName}
                                    avatarId={avatarPrefs.avatarId}
                                    customAvatarUrl={avatarPrefs.customAvatarUrl}
                                    skipInitialAnim={msg.id.startsWith('hist-')}
                                    onOpenVideoStudio={(pid) => {
                                        setModulesInitialView('aegis.domain.video');
                                        setTargetVideoProjectId(pid);
                                        setShowModulesHub(true);
                                    }}
                                />
                            ))
                        )}
                    </AnimatePresence>
                    <div className="h-12" />
                </main>

                {/* Agent Activity — CORE-202 */}
                <Suspense fallback={null}>
                    <AgentActivityPanel />
                </Suspense>

                {/* CORE-248/299: banner de estado enriquecido — aparece tras 5s o de inmediato con progreso en vivo */}
                {status === 'thinking' && (showProcessingBanner || liveToolProgress) && (
                    <motion.div
                        initial={{ opacity: 0, y: 4 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        className="shrink-0 flex flex-col gap-1.5 px-6 py-3 bg-white/5 border-t border-white/5 text-white/70 text-xs font-mono"
                    >
                        <div className="flex items-center gap-3">
                            <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan shrink-0" />
                            <span className="truncate">
                                {liveToolProgress?.message
                                    ? liveToolProgress.message
                                    : lastRoutingInfo
                                        ? `${lastRoutingInfo.provider} / ${lastRoutingInfo.model_id}`
                                        : 'Procesando...'}
                            </span>
                            {typeof liveToolProgress?.progress === 'number' ? (
                                <span className="text-aegis-cyan ml-auto shrink-0">
                                    {Math.round(Math.min(1, Math.max(0, liveToolProgress.progress)) * 100)}%
                                </span>
                            ) : lastRoutingInfo?.latency_ms != null ? (
                                <span className="text-white/20 ml-auto shrink-0">{lastRoutingInfo.latency_ms}ms</span>
                            ) : null}
                        </div>
                        {typeof liveToolProgress?.progress === 'number' && (
                            <div className="w-full h-1 rounded-full bg-white/10 overflow-hidden">
                                <div
                                    className="h-full bg-aegis-cyan transition-all duration-300"
                                    style={{ width: `${Math.round(Math.min(1, Math.max(0, liveToolProgress.progress)) * 100)}%` }}
                                />
                            </div>
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
                    {pendingRetryPrompt && status !== 'thinking' && (
                        <motion.div
                            initial={{ opacity: 0, y: 8 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="max-w-4xl mx-auto mb-2.5 px-3.5 py-2.5 bg-amber-500/10 border border-amber-400/25 rounded-xl flex flex-wrap items-center justify-between gap-2 text-xs font-mono"
                        >
                            <div className="flex items-center gap-2 min-w-0 flex-1 text-amber-200/90">
                                <RefreshCw className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                                <span className="truncate">
                                    Mensaje pendiente: &ldquo;{pendingRetryPrompt.displayPrompt}&rdquo;
                                    {pendingRetryPrompt.retryAfterSecs ? ` · reintento auto (${pendingRetryPrompt.retryAfterSecs}s)` : ''}
                                </span>
                            </div>
                            <div className="flex items-center gap-1.5 shrink-0">
                                <button
                                    type="button"
                                    onClick={() => retryPendingMessage(selectedModelId)}
                                    className="px-2.5 py-1 rounded-lg bg-aegis-cyan/20 hover:bg-aegis-cyan/30 text-aegis-cyan border border-aegis-cyan/30 transition-colors"
                                >
                                    Reintentar ahora
                                </button>
                                {selectedModelId !== null && (
                                    <button
                                        type="button"
                                        onClick={() => {
                                            setSelectedModelId(null);
                                            retryPendingMessage(null);
                                        }}
                                        className="px-2.5 py-1 rounded-lg bg-white/5 hover:bg-white/10 text-white/80 border border-white/10 transition-colors"
                                    >
                                        Usar Auto
                                    </button>
                                )}
                                <button
                                    type="button"
                                    onClick={dismissPendingRetry}
                                    className="p-1 rounded-lg text-white/40 hover:text-white hover:bg-white/10 transition-colors"
                                    title="Descartar"
                                >
                                    <X className="w-3.5 h-3.5" />
                                </button>
                            </div>
                        </motion.div>
                    )}
                    <div className={cn("mx-auto relative transition-all duration-200", isLongInput ? "max-w-5xl" : "max-w-4xl")}>
                        <div className={cn(
                            "glass rounded-xl border border-white/10 flex p-2 gap-2 focus-within:border-aegis-cyan/30 transition-all shadow-2xl",
                            isLongInput ? "flex-col items-stretch" : "flex-col sm:flex-row items-stretch sm:items-end"
                        )}>
                            <textarea
                                ref={textareaRef}
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={handleKeyPress}
                                placeholder="Escribí o hablá con tu asistente..."
                                className="w-full bg-transparent border-none focus:ring-0 focus:outline-none text-sm py-2 px-3 resize-none max-h-[280px] min-h-[44px] overflow-y-auto leading-relaxed font-mono placeholder:text-white/20"
                                rows={1}
                            />
                            <div className={cn(
                                "flex flex-wrap items-center justify-end gap-1.5 px-2 pb-1",
                                isLongInput
                                    ? "border-t border-white/5 pt-1.5 sm:gap-2"
                                    : "sm:pb-0 sm:px-0 sm:flex-nowrap sm:gap-2"
                            )}>
                                <input type="file" className="hidden" ref={fileInputRef} onChange={(e) => { const f = e.target.files?.[0]; if (f) handleFileUpload(f); }} />
                                <button onClick={() => fileInputRef.current?.click()} disabled={isUploading} className={cn("p-2 rounded-lg transition-all", isUploading ? "text-aegis-cyan animate-pulse" : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10")} title="Adjuntar archivo">
                                    {isUploading ? <Loader2 className="w-5 h-5 animate-spin" /> : <Paperclip className="w-5 h-5" />}
                                </button>
                                <button
                                    onClick={handleVoiceToggle}
                                    className={cn(
                                        "p-2 rounded-lg transition-all",
                                        voiceEnabled
                                            ? "bg-aegis-cyan/20 text-aegis-cyan hover:bg-aegis-cyan/30 shadow-[0_0_10px_rgba(0,242,254,0.2)]"
                                            : "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                                    )}
                                    title={voiceEnabled ? "Voz del asistente activada (click para silenciar)" : "Activar voz natural del asistente"}
                                >
                                    {voiceEnabled ? <Volume2 className="w-5 h-5" /> : <VolumeX className="w-5 h-5" />}
                                </button>
                                <button
                                    onClick={handleToggleMic}
                                    className={cn(
                                        "p-2 rounded-lg transition-all",
                                        status === 'listening'    && "bg-green-500 text-white animate-pulse shadow-[0_0_15px_rgba(34,197,94,0.5)]",
                                        status === 'transcribing' && "bg-yellow-500 text-black",
                                        isRecording && status !== 'listening' && status !== 'transcribing' && "bg-red-500/20 text-red-400",
                                        !isRecording && status !== 'listening' && status !== 'transcribing' && "bg-white/5 text-white/40 hover:text-white hover:bg-white/10"
                                    )}
                                    title={isRecording ? "Detener micrófono" : "Hablar por micrófono"}
                                >
                                    {status === 'transcribing'
                                        ? <Loader2 className="w-5 h-5 animate-spin" />
                                        : isRecording
                                            ? <MicOff className="w-5 h-5" />
                                            : <Mic className="w-5 h-5" />
                                    }
                                </button>
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
                                                    onClick={() => handleSelectModel(null)}
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
                                                                        onClick={() => handleSelectModel(m.model_id)}
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
                    <Suspense fallback={null}>
                        <SettingsPanel
                            tenantId={tenantId!}
                            sessionKey={sessionKey!}
                            onClose={() => setShowSettings(false)}
                        />
                    </Suspense>
                )}
            </AnimatePresence>

            {/* Personal Assistant Drawer */}
            {showPersonalDrawer && (
                <Suspense fallback={null}>
                    <PersonalAssistantDrawer
                        isOpen={showPersonalDrawer}
                        onClose={() => setShowPersonalDrawer(false)}
                    />
                </Suspense>
            )}

            {/* Microkernel Modules & Mini-Apps Hub */}
            {showModulesHub && (
                <Suspense fallback={null}>
                    <ModulesAppHub
                        isOpen={showModulesHub}
                        initialView={modulesInitialView}
                        initialVideoProjectId={targetVideoProjectId}
                        onConsumeInitialVideoProject={() => setTargetVideoProjectId(null)}
                        onClose={() => {
                            setShowModulesHub(false);
                            setModulesInitialView('store');
                        }}
                    />
                </Suspense>
            )}

            <Suspense fallback={null}>
                <MusicPlayer />
            </Suspense>
            <AnimatePresence>
                {showQR && (
                    <Suspense fallback={null}>
                        <ConnectionQR onClose={() => setShowQR(false)} />
                    </Suspense>
                )}
            </AnimatePresence>
        </div>
    );
};

export default ChatTerminal;
