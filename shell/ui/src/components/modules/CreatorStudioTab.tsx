import React, { useState } from 'react';
import {
    Film,
    Sparkles,
    Plus,
    Trash2,
    Copy,
    Check,
    Calendar,
    FileText,
    Lightbulb,
    Video,
} from 'lucide-react';
import {
    CreatorOverview,
    CreatorPostForm,
    CreatorScript,
    IDEA_STAGES,
    IDEA_STAGE_LABELS,
    PLATFORM_LABELS,
    RunModuleAction,
} from './types';

export interface CreatorStudioTabProps {
    creatorData: CreatorOverview;
    creatorTab: 'ideas' | 'scripts' | 'calendar';
    setCreatorTab: React.Dispatch<React.SetStateAction<'ideas' | 'scripts' | 'calendar'>>;
    postForm: CreatorPostForm;
    setPostForm: React.Dispatch<React.SetStateAction<CreatorPostForm>>;
    busyTool: boolean;
    copiedKey: string | null;
    handleCopy: (key: string, text: string) => void;
    runActionAndRefresh: RunModuleAction;
    askAegisWithPrompt: (prompt: string) => void;
    onOpenTeleprompter: (script: CreatorScript) => void;
    onOpenVideoFromScript: (scriptId: string) => void;
}

export const CreatorStudioTab: React.FC<CreatorStudioTabProps> = ({
    creatorData,
    creatorTab,
    setCreatorTab,
    postForm,
    setPostForm,
    busyTool,
    copiedKey,
    handleCopy,
    runActionAndRefresh,
    askAegisWithPrompt,
    onOpenTeleprompter,
    onOpenVideoFromScript,
}) => {
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

    const filteredIdeas = creatorData.ideas.filter(
        (i) => ideaPlatformFilter === 'all' || i.platform === ideaPlatformFilter,
    );

    return (
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
                                                onClick={() => void onOpenVideoFromScript(scr.id)}
                                                className="px-2.5 py-1.5 rounded-lg bg-gradient-to-r from-purple-500/80 to-pink-500/80 text-white font-mono text-[11px] font-bold flex items-center gap-1.5 hover:opacity-90"
                                                title="Enviar este guion a Aegis Video Studio para generar voz, visuales y MP4"
                                            >
                                                <Film className="w-3.5 h-3.5" />
                                                Crear Video IA
                                            </button>
                                            <button
                                                onClick={() => onOpenTeleprompter(scr)}
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
    );
};
