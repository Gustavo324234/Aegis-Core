import React, { useState, useEffect, useCallback, useRef, Suspense, lazy } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    X,
    Puzzle,
    Clapperboard,
    Film,
    Code2,
    Store,
    Plus,
    Play,
    Pause,
    RotateCcw,
    Video,
    AlertTriangle,
    ArrowRight,
    Loader2,
    Power,
} from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore } from '../store/useAegisStore';
import {
    BizOverview,
    CreatorOverview,
    CreatorPostForm,
    CreatorScript,
    DevOverview,
    ModuleSummary,
    ModulesAppHubProps,
    moduleDescription,
} from './modules/types';
import { CreatorStudioTab } from './modules/CreatorStudioTab';
import { DevStudioTab } from './modules/DevStudioTab';
import { BusinessPosTab } from './modules/BusinessPosTab';

const VideoStudioPanel = lazy(() =>
    import('./VideoStudioPanel').then((m) => ({ default: m.VideoStudioPanel })),
);

export const ModulesAppHub: React.FC<ModulesAppHubProps> = ({
    isOpen,
    onClose,
    initialView = 'store',
    initialVideoProjectId = null,
    onConsumeInitialVideoProject,
}) => {
    const { tenantId, sessionKey, status, sendMessage, setCurrentView } = useAegisStore(
        useShallow((s) => ({
            tenantId: s.tenantId,
            sessionKey: s.sessionKey,
            status: s.status,
            sendMessage: s.sendMessage,
            setCurrentView: s.setCurrentView,
        })),
    );

    const [modules, setModules] = useState<ModuleSummary[]>([]);
    const [activeApp, setActiveApp] = useState<string>(initialView);

    useEffect(() => {
        if (isOpen && initialView) {
            setActiveApp(initialView);
        }
    }, [isOpen, initialView]);
    const [loadingModules, setLoadingModules] = useState(false);
    const [togglingId, setTogglingId] = useState<string | null>(null);
    const [actionError, setActionError] = useState<string | null>(null);
    const [copiedKey, setCopiedKey] = useState<string | null>(null);
    const [pendingVideoScriptId, setPendingVideoScriptId] = useState<string | null>(null);

    // Sub-tabs inside each Mini-App
    const [creatorTab, setCreatorTab] = useState<'ideas' | 'scripts' | 'calendar'>('ideas');

    // Module data states
    const [creatorData, setCreatorData] = useState<CreatorOverview>({
        ideas: [],
        scripts: [],
        calendar: [],
        stats: { total_ideas: 0, total_scripts: 0, ready_to_film: 0, scheduled_posts: 0 },
    });
    const [devData, setDevData] = useState<DevOverview>({
        projects: [],
        tasks: [],
        snippets: [],
        stats: { total_projects: 0, open_tasks: 0, in_progress_tasks: 0, bugs_count: 0, total_snippets: 0 },
    });
    const [bizData, setBizData] = useState<BizOverview>({
        products: [],
        sales: [],
        stats: { total_products: 0, low_stock_count: 0, inventory_value: 0, total_sales_count: 0, total_revenue: 0 },
    });

    const [teleprompterScript, setTeleprompterScript] = useState<CreatorScript | null>(null);
    const [tpPlaying, setTpPlaying] = useState(false);
    const [tpSpeed, setTpSpeed] = useState(2);
    const [tpFontSize, setTpFontSize] = useState(28);
    const tpScrollRef = useRef<HTMLDivElement>(null);

    const [postForm, setPostForm] = useState<CreatorPostForm>({
        title: '',
        platform: 'tiktok_reels',
        publish_at: '',
        caption: '',
        hashtags: '',
    });
    const [busyTool, setBusyTool] = useState(false);

    const callTool = useCallback(
        async (moduleId: string, toolName: string, args: Record<string, unknown> = {}) => {
            if (!tenantId || !sessionKey) return null;
            const res = await fetch(`/api/router/modules/${encodeURIComponent(moduleId)}/execute`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ tool_name: toolName, arguments: args }),
            });
            const data = await res.json().catch(() => ({}));
            if (!res.ok || data.success === false) {
                throw new Error(data.detail || data.error || `HTTP ${res.status}`);
            }
            return data.result;
        },
        [tenantId, sessionKey],
    );

    const loadAppOverview = useCallback(
        async (moduleId: string) => {
            if (!tenantId || !sessionKey) return;
            try {
                if (moduleId === 'aegis.domain.creator') {
                    const r = await callTool(moduleId, 'creator_get_overview', {});
                    if (r) setCreatorData(r as CreatorOverview);
                } else if (moduleId === 'aegis.domain.dev') {
                    const r = await callTool(moduleId, 'dev_get_overview', {});
                    if (r) setDevData(r as DevOverview);
                } else if (moduleId === 'aegis.domain.business') {
                    const r = await callTool(moduleId, 'biz_get_overview', {});
                    if (r) setBizData(r as BizOverview);
                }
            } catch (e) {
                console.error('Error loading module overview:', e);
            }
        },
        [tenantId, sessionKey, callTool],
    );

    const fetchModulesList = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoadingModules(true);
        try {
            const res = await fetch('/api/router/modules', {
                headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const data = (await res.json()) as { modules?: ModuleSummary[] };
            const list = data.modules || [];
            setModules(list);
            return list;
        } catch (e) {
            setActionError(e instanceof Error ? e.message : 'No se pudieron cargar los módulos');
            return [];
        } finally {
            setLoadingModules(false);
        }
    }, [tenantId, sessionKey]);

    useEffect(() => {
        if (!isOpen) return;
        setActionError(null);
        void (async () => {
            const list = await fetchModulesList();
            if (activeApp !== 'store') {
                const mod = list?.find((m) => m.module_id === activeApp);
                if (mod?.active) {
                    await loadAppOverview(activeApp);
                }
            }
        })();
    }, [isOpen, activeApp, fetchModulesList, loadAppOverview]);

    // Sync automatically after AI completes a turn or broadcasts modules_updated
    const prevStatusRef = useRef(status);
    useEffect(() => {
        if (isOpen && prevStatusRef.current === 'thinking' && status === 'idle') {
            void fetchModulesList().then((list) => {
                if (activeApp !== 'store' && list?.some((m) => m.module_id === activeApp && m.active)) {
                    void loadAppOverview(activeApp);
                }
            });
        }
        prevStatusRef.current = status;
    }, [isOpen, status, activeApp, fetchModulesList, loadAppOverview]);

    useEffect(() => {
        if (!isOpen) return;
        const onUpdated = () => {
            void fetchModulesList().then((list) => {
                if (activeApp !== 'store' && list?.some((m) => m.module_id === activeApp && m.active)) {
                    void loadAppOverview(activeApp);
                }
            });
        };
        window.addEventListener('aegis-modules-updated', onUpdated);
        return () => window.removeEventListener('aegis-modules-updated', onUpdated);
    }, [isOpen, activeApp, fetchModulesList, loadAppOverview]);

    // Teleprompter smooth scroll loop
    useEffect(() => {
        if (!teleprompterScript || !tpPlaying) return;
        let rafId = 0;
        let lastTs = performance.now();
        let carry = 0;
        const tick = (now: number) => {
            const dt = (now - lastTs) / 1000;
            lastTs = now;
            const el = tpScrollRef.current;
            if (el) {
                const pxPerSec = 18 * tpSpeed;
                carry += pxPerSec * dt;
                if (carry >= 1) {
                    const step = Math.floor(carry);
                    el.scrollTop += step;
                    carry -= step;
                }
                if (el.scrollTop + el.clientHeight >= el.scrollHeight - 4) {
                    setTpPlaying(false);
                    return;
                }
            }
            rafId = requestAnimationFrame(tick);
        };
        rafId = requestAnimationFrame(tick);
        return () => cancelAnimationFrame(rafId);
    }, [teleprompterScript, tpPlaying, tpSpeed]);

    const handleToggleModule = async (mod: ModuleSummary, openAfterEnable = false) => {
        if (!tenantId || !sessionKey) return;
        setTogglingId(mod.module_id);
        setActionError(null);
        const nextActive = !mod.active;
        try {
            const res = await fetch(`/api/router/modules/${encodeURIComponent(mod.module_id)}/enable`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ enabled: nextActive }),
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            setModules((prev) =>
                prev.map((m) => (m.module_id === mod.module_id ? { ...m, active: nextActive } : m)),
            );
            if (nextActive && openAfterEnable) {
                setActiveApp(mod.module_id);
                await loadAppOverview(mod.module_id);
            } else if (!nextActive && activeApp === mod.module_id) {
                setActiveApp('store');
            }
        } catch (e) {
            setActionError(e instanceof Error ? e.message : 'Error al cambiar el estado del módulo');
        } finally {
            setTogglingId(null);
        }
    };

    const runActionAndRefresh = async (moduleId: string, toolName: string, args: Record<string, unknown>) => {
        setBusyTool(true);
        setActionError(null);
        try {
            await callTool(moduleId, toolName, args);
            await loadAppOverview(moduleId);
        } catch (e) {
            setActionError(e instanceof Error ? e.message : 'Error ejecutando la acción');
        } finally {
            setBusyTool(false);
        }
    };

    const handleCopy = (key: string, text: string) => {
        navigator.clipboard?.writeText(text).catch(() => {});
        setCopiedKey(key);
        setTimeout(() => setCopiedKey((prev) => (prev === key ? null : prev)), 1500);
    };

    const askAegisWithPrompt = (prompt: string) => {
        onClose();
        sendMessage(prompt);
    };

    if (!isOpen) return null;

    const activeModules = modules.filter((m) => m.active);

    return (
        <AnimatePresence>
            <div className="fixed inset-0 z-[100] flex items-center justify-center p-2 sm:p-5 bg-black/85 backdrop-blur-md">
                <motion.div
                    initial={{ opacity: 0, scale: 0.97, y: 12 }}
                    animate={{ opacity: 1, scale: 1, y: 0 }}
                    exit={{ opacity: 0, scale: 0.97, y: 12 }}
                    className="w-full max-w-6xl h-[92vh] bg-[#0a0d12] border border-white/10 rounded-2xl shadow-2xl flex flex-col overflow-hidden text-white"
                >
                    {/* Header */}
                    <div className="shrink-0 px-4 sm:px-6 py-3.5 border-b border-white/10 bg-black/50 flex flex-wrap items-center justify-between gap-3">
                        <div className="flex items-center gap-3">
                            <div className="p-2 rounded-xl bg-aegis-cyan/15 border border-aegis-cyan/25 text-aegis-cyan">
                                <Puzzle className="w-5 h-5" />
                            </div>
                            <div>
                                <h2 className="text-sm sm:text-base font-bold tracking-wide">
                                    Módulos & Mini-Apps de Aegis
                                </h2>
                                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest">
                                    Instalación bajo demanda · Base cifrada por usuario · Integrado con Chat/Voz
                                </p>
                            </div>
                        </div>

                        <div className="flex items-center gap-2 overflow-x-auto">
                            <button
                                onClick={() => setActiveApp('store')}
                                className={`px-3 py-1.5 rounded-lg text-xs font-mono transition-all flex items-center gap-1.5 border ${
                                    activeApp === 'store'
                                        ? 'bg-aegis-cyan/20 text-aegis-cyan border-aegis-cyan/40'
                                        : 'bg-white/5 text-white/60 border-white/10 hover:text-white'
                                }`}
                            >
                                <Puzzle className="w-3.5 h-3.5" />
                                Catálogo ({modules.length})
                            </button>

                            {activeModules.map((mod) => {
                                const isSel = activeApp === mod.module_id;
                                const Icon =
                                    mod.module_id === 'aegis.domain.creator'
                                        ? Clapperboard
                                        : mod.module_id === 'aegis.domain.video'
                                          ? Film
                                          : mod.module_id === 'aegis.domain.dev'
                                            ? Code2
                                            : Store;
                                return (
                                    <button
                                        key={mod.module_id}
                                        onClick={() => setActiveApp(mod.module_id)}
                                        className={`px-3 py-1.5 rounded-lg text-xs font-mono transition-all flex items-center gap-1.5 border whitespace-nowrap ${
                                            isSel
                                                ? 'bg-aegis-cyan text-black font-bold border-aegis-cyan'
                                                : 'bg-white/5 text-white/80 border-white/10 hover:border-aegis-cyan/40'
                                        }`}
                                    >
                                        <Icon className="w-3.5 h-3.5" />
                                        {mod.display_name.replace('Aegis ', '')}
                                    </button>
                                );
                            })}

                            <button
                                onClick={onClose}
                                className="p-2 rounded-lg bg-white/5 hover:bg-white/10 text-white/50 hover:text-white transition-colors ml-1"
                                title="Cerrar"
                            >
                                <X className="w-4 h-4" />
                            </button>
                        </div>
                    </div>

                    {actionError && (
                        <div className="shrink-0 px-6 py-2.5 bg-red-500/15 border-b border-red-500/30 flex items-center justify-between text-xs font-mono text-red-300">
                            <span className="flex items-center gap-2">
                                <AlertTriangle className="w-4 h-4 text-red-400 shrink-0" />
                                {actionError}
                            </span>
                            <button onClick={() => setActionError(null)} className="text-red-300/60 hover:text-white">
                                <X className="w-3.5 h-3.5" />
                            </button>
                        </div>
                    )}

                    {/* Body */}
                    <div className="flex-1 overflow-y-auto p-4 sm:p-6">
                        {activeApp === 'store' && (
                            <div className="space-y-6">
                                <div className="p-4 rounded-xl bg-white/[0.03] border border-white/10 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                                    <div>
                                        <h3 className="text-sm font-semibold text-white">
                                            Instalá solo los módulos que necesitás
                                        </h3>
                                        <p className="text-xs text-white/55 mt-0.5">
                                            Cada módulo agrega una aplicación dedicada arriba y conecta sus herramientas directo al chat y a la voz de Aegis.
                                        </p>
                                    </div>
                                    <span className="text-[11px] font-mono text-aegis-cyan bg-aegis-cyan/10 border border-aegis-cyan/20 px-3 py-1.5 rounded-lg shrink-0">
                                        {activeModules.length} de {modules.length} activos
                                    </span>
                                </div>

                                {loadingModules && modules.length === 0 ? (
                                    <div className="py-16 flex items-center justify-center text-white/40 gap-2 text-xs font-mono">
                                        <Loader2 className="w-4 h-4 animate-spin text-aegis-cyan" />
                                        Escaneando módulos del microkernel...
                                    </div>
                                ) : (
                                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                                        {modules.map((mod) => {
                                            const info = moduleDescription(mod.module_id);
                                            const isCreator = mod.module_id === 'aegis.domain.creator';
                                            const isVideo = mod.module_id === 'aegis.domain.video';
                                            const isDev = mod.module_id === 'aegis.domain.dev';
                                            const Icon = isCreator
                                                ? Clapperboard
                                                : isVideo
                                                  ? Film
                                                  : isDev
                                                    ? Code2
                                                    : Store;
                                            const badgeColor = isCreator
                                                ? 'text-pink-300 bg-pink-500/15 border-pink-400/30'
                                                : isVideo
                                                  ? 'text-purple-300 bg-purple-500/15 border-purple-400/30'
                                                  : isDev
                                                    ? 'text-aegis-cyan bg-aegis-cyan/15 border-aegis-cyan/30'
                                                    : 'text-emerald-300 bg-emerald-500/15 border-emerald-400/30';

                                            return (
                                                <div
                                                    key={mod.module_id}
                                                    className={`p-5 rounded-2xl border flex flex-col justify-between gap-4 transition-all ${
                                                        mod.active
                                                            ? 'bg-white/[0.04] border-aegis-cyan/35 shadow-[0_0_24px_rgba(0,242,254,0.06)]'
                                                            : 'bg-white/[0.02] border-white/10 hover:border-white/20'
                                                    }`}
                                                >
                                                    <div className="space-y-3">
                                                        <div className="flex items-start justify-between gap-2">
                                                            <div className={`p-2.5 rounded-xl border ${badgeColor}`}>
                                                                <Icon className="w-5 h-5" />
                                                            </div>
                                                            <span
                                                                className={`text-[10px] font-mono px-2.5 py-1 rounded-full border ${
                                                                    mod.active
                                                                        ? 'bg-green-500/15 text-green-300 border-green-500/30'
                                                                        : 'bg-white/5 text-white/40 border-white/10'
                                                                }`}
                                                            >
                                                                {mod.active ? 'ACTIVO' : 'DISPONIBLE'}
                                                            </span>
                                                        </div>

                                                        <div>
                                                            <h4 className="text-base font-bold text-white">
                                                                {mod.display_name}
                                                            </h4>
                                                            <p className="text-[10px] font-mono text-white/35">
                                                                {mod.module_id} · v{mod.version} · {mod.exposed_tools.length} herramientas IA
                                                            </p>
                                                        </div>

                                                        <p className="text-xs text-white/70 leading-relaxed">
                                                            {info.tagline}
                                                        </p>

                                                        <ul className="space-y-1.5 pt-1">
                                                            {info.highlights.map((h) => (
                                                                <li key={h} className="text-[11px] text-white/60 flex items-start gap-2">
                                                                    <span className="text-aegis-cyan mt-0.5">•</span>
                                                                    <span>{h}</span>
                                                                </li>
                                                            ))}
                                                        </ul>
                                                    </div>

                                                    <div className="flex items-center gap-2 pt-3 border-t border-white/10">
                                                        {mod.active ? (
                                                            <>
                                                                <button
                                                                    onClick={() => setActiveApp(mod.module_id)}
                                                                    className="flex-1 py-2 px-3 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold hover:opacity-90 transition-opacity flex items-center justify-center gap-1.5"
                                                                >
                                                                    Abrir App
                                                                    <ArrowRight className="w-3.5 h-3.5" />
                                                                </button>
                                                                <button
                                                                    disabled={togglingId === mod.module_id}
                                                                    onClick={() => void handleToggleModule(mod, false)}
                                                                    className="p-2 rounded-xl bg-white/5 hover:bg-red-500/20 text-white/50 hover:text-red-300 border border-white/10 transition-colors"
                                                                    title="Desactivar módulo"
                                                                >
                                                                    {togglingId === mod.module_id ? (
                                                                        <Loader2 className="w-4 h-4 animate-spin" />
                                                                    ) : (
                                                                        <Power className="w-4 h-4" />
                                                                    )}
                                                                </button>
                                                            </>
                                                        ) : (
                                                            <button
                                                                disabled={togglingId === mod.module_id}
                                                                onClick={() => void handleToggleModule(mod, true)}
                                                                className="w-full py-2 px-3 rounded-xl bg-white/10 hover:bg-aegis-cyan/20 text-white hover:text-aegis-cyan border border-white/15 hover:border-aegis-cyan/40 font-mono text-xs font-semibold transition-all flex items-center justify-center gap-2"
                                                            >
                                                                {togglingId === mod.module_id ? (
                                                                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                                                ) : (
                                                                    <Plus className="w-3.5 h-3.5" />
                                                                )}
                                                                Instalar y Abrir
                                                            </button>
                                                        )}
                                                    </div>
                                                </div>
                                            );
                                        })}
                                    </div>
                                )}
                            </div>
                        )}

                        {/* ═══════════════ 1. AEGIS CREATOR STUDIO ═══════════════ */}
                        {activeApp === 'aegis.domain.creator' && (
                            <CreatorStudioTab
                                creatorData={creatorData}
                                creatorTab={creatorTab}
                                setCreatorTab={setCreatorTab}
                                postForm={postForm}
                                setPostForm={setPostForm}
                                busyTool={busyTool}
                                copiedKey={copiedKey}
                                handleCopy={handleCopy}
                                runActionAndRefresh={runActionAndRefresh}
                                askAegisWithPrompt={askAegisWithPrompt}
                                onOpenTeleprompter={(script) => {
                                    setTpPlaying(false);
                                    setTeleprompterScript(script);
                                }}
                                onOpenVideoFromScript={async (scriptId) => {
                                    const vidMod = modules.find((m) => m.module_id === 'aegis.domain.video');
                                    if (vidMod && !vidMod.active) {
                                        await handleToggleModule(vidMod, true);
                                    }
                                    setPendingVideoScriptId(scriptId);
                                    setActiveApp('aegis.domain.video');
                                }}
                            />
                        )}

                        {/* ═══════════════ 2. AEGIS DEV STUDIO ═══════════════ */}
                        {activeApp === 'aegis.domain.dev' && (
                            <DevStudioTab
                                devData={devData}
                                busyTool={busyTool}
                                copiedKey={copiedKey}
                                handleCopy={handleCopy}
                                runActionAndRefresh={runActionAndRefresh}
                                askAegisWithPrompt={askAegisWithPrompt}
                                onOpenWorkspace={() => {
                                    onClose();
                                    setCurrentView('dashboard');
                                }}
                            />
                        )}

                        {/* ═══════════════ 3. AEGIS BUSINESS & POS ═══════════════ */}
                        {activeApp === 'aegis.domain.business' && (
                            <BusinessPosTab
                                bizData={bizData}
                                busyTool={busyTool}
                                runActionAndRefresh={runActionAndRefresh}
                            />
                        )}

                        {/* ═══════════════ 1B. AEGIS VIDEO STUDIO ═══════════════ */}
                        {activeApp === 'aegis.domain.video' && (
                            <Suspense
                                fallback={
                                    <div className="flex items-center justify-center py-16 text-white/40 gap-2 text-xs font-mono">
                                        <Loader2 className="w-4 h-4 animate-spin text-aegis-cyan" />
                                        Cargando Video Studio...
                                    </div>
                                }
                            >
                                <VideoStudioPanel
                                    tenantId={tenantId}
                                    sessionKey={sessionKey}
                                    pendingCreatorScriptId={pendingVideoScriptId}
                                    onConsumePendingScript={() => setPendingVideoScriptId(null)}
                                    targetProjectId={initialVideoProjectId}
                                    onConsumeTargetProject={onConsumeInitialVideoProject}
                                    onScheduleResult={(projectTitle, platform) => {
                                        setPostForm((prev) => ({
                                            ...prev,
                                            title: projectTitle,
                                            platform: platform || 'tiktok_reels',
                                        }));
                                        setActiveApp('aegis.domain.creator');
                                        setCreatorTab('calendar');
                                    }}
                                    onAskAegis={(prompt) => askAegisWithPrompt(prompt)}
                                />
                            </Suspense>
                        )}
                    </div>
                </motion.div>

                {/* Teleprompter Fullscreen Overlay */}
                {teleprompterScript && (
                    <div className="fixed inset-0 z-[120] bg-black flex flex-col">
                        <div className="shrink-0 px-6 py-3 border-b border-white/10 bg-[#0d1117] flex flex-wrap items-center justify-between gap-3">
                            <div className="flex items-center gap-3">
                                <Video className="w-4 h-4 text-aegis-cyan" />
                                <span className="text-sm font-bold text-white">{teleprompterScript.title}</span>
                            </div>
                            <div className="flex flex-wrap items-center gap-3 text-xs font-mono">
                                <button
                                    onClick={() => setTpPlaying((v) => !v)}
                                    className="px-3.5 py-1.5 rounded-lg bg-aegis-cyan text-black font-bold flex items-center gap-1.5"
                                >
                                    {tpPlaying ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
                                    {tpPlaying ? 'Pausar' : 'Iniciar Scroll'}
                                </button>
                                <button
                                    onClick={() => {
                                        setTpPlaying(false);
                                        if (tpScrollRef.current) tpScrollRef.current.scrollTop = 0;
                                    }}
                                    className="px-2.5 py-1.5 rounded-lg bg-white/10 text-white/80 flex items-center gap-1"
                                >
                                    <RotateCcw className="w-3.5 h-3.5" />
                                    Inicio
                                </button>
                                <label className="flex items-center gap-1.5 text-white/60">
                                    Velocidad: {tpSpeed}x
                                    <input
                                        type="range"
                                        min={1}
                                        max={5}
                                        step={0.5}
                                        value={tpSpeed}
                                        onChange={(e) => setTpSpeed(Number(e.target.value))}
                                        className="w-20"
                                    />
                                </label>
                                <label className="flex items-center gap-1.5 text-white/60">
                                    Texto: {tpFontSize}px
                                    <input
                                        type="range"
                                        min={20}
                                        max={48}
                                        step={2}
                                        value={tpFontSize}
                                        onChange={(e) => setTpFontSize(Number(e.target.value))}
                                        className="w-20"
                                    />
                                </label>
                                <button
                                    onClick={() => {
                                        setTpPlaying(false);
                                        setTeleprompterScript(null);
                                    }}
                                    className="p-2 rounded-lg bg-white/10 hover:bg-red-500/20 text-white/70 hover:text-white"
                                >
                                    <X className="w-4 h-4" />
                                </button>
                            </div>
                        </div>
                        <div
                            ref={tpScrollRef}
                            className="flex-1 overflow-y-auto px-6 sm:px-16 py-24 max-w-4xl mx-auto w-full space-y-8"
                            style={{ fontSize: `${tpFontSize}px`, lineHeight: 1.45 }}
                        >
                            {teleprompterScript.hook_0_3s && (
                                <p className="text-amber-300 font-bold">{teleprompterScript.hook_0_3s}</p>
                            )}
                            {teleprompterScript.body_script && (
                                <p className="text-white whitespace-pre-wrap">{teleprompterScript.body_script}</p>
                            )}
                            {teleprompterScript.cta && (
                                <p className="text-aegis-cyan font-bold">{teleprompterScript.cta}</p>
                            )}
                            <div className="h-[55vh]" />
                        </div>
                    </div>
                )}
            </div>
        </AnimatePresence>
    );
};

export default ModulesAppHub;
