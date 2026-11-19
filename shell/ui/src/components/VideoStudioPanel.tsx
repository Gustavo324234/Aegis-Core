import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
    Film,
    Sparkles,
    Plus,
    Trash2,
    Download,
    Play,
    Settings2,
    Cpu,
    Wand2,
    Upload,
    RefreshCw,
    CheckCircle2,
    AlertTriangle,
    Loader2,
    Mic,
    Image as ImageIcon,
    Video,
    Subtitles,
    Wrench,
    ChevronUp,
    ChevronDown,
    FileText,
    Calendar,
} from 'lucide-react';

export interface VideoScene {
    id: string;
    order: number;
    title: string;
    narration_text: string;
    on_screen_text: string;
    visual_prompt: string;
    stock_keywords: string;
    duration_hint_secs: number;
    motion_effect: string;
    voice_override?: string | null;
    visual_override?: string | null;
    audio_file?: string | null;
    visual_file?: string | null;
    visual_kind?: string | null;
    actual_audio_secs?: number | null;
    status: string;
    error?: string | null;
}

export interface VideoProject {
    id: string;
    title: string;
    prompt: string;
    source_script_id?: string | null;
    platform: string;
    aspect_ratio: string;
    preset: string;
    voice_engine: string;
    voice_id: string;
    visual_engine: string;
    hook_video_engine: string;
    subtitle_engine: string;
    subtitle_style: string;
    bgm_style?: string;
    scenes: VideoScene[];
    output_mp4?: string | null;
    output_srt?: string | null;
    status: string;
    last_error?: string | null;
    fallbacks_applied?: string[];
    created_at: string;
    updated_at: string;
}

interface HardwareSpecs {
    cpu_brand: string;
    cpu_cores: number;
    total_ram_gb: number;
    available_ram_gb: number;
    free_disk_gb: number;
    gpu_name: string;
    dedicated_vram_mb: number;
    has_nvidia_cuda: boolean;
}

interface LocalModelRecommendation {
    category: string;
    model_name: string;
    min_ram_gb: number;
    min_vram_gb: number;
    compatible_with_this_pc: boolean;
    status_note: string;
}

interface ApiPlanRecommendation {
    provider: string;
    current_status: string;
    recommended_plan: string;
    what_it_unlocks: string;
    estimated_cost: string;
}

interface VideoCapabilityAdvisor {
    pc_specs: HardwareSpecs;
    pc_verdict: string;
    local_models: LocalModelRecommendation[];
    api_plans: ApiPlanRecommendation[];
}

interface RuntimeStatus {
    ffmpeg_ready: boolean;
    ffprobe_ready: boolean;
    python_ready: boolean;
    edge_tts_ready: boolean;
    pillow_ready: boolean;
    ready_for_free_video: boolean;
    ffmpeg_source?: string | null;
    python_source?: string | null;
    runtime_dir: string;
}

interface ProviderReadiness {
    has_gemini_key: boolean;
    has_openai_key: boolean;
    has_groq_key: boolean;
    has_elevenlabs_key: boolean;
    has_pexels_key: boolean;
    has_local_image_url: boolean;
    has_local_video_url: boolean;
    has_local_tts_url: boolean;
    has_local_whisper: boolean;
}

interface VoiceOption {
    id: string;
    label: string;
    engine: string;
    lang: string;
}

interface CreatorScriptSummary {
    id: string;
    title: string;
    platform: string;
    hook_0_3s: string;
    body_script: string;
    b_roll_notes: string;
    cta: string;
}

interface VideoProgressEvt {
    project_id: string;
    stage: string;
    progress: number;
    message: string;
    scene_id?: string | null;
    done: boolean;
    error?: string | null;
}

interface RedactedConfig {
    default_preset: string;
    default_voice_engine: string;
    default_voice_id: string;
    default_visual_engine: string;
    default_hook_video_engine: string;
    default_subtitle_engine: string;
    default_subtitle_style: string;
    default_bgm_style?: string;
    has_pexels_key: boolean;
    local_image_url?: string | null;
    local_image_mode?: string | null;
    local_video_url?: string | null;
    local_tts_url?: string | null;
    local_stt_url?: string | null;
}

interface VideoStudioStatus {
    runtime: RuntimeStatus;
    providers: ProviderReadiness;
    config: RedactedConfig;
    voices: VoiceOption[];
    creator_scripts: CreatorScriptSummary[];
    active_install?: VideoProgressEvt | null;
    advisor?: VideoCapabilityAdvisor | null;
}

interface VideoStudioPanelProps {
    tenantId: string | null;
    sessionKey: string | null;
    pendingCreatorScriptId?: string | null;
    onConsumePendingScript?: () => void;
    targetProjectId?: string | null;
    onConsumeTargetProject?: () => void;
    onScheduleResult?: (projectTitle: string, platform: string) => void;
    onAskAegis?: (prompt: string) => void;
}

const PRESET_OPTIONS = [
    {
        id: 'auto',
        label: 'Automático ★ (Recomendado)',
        desc: 'Elige la mejor calidad disponible según tus claves y se adapta sin fallar si falta cuota/GPU',
    },
    {
        id: 'free_builtin',
        label: 'Gratis / IA Sin Key ★',
        desc: 'Edge Voz HD + Fondos IA Pollinations Flux + HUD Animado + Música',
    },
    {
        id: 'gemini_studio',
        label: 'Gemini Studio IA',
        desc: 'Voz Estudio Gemini + Arte Gemini Imagen + Transiciones + Música',
    },
    {
        id: 'hybrid_pro',
        label: 'Híbrido (Hook Veo + Imagen)',
        desc: 'Escena 1 con Veo (o Imagen) + Gemini Imagen + Groq Whisper',
    },
    {
        id: 'cloud_pro',
        label: 'Cloud Pro',
        desc: 'ElevenLabs + Hook Veo + Gemini Imagen + OpenAI Whisper',
    },
    {
        id: 'local_ai',
        label: 'IA Local (GPU)',
        desc: 'ComfyUI / A1111 + TTS Local + Whisper.cpp offline',
    },
    {
        id: 'custom',
        label: 'Personalizado',
        desc: 'Combiná libremente cualquier motor por componente o escena',
    },
];

// Fetches protected /api/video/media/* using Citadel auth headers cleanly without leaking credentials in URLs.
const useAuthenticatedBlobUrl = (
    url: string | null,
    tenantId: string | null,
    sessionKey: string | null,
    reloadToken: string | number,
) => {
    const [blobUrl, setBlobUrl] = useState<string | null>(null);

    useEffect(() => {
        if (!url || !tenantId || !sessionKey) {
            setBlobUrl(null);
            return;
        }
        let cancelled = false;
        let objectUrl: string | null = null;

        fetch(url, {
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        })
            .then((res) => (res.ok ? res.blob() : null))
            .then((blob) => {
                if (!cancelled && blob) {
                    objectUrl = URL.createObjectURL(blob);
                    setBlobUrl(objectUrl);
                }
            })
            .catch(() => {
                if (!cancelled) setBlobUrl(null);
            });

        return () => {
            cancelled = true;
            if (objectUrl) {
                URL.revokeObjectURL(objectUrl);
            }
        };
    }, [url, tenantId, sessionKey, reloadToken]);

    return blobUrl;
};

const SceneVisualPreview: React.FC<{
    projectId: string;
    scene: VideoScene;
    tenantId: string | null;
    sessionKey: string | null;
    updatedAt: string;
}> = ({ projectId, scene, tenantId, sessionKey, updatedAt }) => {
    const visualPath = scene.visual_file
        ? `/api/video/media/${encodeURIComponent(projectId)}/${encodeURIComponent(scene.visual_file)}`
        : null;
    const audioPath = scene.audio_file
        ? `/api/video/media/${encodeURIComponent(projectId)}/${encodeURIComponent(scene.audio_file)}`
        : null;

    const visualUrl = useAuthenticatedBlobUrl(visualPath, tenantId, sessionKey, `${scene.visual_file}-${updatedAt}`);
    const audioUrl = useAuthenticatedBlobUrl(audioPath, tenantId, sessionKey, `${scene.audio_file}-${updatedAt}`);

    return (
        <div className="space-y-2">
            {visualUrl ? (
                scene.visual_kind === 'video' ? (
                    <video
                        src={visualUrl}
                        controls
                        muted
                        loop
                        className="w-full max-h-44 rounded-lg bg-black object-contain border border-white/10"
                    />
                ) : (
                    <img
                        src={visualUrl}
                        alt={scene.title}
                        className="w-full max-h-44 rounded-lg bg-black/70 object-contain border border-white/10"
                    />
                )
            ) : (
                <div className="h-28 rounded-lg border border-dashed border-white/15 bg-black/40 flex flex-col items-center justify-center text-[11px] font-mono text-white/35">
                    <ImageIcon className="w-5 h-5 mb-1 opacity-50" />
                    Visual pendiente
                </div>
            )}
            {audioUrl && (
                <audio src={audioUrl} controls className="w-full h-8 opacity-90" />
            )}
        </div>
    );
};

const FinalVideoPlayer: React.FC<{
    project: VideoProject;
    tenantId: string | null;
    sessionKey: string | null;
}> = ({ project, tenantId, sessionKey }) => {
    const mp4Url = project.output_mp4
        ? `/api/video/media/${encodeURIComponent(project.id)}/${encodeURIComponent(project.output_mp4)}`
        : null;
    const srtUrl = project.output_srt
        ? `/api/video/media/${encodeURIComponent(project.id)}/${encodeURIComponent(project.output_srt)}`
        : null;

    const mp4Blob = useAuthenticatedBlobUrl(mp4Url, tenantId, sessionKey, `${project.output_mp4}-${project.updated_at}`);
    const srtBlob = useAuthenticatedBlobUrl(srtUrl, tenantId, sessionKey, `${project.output_srt}-${project.updated_at}`);

    if (!project.output_mp4) return null;

    return (
        <div className="p-4 rounded-2xl bg-emerald-500/10 border border-emerald-400/30 space-y-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                    <span className="text-xs font-mono font-bold text-emerald-200 uppercase tracking-wider">
                        MP4 Exportado y Listo
                    </span>
                </div>
                <div className="flex items-center gap-2">
                    {mp4Blob && (
                        <a
                            href={mp4Blob}
                            download={`${project.title.replace(/[^a-zA-Z0-9_-]+/g, '_') || project.id}.mp4`}
                            className="px-3 py-1.5 rounded-lg bg-emerald-400 text-black font-mono text-xs font-bold flex items-center gap-1.5 hover:opacity-90"
                        >
                            <Download className="w-3.5 h-3.5" />
                            Descargar MP4
                        </a>
                    )}
                    {srtBlob && (
                        <a
                            href={srtBlob}
                            download={`${project.id}.srt`}
                            className="px-3 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-white/15 text-white font-mono text-xs flex items-center gap-1.5"
                        >
                            <Subtitles className="w-3.5 h-3.5" />
                            Descargar .SRT
                        </a>
                    )}
                </div>
            </div>
            {mp4Blob ? (
                <video
                    key={mp4Blob}
                    src={mp4Blob}
                    controls
                    className="w-full max-h-[420px] rounded-xl bg-black border border-white/10"
                />
            ) : (
                <div className="h-32 flex items-center justify-center text-xs font-mono text-white/50 gap-2">
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Cargando reproductor MP4...
                </div>
            )}
        </div>
    );
};

const FallbacksAppliedBanner: React.FC<{ fallbacks?: string[] }> = ({ fallbacks }) => {
    if (!fallbacks || fallbacks.length === 0) return null;
    return (
        <div className="p-3 rounded-xl bg-amber-500/10 border border-amber-400/35 text-xs space-y-1">
            <p className="font-mono font-bold text-amber-200 flex items-center gap-1.5">
                <AlertTriangle className="w-3.5 h-3.5 text-amber-300 shrink-0" />
                Aegis adaptó motores automáticamente para completar tu video:
            </p>
            <ul className="list-disc pl-5 text-amber-100/80 space-y-0.5 text-[11px]">
                {fallbacks.map((fb, idx) => (
                    <li key={idx}>{fb}</li>
                ))}
            </ul>
        </div>
    );
};

const VideoCapabilityAdvisorCard: React.FC<{ advisor: VideoCapabilityAdvisor }> = ({ advisor }) => {
    const pc = advisor.pc_specs;
    return (
        <div className="p-4 rounded-2xl bg-[#0c121b] border border-aegis-cyan/30 space-y-3 text-xs">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <h4 className="font-bold text-white flex items-center gap-2">
                    <Cpu className="w-4 h-4 text-aegis-cyan" />
                    Diagnóstico de tu PC y Recomendaciones (IA Local vs Planes API)
                </h4>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/10 text-aegis-cyan">
                    {pc.cpu_cores} núcleos · {pc.total_ram_gb.toFixed(1)} GB RAM · {pc.has_nvidia_cuda ? `${pc.gpu_name} (${(pc.dedicated_vram_mb / 1024).toFixed(1)} GB VRAM)` : pc.gpu_name}
                </span>
            </div>

            <p className="text-[11px] text-white/75 leading-relaxed bg-black/40 p-2.5 rounded-xl border border-white/10">
                {advisor.pc_verdict}
            </p>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                <div className="space-y-1.5">
                    <p className="text-[10px] font-mono uppercase text-purple-300 font-bold">
                        Modelos IA Locales (Viabilidad en esta PC)
                    </p>
                    <div className="space-y-1.5">
                        {advisor.local_models.map((lm, i) => (
                            <div key={i} className="p-2 rounded-xl bg-black/45 border border-white/10 space-y-0.5">
                                <div className="flex items-center justify-between gap-2">
                                    <span className="font-mono font-bold text-white/90 text-[11px]">{lm.model_name}</span>
                                    <span
                                        className={`text-[10px] font-mono px-1.5 py-0.5 rounded ${
                                            lm.compatible_with_this_pc
                                                ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-400/30'
                                                : 'bg-amber-500/15 text-amber-200 border border-amber-400/25'
                                        }`}
                                    >
                                        {lm.compatible_with_this_pc ? '✓ Viable' : `Req. ${lm.min_vram_gb > 0 ? `${lm.min_vram_gb}GB VRAM` : `${lm.min_ram_gb}GB RAM`}`}
                                    </span>
                                </div>
                                <p className="text-[10px] text-white/55">{lm.category} · {lm.status_note}</p>
                            </div>
                        ))}
                    </div>
                </div>

                <div className="space-y-1.5">
                    <p className="text-[10px] font-mono uppercase text-aegis-cyan font-bold">
                        Planes y APIs Cloud Recomendadas
                    </p>
                    <div className="space-y-1.5">
                        {advisor.api_plans.map((ap, i) => (
                            <div key={i} className="p-2 rounded-xl bg-black/45 border border-white/10 space-y-0.5">
                                <div className="flex items-center justify-between gap-2">
                                    <span className="font-mono font-bold text-aegis-cyan text-[11px]">{ap.provider}</span>
                                    <span className="text-[10px] font-mono text-emerald-300">{ap.estimated_cost}</span>
                                </div>
                                <p className="text-[10px] text-white/80 font-semibold">{ap.recommended_plan}</p>
                                <p className="text-[10px] text-white/55">{ap.what_it_unlocks}</p>
                                <p className="text-[10px] font-mono text-white/40">Estado: {ap.current_status}</p>
                            </div>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
};

export const ChatVideoResultCard: React.FC<{
    projectId: string;
    tenantId: string | null;
    sessionKey: string | null;
    onOpenInStudio?: (projectId: string) => void;
}> = ({ projectId, tenantId, sessionKey, onOpenInStudio }) => {
    const [project, setProject] = useState<VideoProject | null>(null);
    const [loadingMeta, setLoadingMeta] = useState(true);
    const [metaErr, setMetaErr] = useState<string | null>(null);
    const [showAdvisor, setShowAdvisor] = useState(false);
    const [advisor, setAdvisor] = useState<VideoCapabilityAdvisor | null>(null);
    const [loadingAdvisor, setLoadingAdvisor] = useState(false);

    const toggleAdvisor = () => {
        const next = !showAdvisor;
        setShowAdvisor(next);
        if (next && !advisor && tenantId && sessionKey) {
            setLoadingAdvisor(true);
            fetch('/api/video/status', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            })
                .then((r) => (r.ok ? r.json() : null))
                .then((data) => {
                    if (data?.status?.advisor) setAdvisor(data.status.advisor);
                })
                .catch(() => {})
                .finally(() => setLoadingAdvisor(false));
        }
    };

    useEffect(() => {
        if (!tenantId || !sessionKey || !projectId) {
            setLoadingMeta(false);
            return;
        }
        let cancelled = false;
        setLoadingMeta(true);
        fetch('/api/video/projects', {
            headers: {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
        })
            .then((r) => (r.ok ? r.json() : Promise.reject(new Error(`HTTP ${r.status}`))))
            .then((data) => {
                if (cancelled) return;
                const list: VideoProject[] = Array.isArray(data?.projects) ? data.projects : [];
                const found = list.find((p) => p.id === projectId) ?? null;
                setProject(found);
                setMetaErr(null);
            })
            .catch((e) => {
                if (!cancelled) setMetaErr(e instanceof Error ? e.message : String(e));
            })
            .finally(() => {
                if (!cancelled) setLoadingMeta(false);
            });
        return () => {
            cancelled = true;
        };
    }, [projectId, tenantId, sessionKey]);

    const mp4Path = project?.output_mp4
        ? `/api/video/media/${encodeURIComponent(project.id)}/${encodeURIComponent(project.output_mp4)}`
        : null;
    const srtPath = project?.output_srt
        ? `/api/video/media/${encodeURIComponent(project.id)}/${encodeURIComponent(project.output_srt)}`
        : null;

    const mp4Blob = useAuthenticatedBlobUrl(
        mp4Path,
        tenantId,
        sessionKey,
        project ? `${project.output_mp4}-${project.updated_at}` : 'none',
    );
    const srtBlob = useAuthenticatedBlobUrl(
        srtPath,
        tenantId,
        sessionKey,
        project ? `${project.output_srt}-${project.updated_at}` : 'none',
    );

    if (loadingMeta) {
        return (
            <div className="mt-2 p-3 rounded-xl bg-black/50 border border-purple-400/25 flex items-center gap-2 text-xs font-mono text-purple-200">
                <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan" />
                Cargando video {projectId}...
            </div>
        );
    }

    if (metaErr || !project || !project.output_mp4) {
        return (
            <div className="mt-2 p-3 rounded-xl bg-black/40 border border-white/15 flex items-center justify-between gap-2 text-xs font-mono text-white/60">
                <span>Proyecto {projectId} {metaErr ? `(${metaErr})` : 'sin MP4 exportado aún'}</span>
                {onOpenInStudio && (
                    <button
                        type="button"
                        onClick={() => onOpenInStudio(projectId)}
                        className="px-2.5 py-1 rounded-lg bg-purple-500/25 border border-purple-400/35 text-purple-200 hover:bg-purple-500/35"
                    >
                        Abrir en Video Studio
                    </button>
                )}
            </div>
        );
    }

    const safeFileName = `${project.title.replace(/[^a-zA-Z0-9_-]+/g, '_') || project.id}.mp4`;

    return (
        <div className="mt-3 p-3.5 rounded-2xl bg-black/60 border border-purple-400/35 space-y-2.5 shadow-lg">
            <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                    <div className="p-1.5 rounded-lg bg-purple-500/20 border border-purple-400/30 text-purple-300 shrink-0">
                        <Film className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                        <p className="text-xs font-bold text-white truncate">{project.title}</p>
                        <p className="text-[10px] font-mono text-white/50">
                            {project.id} · {project.aspect_ratio} · {project.scenes.length} escena(s)
                        </p>
                    </div>
                </div>

                <div className="flex flex-wrap items-center gap-1.5">
                    {mp4Blob && (
                        <a
                            href={mp4Blob}
                            download={safeFileName}
                            className="px-3 py-1.5 rounded-lg bg-emerald-400 text-black font-mono text-[11px] font-bold flex items-center gap-1.5 hover:opacity-90"
                        >
                            <Download className="w-3.5 h-3.5" />
                            Descargar MP4
                        </a>
                    )}
                    {srtBlob && (
                        <a
                            href={srtBlob}
                            download={`${project.id}.srt`}
                            className="px-2.5 py-1.5 rounded-lg bg-white/10 hover:bg-white/15 border border-white/15 text-white font-mono text-[11px] flex items-center gap-1"
                        >
                            <Subtitles className="w-3 h-3" />
                            .SRT
                        </a>
                    )}
                    {onOpenInStudio && (
                        <button
                            type="button"
                            onClick={() => onOpenInStudio(project.id)}
                            className="px-2.5 py-1.5 rounded-lg bg-purple-500/25 hover:bg-purple-500/35 border border-purple-400/40 text-purple-100 font-mono text-[11px] flex items-center gap-1"
                        >
                            <Wand2 className="w-3 h-3" />
                            Editar en Studio
                        </button>
                    )}
                </div>
            </div>

            {mp4Blob ? (
                <video
                    key={mp4Blob}
                    src={mp4Blob}
                    controls
                    className="w-full max-h-80 rounded-xl bg-black border border-white/10"
                />
            ) : (
                <div className="h-24 flex items-center justify-center text-xs font-mono text-white/50 gap-2">
                    <Loader2 className="w-4 h-4 animate-spin text-aegis-cyan" />
                    Cargando reproductor MP4...
                </div>
            )}

            <FallbacksAppliedBanner fallbacks={project.fallbacks_applied} />

            <div className="pt-1 border-t border-white/10">
                <button
                    type="button"
                    onClick={toggleAdvisor}
                    className="text-[11px] font-mono text-aegis-cyan hover:text-white flex items-center gap-1.5"
                >
                    <Cpu className="w-3.5 h-3.5" />
                    {showAdvisor ? 'Ocultar diagnóstico de PC y APIs' : 'Cómo mejorar calidad (Diagnóstico de tu PC y APIs)'}
                    {showAdvisor ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                </button>
                {showAdvisor && (
                    <div className="mt-2.5">
                        {loadingAdvisor ? (
                            <div className="p-3 rounded-xl bg-black/40 text-xs font-mono text-white/55 flex items-center gap-2">
                                <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan" />
                                Analizando hardware de tu PC y proveedores...
                            </div>
                        ) : advisor ? (
                            <VideoCapabilityAdvisorCard advisor={advisor} />
                        ) : (
                            <p className="text-[11px] text-white/50 font-mono">
                                Abrí Video Studio para ver el diagnóstico completo de hardware y planes API.
                            </p>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
};

export const VideoStudioPanel: React.FC<VideoStudioPanelProps> = ({
    tenantId,
    sessionKey,
    pendingCreatorScriptId,
    onConsumePendingScript,
    targetProjectId,
    onConsumeTargetProject,
    onScheduleResult,
    onAskAegis,
}) => {
    const [studioStatus, setStudioStatus] = useState<VideoStudioStatus | null>(null);
    const [projects, setProjects] = useState<VideoProject[]>([]);
    const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [busyAction, setBusyAction] = useState<string | null>(null);
    const [errorBanner, setErrorBanner] = useState<string | null>(null);
    const [progressEvt, setProgressEvt] = useState<VideoProgressEvt | null>(null);
    const [showConfigModal, setShowConfigModal] = useState(false);
    const [showCreateForm, setShowCreateForm] = useState(false);
    const [showPcAdvisor, setShowPcAdvisor] = useState(false);
    const [showCreateAdvanced, setShowCreateAdvanced] = useState(false);
    const [showProjectAdvanced, setShowProjectAdvanced] = useState(false);

    // Create form states
    const [newTitle, setNewTitle] = useState('');
    const [newPrompt, setNewPrompt] = useState('');
    const [newScriptId, setNewScriptId] = useState('');
    const [newAspect, setNewAspect] = useState('9:16');
    const [newPreset, setNewPreset] = useState('auto');
    const [useAiStoryboard, setUseAiStoryboard] = useState(true);

    // Engine Settings Drawer state
    const [cfgPexelsKey, setCfgPexelsKey] = useState('');
    const [cfgElevenKey, setCfgElevenKey] = useState('');
    const [cfgLocalImageUrl, setCfgLocalImageUrl] = useState('');
    const [cfgLocalImageMode, setCfgLocalImageMode] = useState('auto');
    const [cfgLocalVideoUrl, setCfgLocalVideoUrl] = useState('');
    const [cfgLocalTtsUrl, setCfgLocalTtsUrl] = useState('');

    // New scene inline state
    const [addSceneTitle, setAddSceneTitle] = useState('');
    const [addSceneNarration, setAddSceneNarration] = useState('');
    const [addSceneVisualPrompt, setAddSceneVisualPrompt] = useState('');

    const fileInputRefs = useRef<Record<string, HTMLInputElement | null>>({});

    const apiFetch = useCallback(
        async (path: string, init: RequestInit = {}) => {
            if (!tenantId || !sessionKey) throw new Error('Sesión no iniciada');
            const headers: Record<string, string> = {
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
                ...(init.body && !(init.body instanceof FormData)
                    ? { 'Content-Type': 'application/json' }
                    : {}),
            };
            const res = await fetch(path, { ...init, headers: { ...headers, ...(init.headers as Record<string, string> | undefined) } });
            const json = await res.json().catch(() => ({}));
            if (!res.ok) {
                throw new Error(json?.error || json?.message || `Error HTTP ${res.status}`);
            }
            return json;
        },
        [tenantId, sessionKey],
    );

    const loadAll = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoading(true);
        try {
            const [stRes, projRes] = await Promise.all([
                apiFetch('/api/video/status'),
                apiFetch('/api/video/projects'),
            ]);
            if (stRes?.status) {
                setStudioStatus(stRes.status);
                setCfgLocalImageUrl(stRes.status.config?.local_image_url || '');
                setCfgLocalImageMode(stRes.status.config?.local_image_mode || 'auto');
                setCfgLocalVideoUrl(stRes.status.config?.local_video_url || '');
                setCfgLocalTtsUrl(stRes.status.config?.local_tts_url || '');
                if (stRes.status.active_install && !stRes.status.active_install.done) {
                    setProgressEvt(stRes.status.active_install);
                }
            }
            if (Array.isArray(projRes?.projects)) {
                setProjects(projRes.projects);
                setSelectedProjectId((prev) => {
                    if (prev && projRes.projects.some((p: VideoProject) => p.id === prev)) return prev;
                    return projRes.projects[0]?.id ?? null;
                });
            }
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey, apiFetch]);

    useEffect(() => {
        void loadAll();
    }, [loadAll]);

    // Handle 1-click Creator Studio -> Video Studio bridge script handoff
    useEffect(() => {
        if (pendingCreatorScriptId && studioStatus) {
            const found = studioStatus.creator_scripts.find((s) => s.id === pendingCreatorScriptId);
            setNewScriptId(pendingCreatorScriptId);
            if (found) {
                setNewTitle(found.title);
                setNewAspect(found.platform === 'youtube' ? '16:9' : '9:16');
                setNewPrompt('');
            }
            setShowCreateForm(true);
            onConsumePendingScript?.();
        }
    }, [pendingCreatorScriptId, studioStatus, onConsumePendingScript]);

    useEffect(() => {
        if (targetProjectId && projects.some((p) => p.id === targetProjectId)) {
            setSelectedProjectId(targetProjectId);
            onConsumeTargetProject?.();
        }
    }, [targetProjectId, projects, onConsumeTargetProject]);

    // Live WebSocket updates
    useEffect(() => {
        const onProgress = (e: Event) => {
            const detail = (e as CustomEvent<VideoProgressEvt>).detail;
            if (!detail) return;
            setProgressEvt(detail);
            if (detail.error) {
                setErrorBanner(detail.error);
            }
            if (detail.done) {
                void loadAll();
            }
        };
        const onModulesUpdated = () => {
            void loadAll();
        };
        window.addEventListener('aegis-video-progress', onProgress);
        window.addEventListener('aegis-modules-updated', onModulesUpdated);
        return () => {
            window.removeEventListener('aegis-video-progress', onProgress);
            window.removeEventListener('aegis-modules-updated', onModulesUpdated);
        };
    }, [loadAll]);

    const selectedProject = projects.find((p) => p.id === selectedProjectId) ?? null;

    const handleInstallRuntime = async () => {
        setErrorBanner(null);
        setBusyAction('install');
        try {
            const res = await apiFetch('/api/video/dependencies/install', {
                method: 'POST',
                body: JSON.stringify({ force: false }),
            });
            setProgressEvt({
                project_id: '_runtime',
                stage: 'install_runtime',
                progress: 0.05,
                message: res.message || 'Instalando motor multimedia...',
                done: false,
            });
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleCreateProject = async (autoRender = true) => {
        setErrorBanner(null);
        setBusyAction('create');
        try {
            const res = await apiFetch('/api/video/projects', {
                method: 'POST',
                body: JSON.stringify({
                    title: newTitle.trim() || 'Nuevo Video IA',
                    prompt: newPrompt.trim(),
                    creator_script_id: newScriptId || null,
                    aspect_ratio: newAspect,
                    preset: newPreset,
                    use_ai_storyboard: useAiStoryboard && !newScriptId,
                    auto_render: autoRender,
                }),
            });
            if (res?.project?.id) {
                setSelectedProjectId(res.project.id);
                setShowCreateForm(false);
                setNewTitle('');
                setNewPrompt('');
                setNewScriptId('');
                if (autoRender) {
                    setProgressEvt({
                        project_id: res.project.id,
                        stage: 'render_mp4',
                        progress: 0.05,
                        message: 'Produciendo escenas, locución y exportación MP4...',
                        done: false,
                    });
                }
            }
            await loadAll();
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleUpdateProjectEngines = async (patch: Record<string, string>) => {
        if (!selectedProject) return;
        setErrorBanner(null);
        setBusyAction('engines');
        try {
            const res = await apiFetch(`/api/video/projects/${selectedProject.id}/engines`, {
                method: 'POST',
                body: JSON.stringify(patch),
            });
            if (res?.project) {
                setProjects((prev) => prev.map((p) => (p.id === res.project.id ? res.project : p)));
            }
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleSaveConfig = async () => {
        setErrorBanner(null);
        setBusyAction('config');
        try {
            await apiFetch('/api/video/config', {
                method: 'POST',
                body: JSON.stringify({
                    ...(cfgPexelsKey.trim() ? { pexels_api_key: cfgPexelsKey.trim() } : {}),
                    ...(cfgElevenKey.trim() ? { elevenlabs_api_key: cfgElevenKey.trim() } : {}),
                    local_image_url: cfgLocalImageUrl.trim(),
                    local_image_mode: cfgLocalImageMode,
                    local_video_url: cfgLocalVideoUrl.trim(),
                    local_tts_url: cfgLocalTtsUrl.trim(),
                }),
            });
            setCfgPexelsKey('');
            setCfgElevenKey('');
            setShowConfigModal(false);
            await loadAll();
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleGenerateAssets = async (sceneId?: string) => {
        if (!selectedProject) return;
        setErrorBanner(null);
        setBusyAction(sceneId ? `gen-${sceneId}` : 'gen-all');
        try {
            await apiFetch(`/api/video/projects/${selectedProject.id}/generate-assets`, {
                method: 'POST',
                body: JSON.stringify({ scene_id: sceneId ?? null }),
            });
            setProgressEvt({
                project_id: selectedProject.id,
                stage: 'generate_assets',
                progress: 0.05,
                message: sceneId ? 'Regenerando escena...' : 'Generando voz y visuales...',
                done: false,
            });
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleRenderMp4 = async () => {
        if (!selectedProject) return;
        setErrorBanner(null);
        setBusyAction('render');
        try {
            await apiFetch(`/api/video/projects/${selectedProject.id}/render`, {
                method: 'POST',
                body: JSON.stringify({ auto_generate_missing: true }),
            });
            setProgressEvt({
                project_id: selectedProject.id,
                stage: 'render_mp4',
                progress: 0.05,
                message: 'Preparando FFmpeg y exportación MP4...',
                done: false,
            });
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleUploadSceneMedia = async (sceneId: string, file: File) => {
        if (!selectedProject) return;
        setErrorBanner(null);
        setBusyAction(`up-${sceneId}`);
        try {
            const fd = new FormData();
            fd.append('file', file);
            await apiFetch(`/api/video/projects/${selectedProject.id}/scenes/${sceneId}/media`, {
                method: 'POST',
                body: fd,
            });
            await loadAll();
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const handleManageScene = async (payload: Record<string, unknown>) => {
        if (!selectedProject) return;
        setErrorBanner(null);
        setBusyAction('scene');
        try {
            const res = await apiFetch(`/api/video/projects/${selectedProject.id}/scenes`, {
                method: 'POST',
                body: JSON.stringify(payload),
            });
            if (res?.project) {
                setProjects((prev) => prev.map((p) => (p.id === res.project.id ? res.project : p)));
            }
        } catch (err) {
            setErrorBanner(err instanceof Error ? err.message : String(err));
        } finally {
            setBusyAction(null);
        }
    };

    const readiness = studioStatus?.providers;
    const runtime = studioStatus?.runtime;

    const getEngineBadge = (kind: string): { ok: boolean; hint: string } => {
        if (!readiness) return { ok: true, hint: '' };
        switch (kind) {
            case 'edge_tts':
                return { ok: Boolean(runtime?.edge_tts_ready), hint: runtime?.edge_tts_ready ? 'Listo (Gratis)' : 'Se auto-instala' };
            case 'kinetic_card':
                return { ok: Boolean(runtime?.pillow_ready), hint: runtime?.pillow_ready ? 'Listo (Gratis)' : 'Se auto-instala' };
            case 'pollinations_ai':
                return { ok: true, hint: 'Listo (IA Gratis sin Key)' };
            case 'scene_auto':
            case 'disabled':
            case 'none':
            case 'inherit':
                return { ok: true, hint: 'Listo' };
            case 'openai_tts':
            case 'openai_image':
            case 'openai_whisper':
                return { ok: readiness.has_openai_key, hint: readiness.has_openai_key ? 'Key OK' : 'Falta key OpenAI' };
            case 'gemini_tts':
            case 'gemini_imagen':
            case 'google_veo':
                return { ok: readiness.has_gemini_key, hint: readiness.has_gemini_key ? 'Key Gemini OK' : 'Falta key Gemini' };
            case 'elevenlabs':
                return { ok: readiness.has_elevenlabs_key, hint: readiness.has_elevenlabs_key ? 'Key OK' : 'Falta key ElevenLabs' };
            case 'pexels_stock':
            case 'pixabay_stock':
                return { ok: readiness.has_pexels_key, hint: readiness.has_pexels_key ? 'Key Pixabay/Pexels OK' : 'Poner key Pixabay gratis en Claves' };
            case 'groq_whisper':
                return { ok: readiness.has_groq_key, hint: readiness.has_groq_key ? 'Key Groq OK' : 'Falta key Groq (usa Auto)' };
            case 'local_tts':
                return { ok: readiness.has_local_tts_url, hint: readiness.has_local_tts_url ? 'URL Local OK' : 'Configurar URL local' };
            case 'local_ai':
                return {
                    ok: readiness.has_local_image_url || readiness.has_local_video_url,
                    hint: readiness.has_local_image_url ? 'URL Local OK' : 'Configurar ComfyUI/A1111',
                };
            case 'whisper_local':
                return { ok: readiness.has_local_whisper, hint: readiness.has_local_whisper ? 'Modelo local OK' : 'Sin modelo .bin (usa Auto)' };
            default:
                return { ok: true, hint: '' };
        }
    };

    const filteredVoices = (studioStatus?.voices || []).filter((v) => {
        if (!selectedProject) return v.engine === 'edge_tts';
        if (selectedProject.voice_engine === 'edge_tts') return v.engine === 'edge_tts';
        if (selectedProject.voice_engine === 'gemini_tts') return v.engine === 'gemini_tts';
        if (selectedProject.voice_engine === 'openai_tts') return v.engine === 'openai_tts';
        if (selectedProject.voice_engine === 'elevenlabs') return v.engine === 'elevenlabs';
        return true;
    });

    return (
        <div className="space-y-5">
            {/* Top Status & Auto-Install Runtime Banner */}
            <div className="p-4 rounded-2xl bg-white/[0.03] border border-white/10 flex flex-col lg:flex-row lg:items-center justify-between gap-4">
                <div className="space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                        <span className="px-2.5 py-0.5 rounded-full text-[10px] font-mono bg-purple-500/20 text-purple-200 border border-purple-400/30 uppercase">
                            Motor Multimedia
                        </span>
                        {runtime?.ready_for_free_video ? (
                            <span className="flex items-center gap-1.5 text-xs font-mono text-emerald-300">
                                <CheckCircle2 className="w-3.5 h-3.5" />
                                FFmpeg + Edge-TTS + Gráficos HD Listos ({runtime.ffmpeg_source})
                            </span>
                        ) : (
                            <span className="flex items-center gap-1.5 text-xs font-mono text-amber-300">
                                <Wrench className="w-3.5 h-3.5" />
                                Motor portable pendiente (se instala automáticamente al usar o con 1 clic)
                            </span>
                        )}
                    </div>
                    <p className="text-[11px] text-white/55">
                        <strong>FFmpeg</strong> edita, une clips y quema subtítulos a MP4 · <strong>Edge-TTS</strong> genera voces neuronales gratis · también podés combinar <strong>Gemini Veo/Imagen, OpenAI, ElevenLabs, Pexels o IA Local</strong>.
                    </p>
                </div>

                <div className="flex flex-wrap items-center gap-2 shrink-0">
                    {!runtime?.ready_for_free_video && (
                        <button
                            disabled={busyAction === 'install'}
                            onClick={() => void handleInstallRuntime()}
                            className="px-3.5 py-2 rounded-xl bg-amber-400 hover:bg-amber-300 text-black font-mono text-xs font-bold flex items-center gap-1.5"
                        >
                            {busyAction === 'install' ? (
                                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                            ) : (
                                <Wrench className="w-3.5 h-3.5" />
                            )}
                            Instalar Motor Automático
                        </button>
                    )}
                    <button
                        onClick={() => setShowPcAdvisor((v) => !v)}
                        className={`px-3.5 py-2 rounded-xl border text-xs font-mono flex items-center gap-1.5 ${
                            showPcAdvisor
                                ? 'bg-aegis-cyan/15 border-aegis-cyan/45 text-aegis-cyan'
                                : 'bg-white/5 hover:bg-white/10 border-white/15 text-white'
                        }`}
                    >
                        <Cpu className="w-3.5 h-3.5 text-aegis-cyan" />
                        Diagnóstico PC & APIs
                    </button>
                    <button
                        onClick={() => setShowConfigModal((v) => !v)}
                        className="px-3.5 py-2 rounded-xl bg-white/5 hover:bg-white/10 border border-white/15 text-xs font-mono text-white flex items-center gap-1.5"
                    >
                        <Settings2 className="w-3.5 h-3.5 text-aegis-cyan" />
                        Claves & IA Local
                    </button>
                    <button
                        onClick={() => setShowCreateForm((v) => !v)}
                        className="px-3.5 py-2 rounded-xl bg-gradient-to-r from-purple-500 to-aegis-cyan text-black font-mono text-xs font-bold flex items-center gap-1.5 hover:opacity-90"
                    >
                        <Plus className="w-3.5 h-3.5" />
                        Nuevo Video IA
                    </button>
                </div>
            </div>

            {showPcAdvisor && studioStatus?.advisor && (
                <VideoCapabilityAdvisorCard advisor={studioStatus.advisor} />
            )}

            {errorBanner && (
                <div className="p-3 rounded-xl bg-red-500/15 border border-red-400/30 flex items-center justify-between text-xs font-mono text-red-200">
                    <span className="flex items-center gap-2">
                        <AlertTriangle className="w-4 h-4 text-red-400 shrink-0" />
                        {errorBanner}
                    </span>
                    <button onClick={() => setErrorBanner(null)} className="text-white/60 hover:text-white">
                        ✕
                    </button>
                </div>
            )}

            {/* Live WebSocket Progress Bar */}
            {progressEvt && !progressEvt.done && (
                <div className="p-3.5 rounded-xl bg-aegis-cyan/10 border border-aegis-cyan/35 space-y-2">
                    <div className="flex items-center justify-between text-xs font-mono text-aegis-cyan">
                        <span className="flex items-center gap-2">
                            <Loader2 className="w-3.5 h-3.5 animate-spin" />
                            {progressEvt.message}
                        </span>
                        <span>{Math.round(progressEvt.progress * 100)}%</span>
                    </div>
                    <div className="w-full h-2 rounded-full bg-black/50 overflow-hidden">
                        <div
                            className="h-full bg-gradient-to-r from-purple-400 to-aegis-cyan transition-all duration-300"
                            style={{ width: `${Math.max(4, Math.round(progressEvt.progress * 100))}%` }}
                        />
                    </div>
                </div>
            )}

            {/* Settings Drawer: Pexels, ElevenLabs, and Local GPU AI endpoints */}
            {showConfigModal && (
                <div className="p-5 rounded-2xl bg-[#0e131b] border border-aegis-cyan/30 space-y-4">
                    <div className="flex items-center justify-between">
                        <div>
                            <h4 className="text-sm font-bold text-white flex items-center gap-2">
                                <Cpu className="w-4 h-4 text-aegis-cyan" />
                                Motores Extra y Endpoints de IA Local (GPU / Offline)
                            </h4>
                            <p className="text-[11px] text-white/50 mt-0.5">
                                Gemini, OpenAI y Groq se toman automáticamente de Settings → Proveedores (y Pollinations Flux IA funciona gratis sin key). Acá podés sumar Pixabay (videos MP4 gratis en pixabay.com/api/docs), ElevenLabs o tus servidores locales (ComfyUI, Automatic1111, Piper/Kokoro).
                            </p>
                        </div>
                        <button
                            onClick={() => setShowConfigModal(false)}
                            className="text-xs font-mono text-white/50 hover:text-white"
                        >
                            Cerrar
                        </button>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div className="space-y-2 p-3.5 rounded-xl bg-black/40 border border-white/10">
                            <p className="text-xs font-mono uppercase text-purple-300 font-bold">
                                Claves Multimedia Opcionales
                            </p>
                            <div>
                                <label className="text-[11px] font-mono text-white/60 block mb-1">
                                    Pixabay / Pexels API Key (Stock Videos/Fotos HD Gratis) {studioStatus?.config.has_pexels_key ? '· ✓ Guardada' : ''}
                                </label>
                                <input
                                    type="password"
                                    value={cfgPexelsKey}
                                    onChange={(e) => setCfgPexelsKey(e.target.value)}
                                    placeholder={studioStatus?.config.has_pexels_key ? '•••••••• (dejar vacío para mantener)' : 'Pegar API key gratuita de pixabay.com/api/docs (ej: 12345678-abc...)'}
                                    className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                />
                            </div>
                            <div>
                                <label className="text-[11px] font-mono text-white/60 block mb-1">
                                    ElevenLabs API Key (Voces Ultra Realistas) {studioStatus?.providers.has_elevenlabs_key ? '· ✓ Guardada' : ''}
                                </label>
                                <input
                                    type="password"
                                    value={cfgElevenKey}
                                    onChange={(e) => setCfgElevenKey(e.target.value)}
                                    placeholder={studioStatus?.providers.has_elevenlabs_key ? '•••••••• (guardada en clave cifrada)' : 'sk_...'}
                                    className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                />
                            </div>
                        </div>

                        <div className="space-y-2 p-3.5 rounded-xl bg-black/40 border border-white/10">
                            <p className="text-xs font-mono uppercase text-aegis-cyan font-bold">
                                IA Local en PC Propia (ComfyUI / A1111 / Piper)
                            </p>
                            <div className="grid grid-cols-3 gap-2">
                                <input
                                    value={cfgLocalImageUrl}
                                    onChange={(e) => setCfgLocalImageUrl(e.target.value)}
                                    placeholder="URL Imagen/Video Local (ej: http://127.0.0.1:8188)"
                                    className="col-span-2 px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                />
                                <select
                                    value={cfgLocalImageMode}
                                    onChange={(e) => setCfgLocalImageMode(e.target.value)}
                                    className="px-2 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                >
                                    <option value="auto">Auto (:8188/:7860)</option>
                                    <option value="comfyui">ComfyUI</option>
                                    <option value="a1111">Automatic1111</option>
                                    <option value="openai_image">OpenAI Compatible</option>
                                </select>
                            </div>
                            <input
                                value={cfgLocalTtsUrl}
                                onChange={(e) => setCfgLocalTtsUrl(e.target.value)}
                                placeholder="URL TTS Local (ej: http://127.0.0.1:8880 para Kokoro/Piper)"
                                className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                            />
                            <input
                                value={cfgLocalVideoUrl}
                                onChange={(e) => setCfgLocalVideoUrl(e.target.value)}
                                placeholder="URL Video Local opcional (ej: Wan/LTX ComfyUI)"
                                className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                            />
                        </div>
                    </div>

                    <div className="flex justify-end gap-2">
                        <button
                            disabled={busyAction === 'config'}
                            onClick={() => void handleSaveConfig()}
                            className="px-4 py-2 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold hover:opacity-90"
                        >
                            Guardar Configuración Segura
                        </button>
                    </div>
                </div>
            )}

            {/* Create New Project Drawer — Automático 1-Click por defecto */}
            {showCreateForm && (
                <div className="p-5 rounded-2xl bg-[#0e131b] border border-purple-400/35 space-y-4">
                    <div className="flex items-center justify-between">
                        <div>
                            <h4 className="text-sm font-bold text-white flex items-center gap-2">
                                <Wand2 className="w-4 h-4 text-purple-300" />
                                Crear Video con IA (Modo Automático)
                            </h4>
                            <p className="text-[11px] text-white/50 mt-0.5">
                                Solo escribí la idea o pegá un guion y elegí el formato: Aegis elige el mejor motor y exporta el MP4 sin configuración manual.
                            </p>
                        </div>
                        <button
                            onClick={() => setShowCreateForm(false)}
                            className="text-xs font-mono text-white/50 hover:text-white"
                        >
                            Cancelar
                        </button>
                    </div>

                    <div className="space-y-3">
                        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3">
                            {(studioStatus?.creator_scripts?.length ?? 0) > 0 && (
                                <div className="lg:col-span-5">
                                    <label className="text-[11px] font-mono text-white/60 block mb-1">
                                        Importar Guion de Creator Studio (opcional)
                                    </label>
                                    <select
                                        value={newScriptId}
                                        onChange={(e) => {
                                            const sid = e.target.value;
                                            setNewScriptId(sid);
                                            const found = studioStatus?.creator_scripts.find((s) => s.id === sid);
                                            if (found) {
                                                setNewTitle(found.title);
                                                setNewAspect(found.platform === 'youtube' ? '16:9' : '9:16');
                                            }
                                        }}
                                        className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                    >
                                        <option value="">-- Escribir tema/guion libre abajo --</option>
                                        {(studioStatus?.creator_scripts || []).map((scr) => (
                                            <option key={scr.id} value={scr.id}>
                                                [{scr.platform}] {scr.title}
                                            </option>
                                        ))}
                                    </select>
                                </div>
                            )}
                            <div className={(studioStatus?.creator_scripts?.length ?? 0) > 0 ? 'lg:col-span-7' : 'lg:col-span-12'}>
                                <label className="text-[11px] font-mono text-white/60 block mb-1">
                                    Título (opcional)
                                </label>
                                <input
                                    value={newTitle}
                                    onChange={(e) => setNewTitle(e.target.value)}
                                    placeholder="Ej: 3 ventajas de tener tu IA privada con Aegis OS"
                                    className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none focus:border-purple-400"
                                />
                            </div>
                        </div>

                        <textarea
                            value={newPrompt}
                            onChange={(e) => setNewPrompt(e.target.value)}
                            rows={3}
                            placeholder={
                                newScriptId
                                    ? 'Usará el guion seleccionado de Creator Studio (podés dejarlo vacío o agregar indicaciones)...'
                                    : 'Escribí el tema, guion o las escenas que querés que Aegis locute y edite en MP4...'
                            }
                            className="w-full px-3 py-2 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none focus:border-purple-400"
                        />

                        <div>
                            <label className="text-[11px] font-mono text-white/60 block mb-1">
                                Formato de Pantalla
                            </label>
                            <div className="grid grid-cols-3 gap-2">
                                {[
                                    { id: '9:16', label: '9:16 Vertical (Reels/TikTok)' },
                                    { id: '16:9', label: '16:9 Horizontal (YouTube)' },
                                    { id: '1:1', label: '1:1 Cuadrado' },
                                ].map((fmt) => (
                                    <button
                                        key={fmt.id}
                                        onClick={() => setNewAspect(fmt.id)}
                                        className={`px-2.5 py-2 rounded-lg text-[11px] font-mono border text-center ${
                                            newAspect === fmt.id
                                                ? 'bg-purple-500/25 border-purple-400 text-purple-100 font-bold'
                                                : 'bg-black/40 border-white/10 text-white/60 hover:text-white'
                                        }`}
                                    >
                                        {fmt.label}
                                    </button>
                                ))}
                            </div>
                        </div>

                        <div className="pt-1 border-t border-white/10">
                            <button
                                type="button"
                                onClick={() => setShowCreateAdvanced((v) => !v)}
                                className="text-[11px] font-mono text-white/60 hover:text-aegis-cyan flex items-center gap-1.5"
                            >
                                <Settings2 className="w-3.5 h-3.5" />
                                {showCreateAdvanced ? 'Ocultar configuración avanzada' : 'Configuración avanzada de motores (opcional)'}
                                {showCreateAdvanced ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                            </button>

                            {showCreateAdvanced && (
                                <div className="mt-3 space-y-3 p-3 rounded-xl bg-black/35 border border-white/10">
                                    {!newScriptId && (
                                        <label className="flex items-center gap-2 text-xs text-white/75 cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={useAiStoryboard}
                                                onChange={(e) => setUseAiStoryboard(e.target.checked)}
                                            />
                                            Usar LLM para desglosar automáticamente el Storyboard por escenas (Hook + Desarrollo + CTA)
                                        </label>
                                    )}
                                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                                        {PRESET_OPTIONS.map((p) => (
                                            <button
                                                key={p.id}
                                                onClick={() => setNewPreset(p.id)}
                                                className={`p-2.5 rounded-xl border text-left transition-all ${
                                                    newPreset === p.id
                                                        ? 'bg-aegis-cyan/15 border-aegis-cyan/50 text-white'
                                                        : 'bg-black/40 border-white/10 text-white/65 hover:border-white/25'
                                                }`}
                                            >
                                                <p className="text-xs font-mono font-bold text-aegis-cyan">
                                                    {p.label}
                                                </p>
                                                <p className="text-[10px] text-white/55 mt-0.5">{p.desc}</p>
                                            </button>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>

                    <div className="flex flex-wrap justify-end gap-2">
                        <button
                            disabled={busyAction === 'create' || (!newTitle.trim() && !newPrompt.trim() && !newScriptId)}
                            onClick={() => void handleCreateProject(false)}
                            className="px-4 py-2.5 rounded-xl bg-white/5 hover:bg-white/10 border border-white/15 text-white/80 font-mono text-xs disabled:opacity-40 flex items-center gap-2"
                        >
                            <FileText className="w-3.5 h-3.5" />
                            Solo crear borrador / editar escenas
                        </button>
                        <button
                            disabled={busyAction === 'create' || (!newTitle.trim() && !newPrompt.trim() && !newScriptId)}
                            onClick={() => void handleCreateProject(true)}
                            className="px-5 py-2.5 rounded-xl bg-gradient-to-r from-purple-500 to-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40 flex items-center gap-2"
                        >
                            {busyAction === 'create' ? (
                                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                            ) : (
                                <Play className="w-3.5 h-3.5" />
                            )}
                            Crear y Renderizar Video (Automático)
                        </button>
                    </div>
                </div>
            )}

            {/* Main Workspace: Project List + Storyboard & Universal Engine Selector */}
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                {/* Left Sidebar: Projects */}
                <div className="lg:col-span-3 space-y-2.5">
                    <div className="flex items-center justify-between">
                        <h4 className="text-xs font-mono uppercase tracking-wider text-white/50 font-bold">
                            Proyectos ({projects.length})
                        </h4>
                        <button
                            onClick={() => void loadAll()}
                            className="p-1 text-white/40 hover:text-white"
                            title="Actualizar"
                        >
                            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
                        </button>
                    </div>

                    {projects.length === 0 ? (
                        <div className="p-5 rounded-xl border border-dashed border-white/15 text-center space-y-2">
                            <Film className="w-6 h-6 text-purple-300/60 mx-auto" />
                            <p className="text-xs text-white/55">
                                Creá tu primer video IA con "+ Nuevo Video IA" o importá un guion desde Creator Studio.
                            </p>
                        </div>
                    ) : (
                        <div className="space-y-2">
                            {projects.map((p) => {
                                const active = p.id === selectedProjectId;
                                return (
                                    <div
                                        key={p.id}
                                        onClick={() => setSelectedProjectId(p.id)}
                                        className={`p-3 rounded-xl border cursor-pointer transition-all space-y-1 ${
                                            active
                                                ? 'bg-purple-500/15 border-purple-400/50 text-white'
                                                : 'bg-white/[0.02] border-white/10 text-white/70 hover:border-white/25'
                                        }`}
                                    >
                                        <div className="flex items-start justify-between gap-2">
                                            <span className="text-xs font-bold line-clamp-2">{p.title}</span>
                                            <button
                                                onClick={async (e) => {
                                                    e.stopPropagation();
                                                    await apiFetch(`/api/video/projects/${p.id}`, { method: 'DELETE' });
                                                    await loadAll();
                                                }}
                                                className="text-white/30 hover:text-red-400 shrink-0"
                                                title="Eliminar proyecto"
                                            >
                                                <Trash2 className="w-3.5 h-3.5" />
                                            </button>
                                        </div>
                                        <div className="flex flex-wrap items-center gap-1.5 text-[10px] font-mono">
                                            <span className="px-1.5 py-0.5 rounded bg-black/50 text-aegis-cyan">
                                                {p.aspect_ratio}
                                            </span>
                                            <span className="px-1.5 py-0.5 rounded bg-black/50 text-white/50">
                                                {p.scenes.length} escenas
                                            </span>
                                            <span
                                                className={`px-1.5 py-0.5 rounded ${
                                                    p.status === 'rendered'
                                                        ? 'bg-emerald-500/20 text-emerald-300'
                                                        : p.status === 'error'
                                                          ? 'bg-red-500/20 text-red-300'
                                                          : 'bg-white/10 text-white/60'
                                                }`}
                                            >
                                                {p.status}
                                            </span>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>

                {/* Right Pane: Selected Project Studio */}
                <div className="lg:col-span-9 space-y-5">
                    {!selectedProject ? (
                        <div className="p-10 rounded-2xl border border-dashed border-white/15 text-center space-y-3">
                            <Sparkles className="w-8 h-8 text-purple-300 mx-auto opacity-75" />
                            <h3 className="text-sm font-bold text-white">
                                Elegí o creá un proyecto para empezar a producir video con IA
                            </h3>
                            <p className="text-xs text-white/55 max-w-lg mx-auto">
                                Podés empezar 100% gratis con <strong>Edge-TTS + Placas Animadas HD</strong>, usar tus claves de <strong>Gemini Veo / Imagen / OpenAI / ElevenLabs</strong>, o apuntar a tu servidor de <strong>IA Local (ComfyUI/A1111)</strong>.
                            </p>
                            {onAskAegis && (
                                <button
                                    onClick={() =>
                                        onAskAegis(
                                            'Usando Aegis Video Studio, creame un proyecto de video vertical 9:16 de 4 escenas explicando las ventajas de usar un asistente IA privado como Aegis, usando el preset gratuito.',
                                        )
                                    }
                                    className="px-4 py-2 rounded-xl bg-purple-500/25 border border-purple-400/40 text-purple-100 text-xs font-mono inline-flex items-center gap-2 hover:bg-purple-500/35"
                                >
                                    <Sparkles className="w-3.5 h-3.5" />
                                    O pedile a Aegis por Chat que arme uno ahora
                                </button>
                            )}
                        </div>
                    ) : (
                        <>
                            {/* Project Header + Actions */}
                            <div className="p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-4">
                                <div className="flex flex-wrap items-start justify-between gap-3">
                                    <div>
                                        <div className="flex items-center gap-2 flex-wrap">
                                            <h3 className="text-base font-bold text-white">{selectedProject.title}</h3>
                                            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-white/10 text-aegis-cyan">
                                                {selectedProject.aspect_ratio}
                                            </span>
                                            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-black/50 text-white/55 border border-white/10">
                                                Voz: {selectedProject.voice_id} · Visual: {selectedProject.visual_engine}
                                            </span>
                                        </div>
                                        {selectedProject.last_error && (
                                            <p className="text-xs text-red-300 font-mono mt-1">
                                                ⚠ {selectedProject.last_error}
                                            </p>
                                        )}
                                    </div>

                                    <div className="flex flex-wrap items-center gap-2">
                                        <button
                                            disabled={Boolean(busyAction)}
                                            onClick={() => void handleRenderMp4()}
                                            className="px-4 py-2 rounded-xl bg-gradient-to-r from-purple-500 to-aegis-cyan hover:opacity-90 text-black font-mono text-xs font-bold flex items-center gap-1.5"
                                        >
                                            <Play className="w-3.5 h-3.5" />
                                            Producir MP4 Completo
                                        </button>

                                        <button
                                            disabled={Boolean(busyAction)}
                                            onClick={() => void handleGenerateAssets()}
                                            className="px-3 py-2 rounded-xl bg-white/5 hover:bg-white/10 border border-white/15 text-white/80 font-mono text-xs flex items-center gap-1.5"
                                        >
                                            <Wand2 className="w-3.5 h-3.5 text-purple-300" />
                                            Regenerar Recursos
                                        </button>

                                        <button
                                            disabled={Boolean(busyAction)}
                                            onClick={async () => {
                                                setBusyAction('storyboard');
                                                try {
                                                    const res = await apiFetch(
                                                        `/api/video/projects/${selectedProject.id}/storyboard`,
                                                        { method: 'POST', body: JSON.stringify({}) },
                                                    );
                                                    if (res?.project) {
                                                        setProjects((prev) =>
                                                            prev.map((p) => (p.id === res.project.id ? res.project : p)),
                                                        );
                                                    }
                                                } catch (e) {
                                                    setErrorBanner(e instanceof Error ? e.message : String(e));
                                                } finally {
                                                    setBusyAction(null);
                                                }
                                            }}
                                            className="px-3 py-2 rounded-xl bg-white/5 hover:bg-white/10 border border-white/15 text-xs font-mono text-white/85 flex items-center gap-1.5"
                                            title="Re-escribir escenas con IA"
                                        >
                                            <Sparkles className="w-3.5 h-3.5 text-purple-300" />
                                            Re-armar Escenas IA
                                        </button>

                                        {selectedProject.output_mp4 && onScheduleResult && (
                                            <button
                                                onClick={() => onScheduleResult(selectedProject.title, selectedProject.platform)}
                                                className="px-3 py-2 rounded-xl bg-pink-500/20 border border-pink-400/35 text-pink-200 font-mono text-xs flex items-center gap-1.5 hover:bg-pink-500/30"
                                            >
                                                <Calendar className="w-3.5 h-3.5" />
                                                Agendar en Creator
                                            </button>
                                        )}
                                    </div>
                                </div>

                                <FallbacksAppliedBanner fallbacks={selectedProject.fallbacks_applied} />

                                <div className="pt-2 border-t border-white/10 flex flex-wrap items-center justify-between gap-2">
                                    <button
                                        type="button"
                                        onClick={() => setShowProjectAdvanced((v) => !v)}
                                        className="text-[11px] font-mono text-white/60 hover:text-aegis-cyan flex items-center gap-1.5"
                                    >
                                        <Settings2 className="w-3.5 h-3.5" />
                                        {showProjectAdvanced ? 'Ocultar motores avanzados' : 'Configurar motores avanzados (opcional)'}
                                        {showProjectAdvanced ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
                                    </button>
                                </div>

                                {/* Universal Engine Selector Grid — colapsable */}
                                {showProjectAdvanced && (
                                <div className="space-y-3 pt-2 border-t border-white/10">
                                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                                    <div>
                                        <label className="text-[10px] font-mono text-white/50 flex items-center gap-1 mb-1">
                                            <Mic className="w-3 h-3 text-aegis-cyan" /> Motor de Voz ({getEngineBadge(selectedProject.voice_engine).hint})
                                        </label>
                                        <select
                                            value={selectedProject.voice_engine}
                                            onChange={(e) =>
                                                void handleUpdateProjectEngines({
                                                    preset: 'custom',
                                                    voice_engine: e.target.value,
                                                })
                                            }
                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                        >
                                            <option value="edge_tts">Edge TTS HD (Gratis · Masterizado)</option>
                                            <option value="gemini_tts">Gemini IA Studio (WAV HD · API)</option>
                                            <option value="openai_tts">OpenAI TTS HD (API)</option>
                                            <option value="elevenlabs">ElevenLabs (API)</option>
                                            <option value="local_tts">TTS Local (Piper/Kokoro)</option>
                                            <option value="none">Sin voz</option>
                                        </select>
                                    </div>

                                    <div>
                                        <label className="text-[10px] font-mono text-white/50 flex items-center gap-1 mb-1">
                                            <Mic className="w-3 h-3 text-purple-300" /> Locutor / Voz
                                        </label>
                                        <select
                                            value={selectedProject.voice_id}
                                            onChange={(e) =>
                                                void handleUpdateProjectEngines({ voice_id: e.target.value })
                                            }
                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                        >
                                            {filteredVoices.map((v) => (
                                                <option key={v.id} value={v.id}>
                                                    {v.label}
                                                </option>
                                            ))}
                                        </select>
                                    </div>

                                    <div>
                                        <label className="text-[10px] font-mono text-white/50 flex items-center gap-1 mb-1">
                                            <ImageIcon className="w-3 h-3 text-pink-300" /> Visuales ({getEngineBadge(selectedProject.visual_engine).hint})
                                        </label>
                                        <select
                                            value={selectedProject.visual_engine}
                                            onChange={(e) =>
                                                void handleUpdateProjectEngines({
                                                    preset: 'custom',
                                                    visual_engine: e.target.value,
                                                })
                                            }
                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                        >
                                            <option value="pollinations_ai">Pollinations Flux IA (Gratis sin Key ★)</option>
                                            <option value="kinetic_card">Placas Animadas (Gratis)</option>
                                            <option value="pexels_stock">Pixabay / Pexels Stock HD (Videos Gratis con Key)</option>
                                            <option value="gemini_imagen">Gemini Imagen (API)</option>
                                            <option value="openai_image">OpenAI Images (API)</option>
                                            <option value="google_veo">Google Veo Video IA (API)</option>
                                            <option value="local_ai">IA Local (ComfyUI / A1111)</option>
                                        </select>
                                    </div>

                                    <div>
                                        <label className="text-[10px] font-mono text-white/50 flex items-center gap-1 mb-1">
                                            <Video className="w-3 h-3 text-amber-300" /> Hook Escena 1
                                        </label>
                                        <select
                                            value={selectedProject.hook_video_engine}
                                            onChange={(e) =>
                                                void handleUpdateProjectEngines({
                                                    preset: 'custom',
                                                    hook_video_engine: e.target.value,
                                                })
                                            }
                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                        >
                                            <option value="inherit">Igual al resto</option>
                                            <option value="pollinations_ai">Pollinations Flux IA (Gratis sin Key)</option>
                                            <option value="google_veo">Google Veo Video IA (Hook)</option>
                                            <option value="pexels_stock">Pixabay / Pexels Stock Video</option>
                                            <option value="gemini_imagen">Gemini Imagen</option>
                                            <option value="openai_image">OpenAI Images</option>
                                            <option value="local_ai">IA Local (GPU)</option>
                                            <option value="kinetic_card">Placa Animada</option>
                                        </select>
                                    </div>

                                    <div>
                                        <label className="text-[10px] font-mono text-white/50 flex items-center gap-1 mb-1">
                                            <Subtitles className="w-3 h-3 text-emerald-300" /> Subtítulos ({getEngineBadge(selectedProject.subtitle_engine).hint})
                                        </label>
                                        <select
                                            value={selectedProject.subtitle_engine}
                                            onChange={(e) =>
                                                void handleUpdateProjectEngines({
                                                    preset: 'custom',
                                                    subtitle_engine: e.target.value,
                                                })
                                            }
                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/60 border border-white/15 text-xs text-white outline-none"
                                        >
                                            <option value="scene_auto">Sync Auto (Gratis)</option>
                                            <option value="groq_whisper">Groq Whisper (Palabra)</option>
                                            <option value="openai_whisper">OpenAI Whisper</option>
                                            <option value="whisper_local">Whisper.cpp Local</option>
                                            <option value="disabled">Sin subtítulos</option>
                                        </select>
                                    </div>
                                </div>
                                </div>
                                )}

                                <div className="flex flex-wrap items-center gap-3 text-[11px] font-mono text-white/55">
                                    <span>Subtítulos:</span>
                                    {[
                                        { id: 'tiktok_bold', label: 'TikTok Karaoke' },
                                        { id: 'amber_highlight', label: 'Amarillo Viral' },
                                        { id: 'clean_bottom', label: 'Minimal' },
                                    ].map((st) => (
                                        <button
                                            key={st.id}
                                            onClick={() => void handleUpdateProjectEngines({ subtitle_style: st.id })}
                                            className={`px-2 py-0.5 rounded border ${
                                                selectedProject.subtitle_style === st.id
                                                    ? 'bg-emerald-500/20 border-emerald-400/40 text-emerald-200'
                                                    : 'bg-black/40 border-white/10 text-white/50 hover:text-white'
                                            }`}
                                        >
                                            {st.label}
                                        </button>
                                    ))}
                                    <span className="ml-2">Música:</span>
                                    {[
                                        { id: 'ambient_pulse', label: 'Pulso Tech' },
                                        { id: 'cinematic_pad', label: 'Pad Suave' },
                                        { id: 'none', label: 'Sin Música' },
                                    ].map((bg) => (
                                        <button
                                            key={bg.id}
                                            onClick={() => void handleUpdateProjectEngines({ bgm_style: bg.id })}
                                            className={`px-2 py-0.5 rounded border ${
                                                (selectedProject.bgm_style || 'ambient_pulse') === bg.id
                                                    ? 'bg-purple-500/25 border-purple-400/45 text-purple-200'
                                                    : 'bg-black/40 border-white/10 text-white/50 hover:text-white'
                                            }`}
                                        >
                                            {bg.label}
                                        </button>
                                    ))}
                                    <span className="ml-auto flex items-center gap-1.5">
                                        Formato:
                                        {(['9:16', '16:9', '1:1'] as const).map((ar) => (
                                            <button
                                                key={ar}
                                                onClick={() => void handleUpdateProjectEngines({ aspect_ratio: ar })}
                                                className={`px-2 py-0.5 rounded border ${
                                                    selectedProject.aspect_ratio === ar
                                                        ? 'bg-aegis-cyan/20 border-aegis-cyan/40 text-aegis-cyan'
                                                        : 'bg-black/40 border-white/10 text-white/50'
                                                }`}
                                            >
                                                {ar}
                                            </button>
                                        ))}
                                    </span>
                                </div>
                            </div>

                            {/* Final Exported Player */}
                            <FinalVideoPlayer
                                project={selectedProject}
                                tenantId={tenantId}
                                sessionKey={sessionKey}
                            />

                            {/* Scenes Storyboard Cards */}
                            <div className="space-y-3">
                                <h4 className="text-xs font-mono uppercase tracking-wider text-white/55 font-bold">
                                    Storyboard Editable ({selectedProject.scenes.length} escenas)
                                </h4>

                                {selectedProject.scenes.map((sc, idx) => (
                                    <div
                                        key={sc.id}
                                        className="p-4 rounded-2xl bg-white/[0.03] border border-white/10 grid grid-cols-1 md:grid-cols-12 gap-4"
                                    >
                                        <div className="md:col-span-8 space-y-2.5">
                                            <div className="flex flex-wrap items-center justify-between gap-2">
                                                <div className="flex items-center gap-2">
                                                    <span className="px-2 py-0.5 rounded-full text-[10px] font-mono font-bold bg-purple-500/20 text-purple-200 border border-purple-400/30">
                                                        #{idx + 1}
                                                    </span>
                                                    <input
                                                        defaultValue={sc.title}
                                                        onBlur={(e) => {
                                                            if (e.target.value.trim() !== sc.title) {
                                                                void handleManageScene({
                                                                    action: 'update',
                                                                    scene_id: sc.id,
                                                                    title: e.target.value,
                                                                });
                                                            }
                                                        }}
                                                        className="bg-transparent border-b border-transparent hover:border-white/20 focus:border-aegis-cyan text-sm font-bold text-white outline-none"
                                                    />
                                                </div>

                                                <div className="flex items-center gap-1">
                                                    {idx > 0 && (
                                                        <button
                                                            onClick={() =>
                                                                void handleManageScene({
                                                                    action: 'move',
                                                                    scene_id: sc.id,
                                                                    new_index: idx - 1,
                                                                })
                                                            }
                                                            className="p-1 text-white/40 hover:text-white"
                                                            title="Subir escena"
                                                        >
                                                            <ChevronUp className="w-3.5 h-3.5" />
                                                        </button>
                                                    )}
                                                    {idx < selectedProject.scenes.length - 1 && (
                                                        <button
                                                            onClick={() =>
                                                                void handleManageScene({
                                                                    action: 'move',
                                                                    scene_id: sc.id,
                                                                    new_index: idx + 1,
                                                                })
                                                            }
                                                            className="p-1 text-white/40 hover:text-white"
                                                            title="Bajar escena"
                                                        >
                                                            <ChevronDown className="w-3.5 h-3.5" />
                                                        </button>
                                                    )}
                                                    <button
                                                        disabled={Boolean(busyAction)}
                                                        onClick={() => void handleGenerateAssets(sc.id)}
                                                        className="px-2.5 py-1 rounded-lg bg-white/10 hover:bg-purple-500/30 text-[11px] font-mono text-white flex items-center gap-1"
                                                    >
                                                        <RefreshCw className="w-3 h-3" />
                                                        Regenerar
                                                    </button>
                                                    <button
                                                        onClick={() =>
                                                            void handleManageScene({
                                                                action: 'delete',
                                                                scene_id: sc.id,
                                                            })
                                                        }
                                                        className="p-1.5 text-white/35 hover:text-red-400"
                                                        title="Eliminar escena"
                                                    >
                                                        <Trash2 className="w-3.5 h-3.5" />
                                                    </button>
                                                </div>
                                            </div>

                                            <div className="space-y-2">
                                                <div>
                                                    <label className="text-[10px] font-mono text-white/40 block">
                                                        Texto Locutado (Voz IA + Subtítulos)
                                                    </label>
                                                    <textarea
                                                        defaultValue={sc.narration_text}
                                                        rows={2}
                                                        onBlur={(e) => {
                                                            if (e.target.value.trim() !== sc.narration_text) {
                                                                void handleManageScene({
                                                                    action: 'update',
                                                                    scene_id: sc.id,
                                                                    narration_text: e.target.value,
                                                                });
                                                            }
                                                        }}
                                                        className="w-full px-2.5 py-1.5 rounded-lg bg-black/50 border border-white/10 text-xs text-white outline-none focus:border-aegis-cyan"
                                                    />
                                                </div>
                                                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                                    <div>
                                                        <label className="text-[10px] font-mono text-white/40 block">
                                                            Texto en Pantalla / Placa
                                                        </label>
                                                        <input
                                                            defaultValue={sc.on_screen_text}
                                                            onBlur={(e) => {
                                                                if (e.target.value.trim() !== sc.on_screen_text) {
                                                                    void handleManageScene({
                                                                        action: 'update',
                                                                        scene_id: sc.id,
                                                                        on_screen_text: e.target.value,
                                                                    });
                                                                }
                                                            }}
                                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/50 border border-white/10 text-xs text-white outline-none"
                                                        />
                                                    </div>
                                                    <div>
                                                        <label className="text-[10px] font-mono text-white/40 block">
                                                            Prompt Visual / Stock Keywords
                                                        </label>
                                                        <input
                                                            defaultValue={sc.visual_prompt}
                                                            onBlur={(e) => {
                                                                if (e.target.value.trim() !== sc.visual_prompt) {
                                                                    void handleManageScene({
                                                                        action: 'update',
                                                                        scene_id: sc.id,
                                                                        visual_prompt: e.target.value,
                                                                    });
                                                                }
                                                            }}
                                                            className="w-full px-2.5 py-1.5 rounded-lg bg-black/50 border border-white/10 text-xs text-white outline-none"
                                                        />
                                                    </div>
                                                </div>

                                                <div className="flex flex-wrap items-center gap-2 pt-1">
                                                    <select
                                                        value={sc.visual_override || 'inherit'}
                                                        onChange={(e) =>
                                                            void handleManageScene({
                                                                action: 'update',
                                                                scene_id: sc.id,
                                                                visual_override: e.target.value,
                                                            })
                                                        }
                                                        className="px-2 py-1 rounded bg-black/60 border border-white/15 text-[11px] font-mono text-white/80 outline-none"
                                                    >
                                                        <option value="inherit">Visual: Heredado del proyecto</option>
                                                        <option value="pollinations_ai">Esta escena: Pollinations Flux IA (Gratis)</option>
                                                        <option value="kinetic_card">Esta escena: Placa Animada (Gratis)</option>
                                                        <option value="pexels_stock">Esta escena: Pixabay / Pexels Stock HD</option>
                                                        <option value="gemini_imagen">Esta escena: Gemini Imagen</option>
                                                        <option value="openai_image">Esta escena: OpenAI Images</option>
                                                        <option value="google_veo">Esta escena: Google Veo Video IA</option>
                                                        <option value="local_ai">Esta escena: IA Local (GPU)</option>
                                                    </select>

                                                    <select
                                                        value={sc.motion_effect}
                                                        onChange={(e) =>
                                                            void handleManageScene({
                                                                action: 'update',
                                                                scene_id: sc.id,
                                                                motion_effect: e.target.value,
                                                            })
                                                        }
                                                        className="px-2 py-1 rounded bg-black/60 border border-white/15 text-[11px] font-mono text-white/80 outline-none"
                                                    >
                                                        <option value="zoom_in">Movimiento: Zoom In</option>
                                                        <option value="zoom_out">Movimiento: Zoom Out</option>
                                                        <option value="pan_left">Movimiento: Paneo</option>
                                                        <option value="static">Estático</option>
                                                    </select>

                                                    <input
                                                        ref={(el) => {
                                                            fileInputRefs.current[sc.id] = el;
                                                        }}
                                                        type="file"
                                                        accept="image/*,video/*,audio/*"
                                                        className="hidden"
                                                        onChange={(e) => {
                                                            const f = e.target.files?.[0];
                                                            if (f) void handleUploadSceneMedia(sc.id, f);
                                                            e.target.value = '';
                                                        }}
                                                    />
                                                    <button
                                                        onClick={() => fileInputRefs.current[sc.id]?.click()}
                                                        className="px-2.5 py-1 rounded bg-white/5 hover:bg-white/15 border border-white/15 text-[11px] font-mono text-aegis-cyan flex items-center gap-1"
                                                        title="Subir tu propia imagen, video o audio para esta escena"
                                                    >
                                                        <Upload className="w-3 h-3" />
                                                        Subir Foto/Clip/Audio
                                                    </button>
                                                </div>

                                                {sc.error && (
                                                    <p className="text-[11px] font-mono text-red-300 bg-red-500/10 border border-red-400/25 px-2.5 py-1.5 rounded-lg">
                                                        ⚠ {sc.error}
                                                    </p>
                                                )}
                                            </div>
                                        </div>

                                        <div className="md:col-span-4 flex flex-col justify-between">
                                            <SceneVisualPreview
                                                projectId={selectedProject.id}
                                                scene={sc}
                                                tenantId={tenantId}
                                                sessionKey={sessionKey}
                                                updatedAt={selectedProject.updated_at}
                                            />
                                        </div>
                                    </div>
                                ))}

                                {/* Add Scene Form */}
                                <div className="p-4 rounded-2xl bg-white/[0.02] border border-dashed border-white/15 space-y-2.5">
                                    <p className="text-xs font-mono text-white/60 font-bold flex items-center gap-1.5">
                                        <FileText className="w-3.5 h-3.5 text-aegis-cyan" />
                                        + Agregar escena al final
                                    </p>
                                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
                                        <input
                                            value={addSceneTitle}
                                            onChange={(e) => setAddSceneTitle(e.target.value)}
                                            placeholder="Título escena..."
                                            className="px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                        />
                                        <input
                                            value={addSceneNarration}
                                            onChange={(e) => setAddSceneNarration(e.target.value)}
                                            placeholder="Texto hablado..."
                                            className="px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                        />
                                        <div className="flex gap-2">
                                            <input
                                                value={addSceneVisualPrompt}
                                                onChange={(e) => setAddSceneVisualPrompt(e.target.value)}
                                                placeholder="Prompt visual..."
                                                className="flex-1 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                                            />
                                            <button
                                                disabled={!addSceneTitle.trim() && !addSceneNarration.trim()}
                                                onClick={async () => {
                                                    await handleManageScene({
                                                        action: 'add',
                                                        title: addSceneTitle.trim() || 'Escena extra',
                                                        narration_text: addSceneNarration.trim(),
                                                        on_screen_text: addSceneTitle.trim(),
                                                        visual_prompt: addSceneVisualPrompt.trim() || addSceneNarration.trim(),
                                                    });
                                                    setAddSceneTitle('');
                                                    setAddSceneNarration('');
                                                    setAddSceneVisualPrompt('');
                                                }}
                                                className="px-3 py-2 rounded-lg bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40"
                                            >
                                                Agregar
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </>
                    )}
                </div>
            </div>
        </div>
    );
};
