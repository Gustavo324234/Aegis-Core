import React from 'react';
import {
    AlertTriangle,
    CheckCircle2,
    Cpu,
    Download,
    Image as ImageIcon,
    Loader2,
    Subtitles,
} from 'lucide-react';
import { VideoCapabilityAdvisor, VideoProject, VideoScene } from './types';
import { useAuthenticatedBlobUrl } from './useAuthenticatedBlobUrl';

export const SceneVisualPreview: React.FC<{
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

export const FinalVideoPlayer: React.FC<{
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

export const FallbacksAppliedBanner: React.FC<{ fallbacks?: string[] }> = ({ fallbacks }) => {
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

export const VideoCapabilityAdvisorCard: React.FC<{ advisor: VideoCapabilityAdvisor }> = ({ advisor }) => {
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
