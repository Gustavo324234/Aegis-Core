import React, { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    X,
    Puzzle,
    Clapperboard,
    Film,
    Code2,
    Store,
    Sparkles,
    Plus,
    Trash2,
    Play,
    Pause,
    RotateCcw,
    Copy,
    Check,
    Calendar,
    FileText,
    Lightbulb,
    Video,
    Bug,
    FolderGit2,
    Terminal,
    ShoppingBag,
    Package,
    AlertTriangle,
    ArrowRight,
    ArrowLeft,
    Loader2,
    Bot,
    Power,
    Search,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { VideoStudioPanel } from './VideoStudioPanel';

interface ExposedToolSchema {
    name: string;
    description: string;
}

interface ModuleSummary {
    module_id: string;
    display_name: string;
    version: string;
    active: boolean;
    exposed_tools: ExposedToolSchema[];
}

interface CreatorIdea {
    id: string;
    title: string;
    platform: string;
    hook: string;
    pillar: string;
    notes: string;
    status: string;
    priority: string;
    updated_at?: string;
}

interface CreatorScript {
    id: string;
    idea_id?: string;
    title: string;
    platform: string;
    hook_0_3s: string;
    body_script: string;
    b_roll_notes: string;
    cta: string;
    caption: string;
    hashtags: string;
    estimated_duration_secs?: number;
    status: string;
}

interface CreatorPost {
    id: string;
    title: string;
    platform: string;
    publish_at: string;
    caption: string;
    hashtags: string;
    status: string;
}

interface CreatorOverview {
    ideas: CreatorIdea[];
    scripts: CreatorScript[];
    calendar: CreatorPost[];
    stats: {
        total_ideas: number;
        total_scripts: number;
        ready_to_film: number;
        scheduled_posts: number;
    };
}

interface DevProject {
    id: string;
    name: string;
    repo_path: string;
    stack: string;
    description: string;
    default_branch: string;
    build_cmd: string;
    test_cmd: string;
    status: string;
}

interface DevTask {
    id: string;
    project_id: string;
    title: string;
    kind: string;
    priority: string;
    status: string;
    file_refs: string;
    notes: string;
}

interface DevSnippet {
    id: string;
    title: string;
    language: string;
    code: string;
    tags: string;
    description: string;
}

interface DevOverview {
    projects: DevProject[];
    tasks: DevTask[];
    snippets: DevSnippet[];
    stats: {
        total_projects: number;
        open_tasks: number;
        in_progress_tasks: number;
        bugs_count: number;
        total_snippets: number;
    };
}

interface BizProduct {
    id: string;
    barcode: string;
    name: string;
    category: string;
    price: number;
    cost: number;
    stock: number;
    min_stock: number;
}

interface BizSale {
    id: string;
    product_name: string;
    quantity: number;
    unit_price: number;
    total: number;
    customer: string;
    payment_method: string;
    created_at: string;
}

interface BizOverview {
    products: BizProduct[];
    sales: BizSale[];
    stats: {
        total_products: number;
        low_stock_count: number;
        inventory_value: number;
        total_sales_count: number;
        total_revenue: number;
    };
}

interface ModulesAppHubProps {
    isOpen: boolean;
    onClose: () => void;
    initialView?: string;
    initialVideoProjectId?: string | null;
    onConsumeInitialVideoProject?: () => void;
}

const PLATFORM_LABELS: Record<string, string> = {
    tiktok_reels: 'TikTok / Reels',
    youtube: 'YouTube',
    shorts: 'YT Shorts',
    instagram: 'Instagram',
    x_twitter: 'X / Twitter',
    linkedin: 'LinkedIn',
};

const IDEA_STAGES = ['idea', 'scripting', 'filming', 'editing', 'scheduled', 'published'] as const;
const IDEA_STAGE_LABELS: Record<string, string> = {
    idea: 'Idea',
    scripting: 'Guion',
    filming: 'Grabando',
    editing: 'Editando',
    scheduled: 'Programado',
    published: 'Publicado',
};

const DEV_COLUMNS = ['backlog', 'in_progress', 'review', 'done'] as const;
const DEV_COLUMN_LABELS: Record<string, string> = {
    backlog: 'Backlog',
    in_progress: 'En Curso',
    review: 'Review',
    done: 'Hecho',
};

function moduleDescription(moduleId: string): { tagline: string; highlights: string[] } {
    switch (moduleId) {
        case 'aegis.domain.creator':
            return {
                tagline: 'Estudio completo para creadores de contenido en TikTok, Reels, YouTube, Shorts, Instagram y X.',
                highlights: [
                    'Banco de ideas y hooks virales por plataforma',
                    'Editor de guiones + Teleprompter en vivo integrado',
                    'Calendario editorial y puente directo hacia Video Studio IA',
                ],
            };
        case 'aegis.domain.video':
            return {
                tagline: 'Fábrica de Videos con IA: convierte ideas o guiones de Creator Studio en MP4 listos con subtítulos y voz.',
                highlights: [
                    'Selector universal: Gratis (Edge-TTS + Placas), Veo/Imagen, OpenAI, ElevenLabs o IA Local (ComfyUI)',
                    'Auto-instalación bajo demanda de FFmpeg y motor de voces en entorno aislado',
                    'Storyboard editable escena por escena con exportación real a MP4 + .SRT',
                ],
            };
        case 'aegis.domain.dev':
            return {
                tagline: 'Centro de ingeniería integrado con tus proyectos, Kanban técnico, snippets y agentes autónomos.',
                highlights: [
                    'Kanban de bugs, features, refactors y tareas técnicas',
                    'Registro de repositorios con comandos build/test en 1 clic',
                    'Bóveda de snippets y delegación directa a Agentes / Antigravity',
                ],
            };
        case 'aegis.domain.business':
            return {
                tagline: 'Mini-App de gestión comercial con catálogo, alertas de stock y Punto de Venta (POS) por voz o chat.',
                highlights: [
                    'Control de inventario con SKU/código de barras y alertas',
                    'Punto de Venta (POS) con descuento automático de stock',
                    'Actualizá precios o registrá ventas hablando con Aegis',
                ],
            };
        default:
            return {
                tagline: 'Módulo de dominio extensible del microkernel de Aegis.',
                highlights: ['Herramientas locales cifradas integradas al chat'],
            };
    }
}

export const ModulesAppHub: React.FC<ModulesAppHubProps> = ({
    isOpen,
    onClose,
    initialView = 'store',
    initialVideoProjectId = null,
    onConsumeInitialVideoProject,
}) => {
    const { tenantId, sessionKey, status, sendMessage, setCurrentView } = useAegisStore();

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
    const [devTab, setDevTab] = useState<'kanban' | 'projects' | 'snippets'>('kanban');
    const [bizTab, setBizTab] = useState<'pos' | 'inventory'>('pos');

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

    // Creator Studio form & Teleprompter states
    const [ideaPlatformFilter, setIdeaPlatformFilter] = useState<string>('all');
    const [newIdeaTitle, setNewIdeaTitle] = useState('');
    const [newIdeaPlatform, setNewIdeaPlatform] = useState('tiktok_reels');
    const [newIdeaHook, setNewIdeaHook] = useState('');
    const [newIdeaPillar, setNewIdeaPillar] = useState('Demo / Tutorial');

    const [scriptForm, setScriptForm] = useState({
        id: '',
        title: '',
        platform: 'tiktok_reels',
        hook_0_3s: '',
        body_script: '',
        b_roll_notes: '',
        cta: '',
        caption: '',
        hashtags: '',
    });
    const [teleprompterScript, setTeleprompterScript] = useState<CreatorScript | null>(null);
    const [tpPlaying, setTpPlaying] = useState(false);
    const [tpSpeed, setTpSpeed] = useState(2);
    const [tpFontSize, setTpFontSize] = useState(28);
    const tpScrollRef = useRef<HTMLDivElement>(null);

    const [postForm, setPostForm] = useState({
        title: '',
        platform: 'tiktok_reels',
        publish_at: '',
        caption: '',
        hashtags: '',
    });

    // Dev Studio form states
    const [newTaskTitle, setNewTaskTitle] = useState('');
    const [newTaskKind, setNewTaskKind] = useState('feature');
    const [newTaskPriority, setNewTaskPriority] = useState('medium');
    const [newTaskFiles, setNewTaskFiles] = useState('');

    const [newProjForm, setNewProjForm] = useState({
        name: '',
        stack: 'Rust + React',
        repo_path: '',
        build_cmd: '',
        test_cmd: '',
    });
    const [newSnippetForm, setNewSnippetForm] = useState({
        title: '',
        language: 'bash',
        code: '',
        tags: '',
    });
    const [snippetSearch, setSnippetSearch] = useState('');

    // Business & POS form states
    const [posProductKey, setPosProductKey] = useState('');
    const [posQty, setPosQty] = useState(1);
    const [posUnitPrice, setPosUnitPrice] = useState<string>('');
    const [posCustomer, setPosCustomer] = useState('Consumidor Final');
    const [posPayment, setPosPayment] = useState('efectivo');

    const [newProductForm, setNewProductForm] = useState({
        barcode: '',
        name: '',
        category: 'General',
        price: '',
        cost: '',
        stock: '1',
        min_stock: '3',
    });
    const [productSearch, setProductSearch] = useState('');
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
    const filteredIdeas = creatorData.ideas.filter(
        (i) => ideaPlatformFilter === 'all' || i.platform === ideaPlatformFilter,
    );
    const selectedPosProduct = bizData.products.find((p) => p.id === posProductKey) ?? null;
    const effectivePosPrice =
        posUnitPrice.trim() !== ''
            ? Number(posUnitPrice) || 0
            : selectedPosProduct?.price ?? 0;

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
                            <div className="space-y-5">
                                {/* KPI Strip */}
                                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Banco de Ideas</p>
                                        <p className="text-2xl font-bold text-white mt-1">{creatorData.stats.total_ideas}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Guiones Guardados</p>
                                        <p className="text-2xl font-bold text-pink-300 mt-1">{creatorData.stats.total_scripts}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Listos para Grabar</p>
                                        <p className="text-2xl font-bold text-aegis-cyan mt-1">{creatorData.stats.ready_to_film}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Programados</p>
                                        <p className="text-2xl font-bold text-emerald-300 mt-1">{creatorData.stats.scheduled_posts}</p>
                                    </div>
                                </div>

                                {/* Tabs */}
                                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-3">
                                    <div className="flex items-center gap-2">
                                        {[
                                            { id: 'ideas', label: 'Ideas & Hooks', icon: Lightbulb },
                                            { id: 'scripts', label: 'Guiones & Teleprompter', icon: FileText },
                                            { id: 'calendar', label: 'Calendario Editorial', icon: Calendar },
                                        ].map((tab) => {
                                            const TIcon = tab.icon;
                                            return (
                                                <button
                                                    key={tab.id}
                                                    onClick={() => setCreatorTab(tab.id as 'ideas' | 'scripts' | 'calendar')}
                                                    className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                                                        creatorTab === tab.id
                                                            ? 'bg-pink-500/20 text-pink-200 border-pink-400/40'
                                                            : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                                                    }`}
                                                >
                                                    <TIcon className="w-3.5 h-3.5" />
                                                    {tab.label}
                                                </button>
                                            );
                                        })}
                                    </div>

                                    <button
                                        onClick={() =>
                                            askAegisWithPrompt(
                                                'Actuando con el módulo Creator Studio, proponeme 3 ideas virales con hook de 0 a 3 segundos sobre cómo usar un asistente IA local como Aegis y guardalas en mi Banco de Ideas.',
                                            )
                                        }
                                        className="px-3.5 py-2 rounded-xl text-xs font-mono bg-gradient-to-r from-pink-500/25 to-aegis-cyan/25 border border-pink-400/35 text-white hover:opacity-90 flex items-center gap-2"
                                    >
                                        <Sparkles className="w-3.5 h-3.5 text-pink-300" />
                                        Pedir 3 Ideas Virales a Aegis
                                    </button>
                                </div>

                                {/* Creator Tab 1: Ideas */}
                                {creatorTab === 'ideas' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-3 h-fit">
                                            <h4 className="text-xs font-mono uppercase tracking-wider text-pink-300 font-bold">
                                                + Nueva Idea / Hook Viral
                                            </h4>
                                            <input
                                                value={newIdeaTitle}
                                                onChange={(e) => setNewIdeaTitle(e.target.value)}
                                                placeholder="Título o concepto del video/post..."
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white focus:border-pink-400 outline-none"
                                            />
                                            <select
                                                value={newIdeaPlatform}
                                                onChange={(e) => setNewIdeaPlatform(e.target.value)}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white focus:border-pink-400 outline-none"
                                            >
                                                {Object.entries(PLATFORM_LABELS).map(([k, v]) => (
                                                    <option key={k} value={k}>
                                                        {v}
                                                    </option>
                                                ))}
                                            </select>
                                            <textarea
                                                value={newIdeaHook}
                                                onChange={(e) => setNewIdeaHook(e.target.value)}
                                                placeholder="Hook primeros 3 segundos..."
                                                rows={2}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white focus:border-pink-400 outline-none resize-none"
                                            />
                                            <input
                                                value={newIdeaPillar}
                                                onChange={(e) => setNewIdeaPillar(e.target.value)}
                                                placeholder="Pilar (Ej: Tutorial, Demo, Opinión)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white focus:border-pink-400 outline-none"
                                            />
                                            <button
                                                disabled={busyTool || !newIdeaTitle.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.creator', 'creator_manage_idea', {
                                                        action: 'add',
                                                        title: newIdeaTitle.trim(),
                                                        platform: newIdeaPlatform,
                                                        hook: newIdeaHook.trim(),
                                                        pillar: newIdeaPillar.trim() || 'General',
                                                    });
                                                    setNewIdeaTitle('');
                                                    setNewIdeaHook('');
                                                }}
                                                className="w-full py-2 rounded-xl bg-pink-500/80 hover:bg-pink-500 disabled:opacity-40 text-white text-xs font-mono font-bold transition-colors flex items-center justify-center gap-1.5"
                                            >
                                                <Plus className="w-3.5 h-3.5" />
                                                Guardar en Banco
                                            </button>
                                        </div>

                                        <div className="lg:col-span-8 space-y-3">
                                            <div className="flex flex-wrap items-center gap-1.5">
                                                <button
                                                    onClick={() => setIdeaPlatformFilter('all')}
                                                    className={`px-2.5 py-1 rounded-lg text-[11px] font-mono border ${
                                                        ideaPlatformFilter === 'all'
                                                            ? 'bg-white/15 text-white border-white/30'
                                                            : 'bg-white/5 text-white/50 border-white/10'
                                                    }`}
                                                >
                                                    Todas
                                                </button>
                                                {Object.entries(PLATFORM_LABELS).map(([k, label]) => (
                                                    <button
                                                        key={k}
                                                        onClick={() => setIdeaPlatformFilter(k)}
                                                        className={`px-2.5 py-1 rounded-lg text-[11px] font-mono border ${
                                                            ideaPlatformFilter === k
                                                                ? 'bg-pink-500/20 text-pink-200 border-pink-400/40'
                                                                : 'bg-white/5 text-white/50 border-white/10'
                                                        }`}
                                                    >
                                                        {label}
                                                    </button>
                                                ))}
                                            </div>

                                            {filteredIdeas.length === 0 ? (
                                                <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                                    Todavía no hay ideas guardadas. Agregá una o pedile ideas a Aegis arriba.
                                                </div>
                                            ) : (
                                                <div className="space-y-2.5">
                                                    {filteredIdeas.map((idea) => (
                                                        <div
                                                            key={idea.id}
                                                            className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2"
                                                        >
                                                            <div className="flex flex-wrap items-start justify-between gap-2">
                                                                <div>
                                                                    <div className="flex items-center gap-2 flex-wrap">
                                                                        <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-pink-500/15 text-pink-200 border border-pink-400/25">
                                                                            {PLATFORM_LABELS[idea.platform] || idea.platform}
                                                                        </span>
                                                                        <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/5 text-white/55">
                                                                            {idea.pillar}
                                                                        </span>
                                                                    </div>
                                                                    <h5 className="text-sm font-semibold text-white mt-1.5">
                                                                        {idea.title}
                                                                    </h5>
                                                                </div>
                                                                <div className="flex items-center gap-1.5">
                                                                    <select
                                                                        value={idea.status}
                                                                        onChange={(e) =>
                                                                            void runActionAndRefresh(
                                                                                'aegis.domain.creator',
                                                                                'creator_manage_idea',
                                                                                {
                                                                                    action: 'update',
                                                                                    id: idea.id,
                                                                                    status: e.target.value,
                                                                                },
                                                                            )
                                                                        }
                                                                        className="px-2 py-1 rounded-lg bg-black/60 border border-white/15 text-[11px] font-mono text-aegis-cyan outline-none"
                                                                    >
                                                                        {IDEA_STAGES.map((st) => (
                                                                            <option key={st} value={st}>
                                                                                {IDEA_STAGE_LABELS[st]}
                                                                            </option>
                                                                        ))}
                                                                    </select>
                                                                    <button
                                                                        onClick={() =>
                                                                            askAegisWithPrompt(
                                                                                `Usá creator_manage_script para generar y guardar un guion completo para la idea '${idea.title}' (idea_id: '${idea.id}', plataforma: '${idea.platform}'${idea.hook ? `, hook base: '${idea.hook}'` : ''}). Incluí hook 0-3s, desarrollo hablado para teleprompter, notas de B-Roll, CTA, caption y hashtags.`,
                                                                            )
                                                                        }
                                                                        className="px-2.5 py-1 rounded-lg bg-aegis-cyan/15 hover:bg-aegis-cyan/25 text-aegis-cyan border border-aegis-cyan/30 text-[11px] font-mono flex items-center gap-1"
                                                                        title="Aegis redacta y guarda el guion en el estudio"
                                                                    >
                                                                        <Sparkles className="w-3 h-3" />
                                                                        Pedir Guion
                                                                    </button>
                                                                    <button
                                                                        onClick={() =>
                                                                            void runActionAndRefresh(
                                                                                'aegis.domain.creator',
                                                                                'creator_manage_idea',
                                                                                { action: 'delete', id: idea.id },
                                                                            )
                                                                        }
                                                                        className="p-1.5 rounded-lg text-white/35 hover:text-red-400 hover:bg-red-500/10"
                                                                        title="Eliminar idea"
                                                                    >
                                                                        <Trash2 className="w-3.5 h-3.5" />
                                                                    </button>
                                                                </div>
                                                            </div>
                                                            {idea.hook && (
                                                                <p className="text-xs text-amber-200/85 bg-amber-500/10 border border-amber-400/15 px-3 py-1.5 rounded-lg">
                                                                    <span className="font-mono text-[10px] uppercase text-amber-300/80 mr-1.5">
                                                                        Hook 0-3s:
                                                                    </span>
                                                                    {idea.hook}
                                                                </p>
                                                            )}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}

                                {/* Creator Tab 2: Scripts + Teleprompter */}
                                {creatorTab === 'scripts' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-5 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                                            <div className="flex items-center justify-between">
                                                <h4 className="text-xs font-mono uppercase tracking-wider text-pink-300 font-bold">
                                                    {scriptForm.id ? 'Editar Guion' : '+ Nuevo Guion para Grabar'}
                                                </h4>
                                                {scriptForm.id && (
                                                    <button
                                                        onClick={() =>
                                                            setScriptForm({
                                                                id: '',
                                                                title: '',
                                                                platform: 'tiktok_reels',
                                                                hook_0_3s: '',
                                                                body_script: '',
                                                                b_roll_notes: '',
                                                                cta: '',
                                                                caption: '',
                                                                hashtags: '',
                                                            })
                                                        }
                                                        className="text-[10px] font-mono text-white/50 hover:text-white"
                                                    >
                                                        Limpiar
                                                    </button>
                                                )}
                                            </div>
                                            <div className="grid grid-cols-3 gap-2">
                                                <input
                                                    value={scriptForm.title}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, title: e.target.value })}
                                                    placeholder="Título del guion"
                                                    className="col-span-2 px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none focus:border-pink-400"
                                                />
                                                <select
                                                    value={scriptForm.platform}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, platform: e.target.value })}
                                                    className="px-2 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                >
                                                    {Object.entries(PLATFORM_LABELS).map(([k, v]) => (
                                                        <option key={k} value={k}>
                                                            {v}
                                                        </option>
                                                    ))}
                                                </select>
                                            </div>
                                            <textarea
                                                value={scriptForm.hook_0_3s}
                                                onChange={(e) => setScriptForm({ ...scriptForm, hook_0_3s: e.target.value })}
                                                placeholder="Hook inicial (0-3s)..."
                                                rows={2}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none focus:border-pink-400 resize-none"
                                            />
                                            <textarea
                                                value={scriptForm.body_script}
                                                onChange={(e) => setScriptForm({ ...scriptForm, body_script: e.target.value })}
                                                placeholder="Guion hablado completo (texto para leer en Teleprompter)..."
                                                rows={5}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none focus:border-pink-400"
                                            />
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    value={scriptForm.cta}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, cta: e.target.value })}
                                                    placeholder="CTA final"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    value={scriptForm.b_roll_notes}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, b_roll_notes: e.target.value })}
                                                    placeholder="Notas de B-Roll / Edición"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    value={scriptForm.caption}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, caption: e.target.value })}
                                                    placeholder="Caption del post"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    value={scriptForm.hashtags}
                                                    onChange={(e) => setScriptForm({ ...scriptForm, hashtags: e.target.value })}
                                                    placeholder="#aegis #ia #tech"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <button
                                                disabled={busyTool || !scriptForm.title.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.creator', 'creator_manage_script', {
                                                        action: 'save',
                                                        ...scriptForm,
                                                    });
                                                    setScriptForm({
                                                        id: '',
                                                        title: '',
                                                        platform: 'tiktok_reels',
                                                        hook_0_3s: '',
                                                        body_script: '',
                                                        b_roll_notes: '',
                                                        cta: '',
                                                        caption: '',
                                                        hashtags: '',
                                                    });
                                                }}
                                                className="w-full py-2 rounded-xl bg-pink-500/80 hover:bg-pink-500 disabled:opacity-40 text-white text-xs font-mono font-bold transition-colors"
                                            >
                                                Guardar Guion
                                            </button>
                                        </div>

                                        <div className="lg:col-span-7 space-y-3">
                                            {creatorData.scripts.length === 0 ? (
                                                <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                                    No tenés guiones guardados todavía. Creá uno o usá "Pedir Guion" desde tus ideas.
                                                </div>
                                            ) : (
                                                creatorData.scripts.map((scr) => (
                                                    <div
                                                        key={scr.id}
                                                        className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2.5"
                                                    >
                                                        <div className="flex flex-wrap items-center justify-between gap-2">
                                                            <div>
                                                                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-pink-500/15 text-pink-200 border border-pink-400/25 mr-2">
                                                                    {PLATFORM_LABELS[scr.platform] || scr.platform}
                                                                </span>
                                                                {scr.estimated_duration_secs && (
                                                                    <span className="text-[10px] font-mono text-white/45">
                                                                        ~{scr.estimated_duration_secs}s
                                                                    </span>
                                                                )}
                                                                <h5 className="text-sm font-bold text-white mt-1">{scr.title}</h5>
                                                            </div>
                                                            <div className="flex flex-wrap items-center gap-1.5">
                                                                <button
                                                                    onClick={async () => {
                                                                        const vidMod = modules.find((m) => m.module_id === 'aegis.domain.video');
                                                                        if (vidMod && !vidMod.active) {
                                                                            await handleToggleModule(vidMod, true);
                                                                        }
                                                                        setPendingVideoScriptId(scr.id);
                                                                        setActiveApp('aegis.domain.video');
                                                                    }}
                                                                    className="px-2.5 py-1.5 rounded-lg bg-gradient-to-r from-purple-500/80 to-pink-500/80 text-white font-mono text-[11px] font-bold flex items-center gap-1.5 hover:opacity-90"
                                                                    title="Enviar este guion a Aegis Video Studio para generar voz, visuales y MP4"
                                                                >
                                                                    <Film className="w-3.5 h-3.5" />
                                                                    Crear Video IA
                                                                </button>
                                                                <button
                                                                    onClick={() => {
                                                                        setTeleprompterScript(scr);
                                                                        setTpPlaying(false);
                                                                    }}
                                                                    className="px-2.5 py-1.5 rounded-lg bg-aegis-cyan text-black font-mono text-[11px] font-bold flex items-center gap-1.5 hover:opacity-90"
                                                                >
                                                                    <Video className="w-3.5 h-3.5" />
                                                                    Teleprompter
                                                                </button>
                                                                <button
                                                                    onClick={() =>
                                                                        setScriptForm({
                                                                            id: scr.id,
                                                                            title: scr.title,
                                                                            platform: scr.platform,
                                                                            hook_0_3s: scr.hook_0_3s,
                                                                            body_script: scr.body_script,
                                                                            b_roll_notes: scr.b_roll_notes,
                                                                            cta: scr.cta,
                                                                            caption: scr.caption,
                                                                            hashtags: scr.hashtags,
                                                                        })
                                                                    }
                                                                    className="px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 text-xs font-mono text-white/75"
                                                                >
                                                                    Editar
                                                                </button>
                                                                <button
                                                                    onClick={() =>
                                                                        void runActionAndRefresh(
                                                                            'aegis.domain.creator',
                                                                            'creator_manage_script',
                                                                            { action: 'delete', id: scr.id },
                                                                        )
                                                                    }
                                                                    className="p-1.5 rounded-lg text-white/35 hover:text-red-400"
                                                                >
                                                                    <Trash2 className="w-3.5 h-3.5" />
                                                                </button>
                                                            </div>
                                                        </div>
                                                        {scr.hook_0_3s && (
                                                            <p className="text-xs text-amber-200/90">
                                                                <strong className="font-mono text-[10px] uppercase text-amber-400 mr-1.5">
                                                                    Hook:
                                                                </strong>
                                                                {scr.hook_0_3s}
                                                            </p>
                                                        )}
                                                        {scr.body_script && (
                                                            <p className="text-xs text-white/75 whitespace-pre-wrap line-clamp-3">
                                                                {scr.body_script}
                                                            </p>
                                                        )}
                                                        {(scr.caption || scr.hashtags) && (
                                                            <div className="flex items-center justify-between gap-2 pt-2 border-t border-white/10 text-[11px] text-white/50">
                                                                <span className="truncate">
                                                                    {scr.caption} {scr.hashtags}
                                                                </span>
                                                                <button
                                                                    onClick={() =>
                                                                        handleCopy(
                                                                            `cap-${scr.id}`,
                                                                            `${scr.caption}\n\n${scr.hashtags}`.trim(),
                                                                        )
                                                                    }
                                                                    className="shrink-0 flex items-center gap-1 text-aegis-cyan hover:underline font-mono"
                                                                >
                                                                    {copiedKey === `cap-${scr.id}` ? (
                                                                        <Check className="w-3 h-3" />
                                                                    ) : (
                                                                        <Copy className="w-3 h-3" />
                                                                    )}
                                                                    Copy
                                                                </button>
                                                            </div>
                                                        )}
                                                    </div>
                                                ))
                                            )}
                                        </div>
                                    </div>
                                )}

                                {/* Creator Tab 3: Calendar */}
                                {creatorTab === 'calendar' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-3 h-fit">
                                            <h4 className="text-xs font-mono uppercase tracking-wider text-pink-300 font-bold">
                                                + Programar Publicación
                                            </h4>
                                            <input
                                                value={postForm.title}
                                                onChange={(e) => setPostForm({ ...postForm, title: e.target.value })}
                                                placeholder="Título de la pieza..."
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <select
                                                value={postForm.platform}
                                                onChange={(e) => setPostForm({ ...postForm, platform: e.target.value })}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            >
                                                {Object.entries(PLATFORM_LABELS).map(([k, v]) => (
                                                    <option key={k} value={k}>
                                                        {v}
                                                    </option>
                                                ))}
                                            </select>
                                            <input
                                                value={postForm.publish_at}
                                                onChange={(e) => setPostForm({ ...postForm, publish_at: e.target.value })}
                                                placeholder="Fecha/hora (ej: 2026-11-20 19:00)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <textarea
                                                value={postForm.caption}
                                                onChange={(e) => setPostForm({ ...postForm, caption: e.target.value })}
                                                placeholder="Caption / descripción..."
                                                rows={2}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none resize-none"
                                            />
                                            <input
                                                value={postForm.hashtags}
                                                onChange={(e) => setPostForm({ ...postForm, hashtags: e.target.value })}
                                                placeholder="#hashtags"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <button
                                                disabled={busyTool || !postForm.title.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh(
                                                        'aegis.domain.creator',
                                                        'creator_schedule_post',
                                                        { action: 'schedule', ...postForm },
                                                    );
                                                    setPostForm({
                                                        title: '',
                                                        platform: 'tiktok_reels',
                                                        publish_at: '',
                                                        caption: '',
                                                        hashtags: '',
                                                    });
                                                }}
                                                className="w-full py-2 rounded-xl bg-pink-500/80 hover:bg-pink-500 disabled:opacity-40 text-white text-xs font-mono font-bold"
                                            >
                                                Agendar Publicación
                                            </button>
                                        </div>

                                        <div className="lg:col-span-8 space-y-2.5">
                                            {creatorData.calendar.length === 0 ? (
                                                <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                                    Tu calendario editorial está vacío.
                                                </div>
                                            ) : (
                                                creatorData.calendar.map((post) => (
                                                    <div
                                                        key={post.id}
                                                        className="p-4 rounded-xl bg-white/[0.03] border border-white/10 flex flex-wrap items-center justify-between gap-3"
                                                    >
                                                        <div className="space-y-1 min-w-0 flex-1">
                                                            <div className="flex items-center gap-2 text-[10px] font-mono">
                                                                <span className="px-2 py-0.5 rounded bg-pink-500/15 text-pink-200 border border-pink-400/25">
                                                                    {PLATFORM_LABELS[post.platform] || post.platform}
                                                                </span>
                                                                <span className="text-aegis-cyan">{post.publish_at}</span>
                                                                <span
                                                                    className={`px-2 py-0.5 rounded ${
                                                                        post.status === 'published'
                                                                            ? 'bg-emerald-500/20 text-emerald-300'
                                                                            : 'bg-white/10 text-white/60'
                                                                    }`}
                                                                >
                                                                    {post.status === 'published' ? 'PUBLICADO' : 'PROGRAMADO'}
                                                                </span>
                                                            </div>
                                                            <h5 className="text-sm font-semibold text-white">{post.title}</h5>
                                                            {(post.caption || post.hashtags) && (
                                                                <p className="text-xs text-white/55 truncate">
                                                                    {post.caption} {post.hashtags}
                                                                </p>
                                                            )}
                                                        </div>
                                                        <div className="flex items-center gap-1.5">
                                                            <button
                                                                onClick={() =>
                                                                    void runActionAndRefresh(
                                                                        'aegis.domain.creator',
                                                                        'creator_schedule_post',
                                                                        {
                                                                            action: 'update',
                                                                            id: post.id,
                                                                            status:
                                                                                post.status === 'published'
                                                                                    ? 'scheduled'
                                                                                    : 'published',
                                                                        },
                                                                    )
                                                                }
                                                                className="px-2.5 py-1.5 rounded-lg bg-white/5 hover:bg-emerald-500/20 text-xs font-mono text-white/75 hover:text-emerald-200"
                                                            >
                                                                {post.status === 'published' ? 'Reprogramar' : 'Marcar Publicado'}
                                                            </button>
                                                            <button
                                                                onClick={() =>
                                                                    void runActionAndRefresh(
                                                                        'aegis.domain.creator',
                                                                        'creator_schedule_post',
                                                                        { action: 'delete', id: post.id },
                                                                    )
                                                                }
                                                                className="p-1.5 rounded-lg text-white/35 hover:text-red-400"
                                                            >
                                                                <Trash2 className="w-3.5 h-3.5" />
                                                            </button>
                                                        </div>
                                                    </div>
                                                ))
                                            )}
                                        </div>
                                    </div>
                                )}
                            </div>
                        )}

                        {/* ═══════════════ 2. AEGIS DEV STUDIO ═══════════════ */}
                        {activeApp === 'aegis.domain.dev' && (
                            <div className="space-y-5">
                                <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Proyectos</p>
                                        <p className="text-2xl font-bold text-white mt-1">{devData.stats.total_projects}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Tareas Abiertas</p>
                                        <p className="text-2xl font-bold text-aegis-cyan mt-1">{devData.stats.open_tasks}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">En Curso</p>
                                        <p className="text-2xl font-bold text-amber-300 mt-1">{devData.stats.in_progress_tasks}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Bugs Activos</p>
                                        <p className="text-2xl font-bold text-red-400 mt-1">{devData.stats.bugs_count}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Snippets</p>
                                        <p className="text-2xl font-bold text-emerald-300 mt-1">{devData.stats.total_snippets}</p>
                                    </div>
                                </div>

                                <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-3">
                                    <div className="flex items-center gap-2">
                                        {[
                                            { id: 'kanban', label: 'Kanban Técnico', icon: Bug },
                                            { id: 'projects', label: 'Proyectos & Repos', icon: FolderGit2 },
                                            { id: 'snippets', label: 'Snippets & Runbooks', icon: Terminal },
                                        ].map((tab) => {
                                            const TIcon = tab.icon;
                                            return (
                                                <button
                                                    key={tab.id}
                                                    onClick={() => setDevTab(tab.id as 'kanban' | 'projects' | 'snippets')}
                                                    className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                                                        devTab === tab.id
                                                            ? 'bg-aegis-cyan/20 text-aegis-cyan border-aegis-cyan/40'
                                                            : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                                                    }`}
                                                >
                                                    <TIcon className="w-3.5 h-3.5" />
                                                    {tab.label}
                                                </button>
                                            );
                                        })}
                                    </div>

                                    <button
                                        onClick={() => {
                                            onClose();
                                            setCurrentView('dashboard');
                                        }}
                                        className="px-3.5 py-2 rounded-xl text-xs font-mono bg-white/5 hover:bg-white/10 border border-white/15 text-white/80 flex items-center gap-2"
                                    >
                                        <Code2 className="w-3.5 h-3.5 text-aegis-cyan" />
                                        Abrir Terminal / Git / Código
                                    </button>
                                </div>

                                {/* Dev Tab 1: Kanban */}
                                {devTab === 'kanban' && (
                                    <div className="space-y-4">
                                        <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 grid grid-cols-1 sm:grid-cols-12 gap-2.5 items-center">
                                            <input
                                                value={newTaskTitle}
                                                onChange={(e) => setNewTaskTitle(e.target.value)}
                                                placeholder="Nueva tarea técnica o bug..."
                                                className="sm:col-span-5 px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none focus:border-aegis-cyan"
                                            />
                                            <select
                                                value={newTaskKind}
                                                onChange={(e) => setNewTaskKind(e.target.value)}
                                                className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            >
                                                <option value="feature">Feature</option>
                                                <option value="bug">Bug</option>
                                                <option value="refactor">Refactor</option>
                                                <option value="security">Security</option>
                                                <option value="devops">DevOps</option>
                                            </select>
                                            <select
                                                value={newTaskPriority}
                                                onChange={(e) => setNewTaskPriority(e.target.value)}
                                                className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            >
                                                <option value="critical">Crítica</option>
                                                <option value="high">Alta</option>
                                                <option value="medium">Media</option>
                                                <option value="low">Baja</option>
                                            </select>
                                            <input
                                                value={newTaskFiles}
                                                onChange={(e) => setNewTaskFiles(e.target.value)}
                                                placeholder="Archivos (opc.)"
                                                className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <button
                                                disabled={busyTool || !newTaskTitle.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.dev', 'dev_manage_task', {
                                                        action: 'add',
                                                        title: newTaskTitle.trim(),
                                                        kind: newTaskKind,
                                                        priority: newTaskPriority,
                                                        file_refs: newTaskFiles.trim(),
                                                    });
                                                    setNewTaskTitle('');
                                                    setNewTaskFiles('');
                                                }}
                                                className="sm:col-span-1 py-2 px-3 rounded-lg bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40 flex items-center justify-center"
                                            >
                                                <Plus className="w-4 h-4" />
                                            </button>
                                        </div>

                                        <div className="grid grid-cols-1 md:grid-cols-4 gap-3.5">
                                            {DEV_COLUMNS.map((col, colIdx) => {
                                                const colTasks = devData.tasks.filter((t) => t.status === col);
                                                return (
                                                    <div
                                                        key={col}
                                                        className="p-3 rounded-xl bg-white/[0.02] border border-white/10 flex flex-col gap-2.5 min-h-[260px]"
                                                    >
                                                        <div className="flex items-center justify-between text-xs font-mono uppercase tracking-wider text-white/60 pb-1 border-b border-white/10">
                                                            <span>{DEV_COLUMN_LABELS[col]}</span>
                                                            <span className="px-2 py-0.5 rounded-full bg-white/5 text-[10px]">
                                                                {colTasks.length}
                                                            </span>
                                                        </div>
                                                        {colTasks.map((t) => (
                                                            <div
                                                                key={t.id}
                                                                className="p-3 rounded-lg bg-black/50 border border-white/10 space-y-2"
                                                            >
                                                                <div className="flex items-center justify-between gap-1 text-[10px] font-mono">
                                                                    <span
                                                                        className={`px-1.5 py-0.5 rounded uppercase ${
                                                                            t.kind === 'bug'
                                                                                ? 'bg-red-500/20 text-red-300'
                                                                                : t.kind === 'security'
                                                                                  ? 'bg-amber-500/20 text-amber-300'
                                                                                  : 'bg-aegis-cyan/15 text-aegis-cyan'
                                                                        }`}
                                                                    >
                                                                        {t.kind}
                                                                    </span>
                                                                    <span className="text-white/40">{t.priority}</span>
                                                                </div>
                                                                <p className="text-xs font-medium text-white">{t.title}</p>
                                                                {t.file_refs && (
                                                                    <p className="text-[10px] font-mono text-white/40 truncate">
                                                                        {t.file_refs}
                                                                    </p>
                                                                )}
                                                                <div className="flex items-center justify-between pt-1 border-t border-white/10">
                                                                    <div className="flex items-center gap-1">
                                                                        {colIdx > 0 && (
                                                                            <button
                                                                                onClick={() =>
                                                                                    void runActionAndRefresh(
                                                                                        'aegis.domain.dev',
                                                                                        'dev_manage_task',
                                                                                        {
                                                                                            action: 'update',
                                                                                            id: t.id,
                                                                                            status: DEV_COLUMNS[colIdx - 1],
                                                                                        },
                                                                                    )
                                                                                }
                                                                                className="p-1 rounded hover:bg-white/10 text-white/50"
                                                                                title="Mover atrás"
                                                                            >
                                                                                <ArrowLeft className="w-3 h-3" />
                                                                            </button>
                                                                        )}
                                                                        {colIdx < DEV_COLUMNS.length - 1 && (
                                                                            <button
                                                                                onClick={() =>
                                                                                    void runActionAndRefresh(
                                                                                        'aegis.domain.dev',
                                                                                        'dev_manage_task',
                                                                                        {
                                                                                            action: 'update',
                                                                                            id: t.id,
                                                                                            status: DEV_COLUMNS[colIdx + 1],
                                                                                        },
                                                                                    )
                                                                                }
                                                                                className="p-1 rounded hover:bg-white/10 text-white/50"
                                                                                title="Avanzar etapa"
                                                                            >
                                                                                <ArrowRight className="w-3 h-3" />
                                                                            </button>
                                                                        )}
                                                                    </div>
                                                                    <div className="flex items-center gap-1">
                                                                        <button
                                                                            onClick={() =>
                                                                                askAegisWithPrompt(
                                                                                    `Delegá a un agente especializado esta tarea de Aegis Dev Studio (${t.kind} / prioridad ${t.priority}): "${t.title}"${t.file_refs ? ` en ${t.file_refs}` : ''}${t.notes ? `. Contexto: ${t.notes}` : ''}. Cuando termines, actualizá su estado con dev_manage_task.`,
                                                                                )
                                                                            }
                                                                            className="px-1.5 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan text-[10px] font-mono flex items-center gap-1 hover:bg-aegis-cyan/25"
                                                                            title="Delegar a Aegis / Supervisor"
                                                                        >
                                                                            <Bot className="w-3 h-3" />
                                                                            Delegar
                                                                        </button>
                                                                        <button
                                                                            onClick={() =>
                                                                                void runActionAndRefresh(
                                                                                    'aegis.domain.dev',
                                                                                    'dev_manage_task',
                                                                                    { action: 'delete', id: t.id },
                                                                                )
                                                                            }
                                                                            className="p-1 rounded text-white/30 hover:text-red-400"
                                                                        >
                                                                            <Trash2 className="w-3 h-3" />
                                                                        </button>
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        ))}
                                                    </div>
                                                );
                                            })}
                                        </div>
                                    </div>
                                )}

                                {/* Dev Tab 2: Projects */}
                                {devTab === 'projects' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                                            <h4 className="text-xs font-mono uppercase text-aegis-cyan font-bold">
                                                + Registrar Proyecto / Repo
                                            </h4>
                                            <input
                                                value={newProjForm.name}
                                                onChange={(e) => setNewProjForm({ ...newProjForm, name: e.target.value })}
                                                placeholder="Nombre (ej: Aegis-Core)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <input
                                                value={newProjForm.stack}
                                                onChange={(e) => setNewProjForm({ ...newProjForm, stack: e.target.value })}
                                                placeholder="Stack (Rust, React, Python...)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <input
                                                value={newProjForm.repo_path}
                                                onChange={(e) => setNewProjForm({ ...newProjForm, repo_path: e.target.value })}
                                                placeholder="Ruta local o URL repo"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <input
                                                value={newProjForm.build_cmd}
                                                onChange={(e) => setNewProjForm({ ...newProjForm, build_cmd: e.target.value })}
                                                placeholder="Build cmd (ej: cargo build --release)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <input
                                                value={newProjForm.test_cmd}
                                                onChange={(e) => setNewProjForm({ ...newProjForm, test_cmd: e.target.value })}
                                                placeholder="Test cmd (ej: cargo test)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <button
                                                disabled={busyTool || !newProjForm.name.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.dev', 'dev_manage_project', {
                                                        action: 'add',
                                                        ...newProjForm,
                                                    });
                                                    setNewProjForm({
                                                        name: '',
                                                        stack: 'Rust + React',
                                                        repo_path: '',
                                                        build_cmd: '',
                                                        test_cmd: '',
                                                    });
                                                }}
                                                className="w-full py-2 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40"
                                            >
                                                Guardar Proyecto
                                            </button>
                                        </div>

                                        <div className="lg:col-span-8 space-y-3">
                                            {devData.projects.length === 0 ? (
                                                <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                                    Sin proyectos registrados todavía.
                                                </div>
                                            ) : (
                                                devData.projects.map((p) => (
                                                    <div
                                                        key={p.id}
                                                        className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2"
                                                    >
                                                        <div className="flex items-start justify-between gap-2">
                                                            <div>
                                                                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan mr-2">
                                                                    {p.stack}
                                                                </span>
                                                                <span className="text-[10px] font-mono text-white/45">
                                                                    rama: {p.default_branch}
                                                                </span>
                                                                <h5 className="text-sm font-bold text-white mt-1">{p.name}</h5>
                                                                {p.repo_path && (
                                                                    <p className="text-xs font-mono text-white/45">{p.repo_path}</p>
                                                                )}
                                                            </div>
                                                            <button
                                                                onClick={() =>
                                                                    void runActionAndRefresh(
                                                                        'aegis.domain.dev',
                                                                        'dev_manage_project',
                                                                        { action: 'delete', id: p.id },
                                                                    )
                                                                }
                                                                className="p-1.5 text-white/30 hover:text-red-400"
                                                            >
                                                                <Trash2 className="w-3.5 h-3.5" />
                                                            </button>
                                                        </div>
                                                        <div className="flex flex-wrap gap-2 pt-1">
                                                            {p.build_cmd && (
                                                                <button
                                                                    onClick={() => handleCopy(`b-${p.id}`, p.build_cmd)}
                                                                    className="px-2.5 py-1 rounded-lg bg-black/50 border border-white/10 text-[11px] font-mono text-white/75 hover:text-aegis-cyan flex items-center gap-1.5"
                                                                >
                                                                    <Copy className="w-3 h-3" />
                                                                    Build: {p.build_cmd}
                                                                </button>
                                                            )}
                                                            {p.test_cmd && (
                                                                <button
                                                                    onClick={() => handleCopy(`t-${p.id}`, p.test_cmd)}
                                                                    className="px-2.5 py-1 rounded-lg bg-black/50 border border-white/10 text-[11px] font-mono text-white/75 hover:text-emerald-300 flex items-center gap-1.5"
                                                                >
                                                                    <Copy className="w-3 h-3" />
                                                                    Test: {p.test_cmd}
                                                                </button>
                                                            )}
                                                        </div>
                                                    </div>
                                                ))
                                            )}
                                        </div>
                                    </div>
                                )}

                                {/* Dev Tab 3: Snippets */}
                                {devTab === 'snippets' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                                            <h4 className="text-xs font-mono uppercase text-aegis-cyan font-bold">
                                                + Guardar Snippet / Runbook
                                            </h4>
                                            <div className="grid grid-cols-3 gap-2">
                                                <input
                                                    value={newSnippetForm.title}
                                                    onChange={(e) => setNewSnippetForm({ ...newSnippetForm, title: e.target.value })}
                                                    placeholder="Título"
                                                    className="col-span-2 px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    value={newSnippetForm.language}
                                                    onChange={(e) => setNewSnippetForm({ ...newSnippetForm, language: e.target.value })}
                                                    placeholder="bash/rust"
                                                    className="px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <textarea
                                                value={newSnippetForm.code}
                                                onChange={(e) => setNewSnippetForm({ ...newSnippetForm, code: e.target.value })}
                                                placeholder="Pegá el comando o código..."
                                                rows={5}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs font-mono text-white outline-none"
                                            />
                                            <input
                                                value={newSnippetForm.tags}
                                                onChange={(e) => setNewSnippetForm({ ...newSnippetForm, tags: e.target.value })}
                                                placeholder="Tags (docker, deploy, sql)"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <button
                                                disabled={busyTool || !newSnippetForm.title.trim()}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.dev', 'dev_manage_snippet', {
                                                        action: 'save',
                                                        ...newSnippetForm,
                                                    });
                                                    setNewSnippetForm({ title: '', language: 'bash', code: '', tags: '' });
                                                }}
                                                className="w-full py-2 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40"
                                            >
                                                Guardar en Bóveda
                                            </button>
                                        </div>

                                        <div className="lg:col-span-8 space-y-3">
                                            <div className="relative">
                                                <Search className="w-3.5 h-3.5 text-white/35 absolute left-3 top-2.5" />
                                                <input
                                                    value={snippetSearch}
                                                    onChange={(e) => setSnippetSearch(e.target.value)}
                                                    placeholder="Buscar snippet por título, lenguaje o tag..."
                                                    className="w-full pl-9 pr-3 py-2 rounded-lg bg-white/5 border border-white/10 text-xs text-white outline-none"
                                                />
                                            </div>
                                            {devData.snippets
                                                .filter(
                                                    (s) =>
                                                        !snippetSearch.trim() ||
                                                        `${s.title} ${s.language} ${s.tags}`
                                                            .toLowerCase()
                                                            .includes(snippetSearch.toLowerCase()),
                                                )
                                                .map((snip) => (
                                                    <div
                                                        key={snip.id}
                                                        className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2"
                                                    >
                                                        <div className="flex items-center justify-between gap-2">
                                                            <div className="flex items-center gap-2">
                                                                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan">
                                                                    {snip.language}
                                                                </span>
                                                                <h5 className="text-sm font-semibold text-white">{snip.title}</h5>
                                                            </div>
                                                            <div className="flex items-center gap-1.5">
                                                                <button
                                                                    onClick={() => handleCopy(`snp-${snip.id}`, snip.code)}
                                                                    className="px-2.5 py-1 rounded-lg bg-white/5 hover:bg-white/10 text-[11px] font-mono text-aegis-cyan flex items-center gap-1"
                                                                >
                                                                    {copiedKey === `snp-${snip.id}` ? (
                                                                        <Check className="w-3 h-3" />
                                                                    ) : (
                                                                        <Copy className="w-3 h-3" />
                                                                    )}
                                                                    Copiar
                                                                </button>
                                                                <button
                                                                    onClick={() =>
                                                                        void runActionAndRefresh(
                                                                            'aegis.domain.dev',
                                                                            'dev_manage_snippet',
                                                                            { action: 'delete', id: snip.id },
                                                                        )
                                                                    }
                                                                    className="p-1.5 text-white/30 hover:text-red-400"
                                                                >
                                                                    <Trash2 className="w-3.5 h-3.5" />
                                                                </button>
                                                            </div>
                                                        </div>
                                                        <pre className="p-3 rounded-lg bg-black/60 border border-white/5 text-[11px] font-mono text-white/80 overflow-x-auto">
                                                            {snip.code}
                                                        </pre>
                                                    </div>
                                                ))}
                                        </div>
                                    </div>
                                )}
                            </div>
                        )}

                        {/* ═══════════════ 3. AEGIS BUSINESS & POS ═══════════════ */}
                        {activeApp === 'aegis.domain.business' && (
                            <div className="space-y-5">
                                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Productos</p>
                                        <p className="text-2xl font-bold text-white mt-1">{bizData.stats.total_products}</p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Alertas Stock Bajo</p>
                                        <p
                                            className={`text-2xl font-bold mt-1 ${
                                                bizData.stats.low_stock_count > 0 ? 'text-amber-300' : 'text-emerald-300'
                                            }`}
                                        >
                                            {bizData.stats.low_stock_count}
                                        </p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Valor Inventario</p>
                                        <p className="text-2xl font-bold text-aegis-cyan mt-1">
                                            ${bizData.stats.inventory_value.toLocaleString('es-AR')}
                                        </p>
                                    </div>
                                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                                        <p className="text-[10px] font-mono text-white/40 uppercase">Ingresos Ventas</p>
                                        <p className="text-2xl font-bold text-emerald-300 mt-1">
                                            ${bizData.stats.total_revenue.toLocaleString('es-AR')}
                                        </p>
                                    </div>
                                </div>

                                <div className="flex items-center gap-2 border-b border-white/10 pb-3">
                                    <button
                                        onClick={() => setBizTab('pos')}
                                        className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                                            bizTab === 'pos'
                                                ? 'bg-emerald-500/20 text-emerald-200 border-emerald-400/40'
                                                : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                                        }`}
                                    >
                                        <ShoppingBag className="w-3.5 h-3.5" />
                                        Punto de Venta (POS)
                                    </button>
                                    <button
                                        onClick={() => setBizTab('inventory')}
                                        className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                                            bizTab === 'inventory'
                                                ? 'bg-emerald-500/20 text-emerald-200 border-emerald-400/40'
                                                : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                                        }`}
                                    >
                                        <Package className="w-3.5 h-3.5" />
                                        Catálogo & Stock
                                    </button>
                                </div>

                                {/* Biz Tab 1: POS */}
                                {bizTab === 'pos' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-5 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-3 h-fit">
                                            <h4 className="text-xs font-mono uppercase text-emerald-300 font-bold">
                                                Nueva Venta (descuenta stock automático)
                                            </h4>
                                            <select
                                                value={posProductKey}
                                                onChange={(e) => {
                                                    setPosProductKey(e.target.value);
                                                    const found = bizData.products.find((p) => p.id === e.target.value);
                                                    if (found) setPosUnitPrice(String(found.price));
                                                }}
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            >
                                                <option value="">-- Elegí un producto del catálogo --</option>
                                                {bizData.products.map((p) => (
                                                    <option key={p.id} value={p.id}>
                                                        {p.name} (Stock: {p.stock} · ${p.price})
                                                    </option>
                                                ))}
                                            </select>
                                            <div className="grid grid-cols-2 gap-2">
                                                <div>
                                                    <label className="text-[10px] font-mono text-white/40 block mb-1">
                                                        Cantidad
                                                    </label>
                                                    <input
                                                        type="number"
                                                        min={1}
                                                        value={posQty}
                                                        onChange={(e) => setPosQty(Math.max(1, Number(e.target.value) || 1))}
                                                        className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                    />
                                                </div>
                                                <div>
                                                    <label className="text-[10px] font-mono text-white/40 block mb-1">
                                                        Precio Unitario ($)
                                                    </label>
                                                    <input
                                                        type="number"
                                                        value={posUnitPrice}
                                                        onChange={(e) => setPosUnitPrice(e.target.value)}
                                                        placeholder="Precio"
                                                        className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                    />
                                                </div>
                                            </div>
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    value={posCustomer}
                                                    onChange={(e) => setPosCustomer(e.target.value)}
                                                    placeholder="Cliente"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <select
                                                    value={posPayment}
                                                    onChange={(e) => setPosPayment(e.target.value)}
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                >
                                                    <option value="efectivo">Efectivo</option>
                                                    <option value="transferencia">Transferencia</option>
                                                    <option value="tarjeta">Tarjeta</option>
                                                    <option value="qr">QR</option>
                                                </select>
                                            </div>
                                            <div className="p-3 rounded-xl bg-emerald-500/10 border border-emerald-400/20 flex items-center justify-between text-sm font-mono">
                                                <span className="text-white/70">TOTAL:</span>
                                                <span className="font-bold text-emerald-300">
                                                    ${(effectivePosPrice * posQty).toLocaleString('es-AR')}
                                                </span>
                                            </div>
                                            <button
                                                disabled={busyTool || !posProductKey}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.business', 'biz_record_sale', {
                                                        product_id: posProductKey,
                                                        quantity: posQty,
                                                        unit_price: effectivePosPrice,
                                                        customer: posCustomer.trim() || 'Consumidor Final',
                                                        payment_method: posPayment,
                                                    });
                                                    setPosQty(1);
                                                }}
                                                className="w-full py-2.5 rounded-xl bg-emerald-500 hover:bg-emerald-400 disabled:opacity-40 text-black font-mono text-xs font-bold"
                                            >
                                                Cobrar y Registrar Venta
                                            </button>
                                        </div>

                                        <div className="lg:col-span-7 space-y-2.5">
                                            <h4 className="text-xs font-mono uppercase text-white/50">
                                                Últimas Ventas ({bizData.sales.length})
                                            </h4>
                                            {bizData.sales.length === 0 ? (
                                                <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                                    Sin ventas registradas aún. También podés decirle a Aegis: "Vendí 2 unidades de X".
                                                </div>
                                            ) : (
                                                <div className="space-y-2">
                                                    {bizData.sales.slice(0, 20).map((s) => (
                                                        <div
                                                            key={s.id}
                                                            className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 flex items-center justify-between gap-3"
                                                        >
                                                            <div>
                                                                <p className="text-xs font-semibold text-white">
                                                                    {s.quantity}× {s.product_name}
                                                                </p>
                                                                <p className="text-[10px] font-mono text-white/45">
                                                                    {s.customer} · {s.payment_method} · {s.created_at}
                                                                </p>
                                                            </div>
                                                            <span className="text-sm font-mono font-bold text-emerald-300">
                                                                +${s.total.toLocaleString('es-AR')}
                                                            </span>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}

                                {/* Biz Tab 2: Inventory */}
                                {bizTab === 'inventory' && (
                                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                                        <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                                            <h4 className="text-xs font-mono uppercase text-emerald-300 font-bold">
                                                + Agregar Producto
                                            </h4>
                                            <input
                                                value={newProductForm.name}
                                                onChange={(e) => setNewProductForm({ ...newProductForm, name: e.target.value })}
                                                placeholder="Nombre del producto"
                                                className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    value={newProductForm.barcode}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, barcode: e.target.value })}
                                                    placeholder="SKU / Código"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    value={newProductForm.category}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, category: e.target.value })}
                                                    placeholder="Categoría"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    type="number"
                                                    value={newProductForm.price}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, price: e.target.value })}
                                                    placeholder="Precio venta ($)"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    type="number"
                                                    value={newProductForm.cost}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, cost: e.target.value })}
                                                    placeholder="Costo ($)"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <div className="grid grid-cols-2 gap-2">
                                                <input
                                                    type="number"
                                                    value={newProductForm.stock}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, stock: e.target.value })}
                                                    placeholder="Stock"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                                <input
                                                    type="number"
                                                    value={newProductForm.min_stock}
                                                    onChange={(e) => setNewProductForm({ ...newProductForm, min_stock: e.target.value })}
                                                    placeholder="Stock mínimo"
                                                    className="px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <button
                                                disabled={busyTool || !newProductForm.name.trim() || !newProductForm.price}
                                                onClick={async () => {
                                                    await runActionAndRefresh('aegis.domain.business', 'biz_add_product', {
                                                        barcode: newProductForm.barcode.trim(),
                                                        name: newProductForm.name.trim(),
                                                        category: newProductForm.category.trim() || 'General',
                                                        price: Number(newProductForm.price) || 0,
                                                        cost: Number(newProductForm.cost) || 0,
                                                        stock: Number(newProductForm.stock) || 0,
                                                        min_stock: Number(newProductForm.min_stock) || 3,
                                                    });
                                                    setNewProductForm({
                                                        barcode: '',
                                                        name: '',
                                                        category: 'General',
                                                        price: '',
                                                        cost: '',
                                                        stock: '1',
                                                        min_stock: '3',
                                                    });
                                                }}
                                                className="w-full py-2 rounded-xl bg-emerald-500 hover:bg-emerald-400 disabled:opacity-40 text-black font-mono text-xs font-bold"
                                            >
                                                Guardar Producto
                                            </button>
                                        </div>

                                        <div className="lg:col-span-8 space-y-3">
                                            <div className="relative">
                                                <Search className="w-3.5 h-3.5 text-white/35 absolute left-3 top-2.5" />
                                                <input
                                                    value={productSearch}
                                                    onChange={(e) => setProductSearch(e.target.value)}
                                                    placeholder="Buscar por nombre, categoría o SKU..."
                                                    className="w-full pl-9 pr-3 py-2 rounded-lg bg-white/5 border border-white/10 text-xs text-white outline-none"
                                                />
                                            </div>
                                            <div className="space-y-2">
                                                {bizData.products
                                                    .filter(
                                                        (p) =>
                                                            !productSearch.trim() ||
                                                            `${p.name} ${p.category} ${p.barcode}`
                                                                .toLowerCase()
                                                                .includes(productSearch.toLowerCase()),
                                                    )
                                                    .map((prod) => {
                                                        const low = prod.stock <= prod.min_stock;
                                                        return (
                                                            <div
                                                                key={prod.id}
                                                                className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 flex flex-wrap items-center justify-between gap-3"
                                                            >
                                                                <div className="min-w-0">
                                                                    <div className="flex items-center gap-2 flex-wrap">
                                                                        <h5 className="text-sm font-semibold text-white">{prod.name}</h5>
                                                                        <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/5 text-white/55">
                                                                            {prod.category}
                                                                        </span>
                                                                        {low && (
                                                                            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/20 text-amber-300 border border-amber-400/30">
                                                                                ⚠ Stock Bajo
                                                                            </span>
                                                                        )}
                                                                    </div>
                                                                    <p className="text-[11px] font-mono text-white/45 mt-0.5">
                                                                        Venta: ${prod.price.toLocaleString('es-AR')}
                                                                        {prod.barcode ? ` · SKU: ${prod.barcode}` : ''}
                                                                    </p>
                                                                </div>
                                                                <div className="flex items-center gap-2">
                                                                    <button
                                                                        onClick={() =>
                                                                            void runActionAndRefresh(
                                                                                'aegis.domain.business',
                                                                                'biz_update_stock',
                                                                                { product_id: prod.id, quantity_change: -1 },
                                                                            )
                                                                        }
                                                                        className="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/15 text-white font-mono text-xs"
                                                                    >
                                                                        -1
                                                                    </button>
                                                                    <span
                                                                        className={`min-w-[44px] text-center font-mono text-sm font-bold ${
                                                                            low ? 'text-amber-300' : 'text-emerald-300'
                                                                        }`}
                                                                    >
                                                                        {prod.stock}
                                                                    </span>
                                                                    <button
                                                                        onClick={() =>
                                                                            void runActionAndRefresh(
                                                                                'aegis.domain.business',
                                                                                'biz_update_stock',
                                                                                { product_id: prod.id, quantity_change: 1 },
                                                                            )
                                                                        }
                                                                        className="w-7 h-7 rounded-lg bg-white/5 hover:bg-white/15 text-white font-mono text-xs"
                                                                    >
                                                                        +1
                                                                    </button>
                                                                    <button
                                                                        onClick={() =>
                                                                            void runActionAndRefresh(
                                                                                'aegis.domain.business',
                                                                                'biz_delete_product',
                                                                                { id: prod.id },
                                                                            )
                                                                        }
                                                                        className="p-1.5 text-white/30 hover:text-red-400"
                                                                    >
                                                                        <Trash2 className="w-3.5 h-3.5" />
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        );
                                                    })}
                                            </div>
                                        </div>
                                    </div>
                                )}
                            </div>
                        )}

                        {/* ═══════════════ 1B. AEGIS VIDEO STUDIO ═══════════════ */}
                        {activeApp === 'aegis.domain.video' && (
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
