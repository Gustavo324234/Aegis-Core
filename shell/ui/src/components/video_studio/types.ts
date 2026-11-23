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

export interface HardwareSpecs {
    cpu_brand: string;
    cpu_cores: number;
    total_ram_gb: number;
    available_ram_gb: number;
    free_disk_gb: number;
    gpu_name: string;
    dedicated_vram_mb: number;
    has_nvidia_cuda: boolean;
}

export interface LocalModelRecommendation {
    category: string;
    model_name: string;
    min_ram_gb: number;
    min_vram_gb: number;
    compatible_with_this_pc: boolean;
    status_note: string;
}

export interface ApiPlanRecommendation {
    provider: string;
    current_status: string;
    recommended_plan: string;
    what_it_unlocks: string;
    estimated_cost: string;
}

export interface VideoCapabilityAdvisor {
    pc_specs: HardwareSpecs;
    pc_verdict: string;
    local_models: LocalModelRecommendation[];
    api_plans: ApiPlanRecommendation[];
}

export interface RuntimeStatus {
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

export interface ProviderReadiness {
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

export interface VoiceOption {
    id: string;
    label: string;
    engine: string;
    lang: string;
}

export interface CreatorScriptSummary {
    id: string;
    title: string;
    platform: string;
    hook_0_3s: string;
    body_script: string;
    b_roll_notes: string;
    cta: string;
}

export interface VideoProgressEvt {
    project_id: string;
    stage: string;
    progress: number;
    message: string;
    scene_id?: string | null;
    done: boolean;
    error?: string | null;
}

export interface RedactedConfig {
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

export interface VideoStudioStatus {
    runtime: RuntimeStatus;
    providers: ProviderReadiness;
    config: RedactedConfig;
    voices: VoiceOption[];
    creator_scripts: CreatorScriptSummary[];
    active_install?: VideoProgressEvt | null;
    advisor?: VideoCapabilityAdvisor | null;
}

export interface VideoStudioPanelProps {
    tenantId: string | null;
    sessionKey: string | null;
    pendingCreatorScriptId?: string | null;
    onConsumePendingScript?: () => void;
    targetProjectId?: string | null;
    onConsumeTargetProject?: () => void;
    onScheduleResult?: (projectTitle: string, platform: string) => void;
    onAskAegis?: (prompt: string) => void;
}

export const PRESET_OPTIONS = [
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
