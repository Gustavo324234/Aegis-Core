import React, { useState, useEffect } from 'react';
import {
    ChevronDown,
    ChevronUp,
    Cpu,
    Download,
    Film,
    Loader2,
    Subtitles,
    Wand2,
} from 'lucide-react';
import { VideoCapabilityAdvisor, VideoProject } from './types';
import { useAuthenticatedBlobUrl } from './useAuthenticatedBlobUrl';
import { FallbacksAppliedBanner, VideoCapabilityAdvisorCard } from './MediaPreviews';

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
