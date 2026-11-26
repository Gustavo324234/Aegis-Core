use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type ProgressCallback = Arc<dyn Fn(VideoProgressEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProgressEvent {
    pub project_id: String,
    pub stage: String,
    pub progress: f32, // 0.0 ..= 1.0
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoScene {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub order: usize,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub narration_text: String,
    #[serde(default)]
    pub on_screen_text: String,
    #[serde(default)]
    pub visual_prompt: String,
    #[serde(default)]
    pub stock_keywords: String,
    #[serde(default = "default_duration_hint")]
    pub duration_hint_secs: f64,
    #[serde(default = "default_motion_effect")]
    pub motion_effect: String,
    #[serde(default)]
    pub voice_override: Option<String>,
    #[serde(default)]
    pub visual_override: Option<String>,
    #[serde(default)]
    pub audio_file: Option<String>,
    #[serde(default)]
    pub visual_file: Option<String>,
    #[serde(default)]
    pub visual_kind: Option<String>, // "image" | "video"
    #[serde(default)]
    pub actual_audio_secs: Option<f64>,
    #[serde(default = "default_pending")]
    pub status: String, // "pending" | "ready" | "error"
    #[serde(default)]
    pub error: Option<String>,
}

fn default_duration_hint() -> f64 {
    4.5
}

fn default_motion_effect() -> String {
    "zoom_in".to_string()
}

fn default_pending() -> String {
    "pending".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProject {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub source_script_id: Option<String>,
    #[serde(default = "default_platform")]
    pub platform: String,
    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_voice_engine")]
    pub voice_engine: String,
    #[serde(default = "default_voice_id")]
    pub voice_id: String,
    #[serde(default = "default_visual_engine")]
    pub visual_engine: String,
    #[serde(default = "default_inherit")]
    pub hook_video_engine: String,
    #[serde(default = "default_subtitle_engine")]
    pub subtitle_engine: String,
    #[serde(default = "default_subtitle_style")]
    pub subtitle_style: String,
    #[serde(default = "default_bgm_style")]
    pub bgm_style: String,
    #[serde(default)]
    pub scenes: Vec<VideoScene>,
    #[serde(default)]
    pub output_mp4: Option<String>,
    #[serde(default)]
    pub output_srt: Option<String>,
    #[serde(default = "default_draft")]
    pub status: String, // "draft" | "assets_ready" | "rendered" | "error"
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub fallbacks_applied: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn default_platform() -> String {
    "tiktok_reels".to_string()
}
fn default_aspect_ratio() -> String {
    "9:16".to_string()
}
fn default_preset() -> String {
    "free_builtin".to_string()
}
fn default_voice_engine() -> String {
    "edge_tts".to_string()
}
fn default_voice_id() -> String {
    "es-AR-TomasNeural".to_string()
}
fn default_visual_engine() -> String {
    "pollinations_ai".to_string()
}
fn default_inherit() -> String {
    "inherit".to_string()
}
fn default_subtitle_engine() -> String {
    "scene_auto".to_string()
}
fn default_subtitle_style() -> String {
    "tiktok_bold".to_string()
}
fn default_bgm_style() -> String {
    "ambient_pulse".to_string()
}
fn default_draft() -> String {
    "draft".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoModuleConfig {
    #[serde(default = "default_preset")]
    pub default_preset: String,
    #[serde(default = "default_voice_engine")]
    pub default_voice_engine: String,
    #[serde(default = "default_voice_id")]
    pub default_voice_id: String,
    #[serde(default = "default_visual_engine")]
    pub default_visual_engine: String,
    #[serde(default = "default_inherit")]
    pub default_hook_video_engine: String,
    #[serde(default = "default_subtitle_engine")]
    pub default_subtitle_engine: String,
    #[serde(default = "default_subtitle_style")]
    pub default_subtitle_style: String,
    #[serde(default = "default_bgm_style")]
    pub default_bgm_style: String,
    #[serde(default)]
    pub pexels_api_key: Option<String>,
    #[serde(default)]
    pub local_image_url: Option<String>,
    #[serde(default)]
    pub local_image_mode: Option<String>, // "auto" | "comfyui" | "openai_image" | "a1111"
    #[serde(default)]
    pub local_video_url: Option<String>,
    #[serde(default)]
    pub local_tts_url: Option<String>,
    #[serde(default)]
    pub local_stt_url: Option<String>,
}

impl Default for VideoModuleConfig {
    fn default() -> Self {
        Self {
            default_preset: default_preset(),
            default_voice_engine: default_voice_engine(),
            default_voice_id: default_voice_id(),
            default_visual_engine: default_visual_engine(),
            default_hook_video_engine: default_inherit(),
            default_subtitle_engine: default_subtitle_engine(),
            default_subtitle_style: default_subtitle_style(),
            default_bgm_style: default_bgm_style(),
            pexels_api_key: None,
            local_image_url: None,
            local_image_mode: Some("auto".to_string()),
            local_video_url: None,
            local_tts_url: None,
            local_stt_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedVideoModuleConfig {
    pub default_preset: String,
    pub default_voice_engine: String,
    pub default_voice_id: String,
    pub default_visual_engine: String,
    pub default_hook_video_engine: String,
    pub default_subtitle_engine: String,
    pub default_subtitle_style: String,
    pub default_bgm_style: String,
    pub has_pexels_key: bool,
    pub local_image_url: Option<String>,
    pub local_image_mode: Option<String>,
    pub local_video_url: Option<String>,
    pub local_tts_url: Option<String>,
    pub local_stt_url: Option<String>,
}

impl From<&VideoModuleConfig> for RedactedVideoModuleConfig {
    fn from(cfg: &VideoModuleConfig) -> Self {
        Self {
            default_preset: cfg.default_preset.clone(),
            default_voice_engine: cfg.default_voice_engine.clone(),
            default_voice_id: cfg.default_voice_id.clone(),
            default_visual_engine: cfg.default_visual_engine.clone(),
            default_hook_video_engine: cfg.default_hook_video_engine.clone(),
            default_subtitle_engine: cfg.default_subtitle_engine.clone(),
            default_subtitle_style: cfg.default_subtitle_style.clone(),
            default_bgm_style: cfg.default_bgm_style.clone(),
            has_pexels_key: cfg
                .pexels_api_key
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            local_image_url: cfg.local_image_url.clone(),
            local_image_mode: cfg.local_image_mode.clone(),
            local_video_url: cfg.local_video_url.clone(),
            local_tts_url: cfg.local_tts_url.clone(),
            local_stt_url: cfg.local_stt_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDependencyStatus {
    pub ffmpeg_ready: bool,
    pub ffprobe_ready: bool,
    pub python_ready: bool,
    pub edge_tts_ready: bool,
    pub pillow_ready: bool,
    pub ready_for_free_video: bool,
    pub ffmpeg_source: Option<String>,
    pub python_source: Option<String>,
    pub runtime_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReadiness {
    pub has_gemini_key: bool,
    pub has_openai_key: bool,
    pub has_groq_key: bool,
    pub has_elevenlabs_key: bool,
    pub has_pexels_key: bool,
    pub has_local_image_url: bool,
    pub has_local_video_url: bool,
    pub has_local_tts_url: bool,
    pub has_local_whisper: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceOption {
    pub id: String,
    pub label: String,
    pub engine: String,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorScriptSummary {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub hook_0_3s: String,
    pub body_script: String,
    pub b_roll_notes: String,
    pub cta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSpecs {
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub total_ram_gb: f32,
    pub available_ram_gb: f32,
    pub free_disk_gb: f32,
    pub gpu_name: String,
    pub dedicated_vram_mb: u64,
    pub has_nvidia_cuda: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelRecommendation {
    pub category: String,
    pub model_name: String,
    pub min_ram_gb: u32,
    pub min_vram_gb: u32,
    pub compatible_with_this_pc: bool,
    pub status_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPlanRecommendation {
    pub provider: String,
    pub current_status: String,
    pub recommended_plan: String,
    pub what_it_unlocks: String,
    pub estimated_cost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCapabilityAdvisor {
    pub pc_specs: HardwareSpecs,
    pub pc_verdict: String,
    pub local_models: Vec<LocalModelRecommendation>,
    pub api_plans: Vec<ApiPlanRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStudioStatusResponse {
    pub runtime: RuntimeDependencyStatus,
    pub providers: ProviderReadiness,
    pub config: RedactedVideoModuleConfig,
    pub voices: Vec<VoiceOption>,
    pub creator_scripts: Vec<CreatorScriptSummary>,
    pub active_install: Option<VideoProgressEvent>,
    #[serde(default)]
    pub advisor: Option<VideoCapabilityAdvisor>,
}

pub fn aspect_dimensions(aspect: &str) -> (u32, u32) {
    match aspect.trim() {
        "16:9" => (1920, 1080),
        "1:1" => (1080, 1080),
        _ => (1080, 1920),
    }
}

pub fn preset_engines(preset: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    // returns (voice_engine, visual_engine, hook_video_engine, subtitle_engine)
    match preset.trim() {
        "gemini_studio" => ("gemini_tts", "gemini_imagen", "inherit", "scene_auto"),
        "hybrid_pro" => ("edge_tts", "gemini_imagen", "google_veo", "groq_whisper"),
        "cloud_pro" => (
            "elevenlabs",
            "gemini_imagen",
            "google_veo",
            "openai_whisper",
        ),
        "local_ai" => ("local_tts", "local_ai", "inherit", "whisper_local"),
        "free_builtin" => ("edge_tts", "pollinations_ai", "inherit", "scene_auto"),
        _ => ("edge_tts", "pollinations_ai", "inherit", "scene_auto"),
    }
}

pub fn voice_catalog() -> Vec<VoiceOption> {
    [
        (
            "en-US-AndrewMultilingualNeural",
            "Andrew HD Multilingual (Masculina cálida · Gratis ★)",
            "edge_tts",
            "multi",
        ),
        (
            "en-US-AvaMultilingualNeural",
            "Ava HD Multilingual (Femenina natural · Gratis ★)",
            "edge_tts",
            "multi",
        ),
        (
            "en-US-BrianMultilingualNeural",
            "Brian HD Multilingual (Masculina profunda · Gratis ★)",
            "edge_tts",
            "multi",
        ),
        (
            "es-AR-ElenaNeural",
            "Elena (Argentina · Femenina · Gratis)",
            "edge_tts",
            "es-AR",
        ),
        (
            "es-AR-TomasNeural",
            "Tomás (Argentina · Masculina · Gratis)",
            "edge_tts",
            "es-AR",
        ),
        (
            "es-MX-JorgeNeural",
            "Jorge (México · Masculina · Gratis)",
            "edge_tts",
            "es-MX",
        ),
        (
            "es-MX-DaliaNeural",
            "Dalia (México · Femenina · Gratis)",
            "edge_tts",
            "es-MX",
        ),
        (
            "es-ES-XimenaNeural",
            "Ximena (España · Femenina · Gratis)",
            "edge_tts",
            "es-ES",
        ),
        (
            "es-ES-AlvaroNeural",
            "Álvaro (España · Masculina · Gratis)",
            "edge_tts",
            "es-ES",
        ),
        (
            "en-US-EmmaMultilingualNeural",
            "Emma HD Multilingual (English US · Free ★)",
            "edge_tts",
            "en-US",
        ),
        (
            "Charon",
            "Charon (Gemini IA · Masculina Profunda · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "Orus",
            "Orus (Gemini IA · Masculina Tech · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "Fenrir",
            "Fenrir (Gemini IA · Comercial Enérgica · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "Kore",
            "Kore (Gemini IA · Femenina Cálida · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "Aoede",
            "Aoede (Gemini IA · Femenina Brillante · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "Puck",
            "Puck (Gemini IA · Ágil Reels · Studio)",
            "gemini_tts",
            "multi",
        ),
        (
            "onyx",
            "Onyx (OpenAI TTS HD · Masculina)",
            "openai_tts",
            "multi",
        ),
        (
            "nova",
            "Nova (OpenAI TTS HD · Femenina)",
            "openai_tts",
            "multi",
        ),
        (
            "alloy",
            "Alloy (OpenAI TTS HD · Neutra)",
            "openai_tts",
            "multi",
        ),
        (
            "EXAVITQu4vr4xnSDxMaL",
            "Bella / Rachel (ElevenLabs Default)",
            "elevenlabs",
            "multi",
        ),
    ]
    .into_iter()
    .map(|(id, label, engine, lang)| VoiceOption {
        id: id.to_string(),
        label: label.to_string(),
        engine: engine.to_string(),
        lang: lang.to_string(),
    })
    .collect()
}
