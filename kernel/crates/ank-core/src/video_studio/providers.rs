use super::runtime_deps::{ensure_helper_scripts, resolve_python_for_video};
use super::store::{new_id, split_text_to_scenes};
use super::types::{
    aspect_dimensions, ProgressCallback, ProviderReadiness, VideoModuleConfig, VideoProgressEvent,
    VideoScene,
};
use crate::chal::drivers::cloud::CloudProxyDriver;
use crate::chal::drivers::siren::whisper_local::is_safe_whisper_model_id;
use crate::chal::{ChatMessage, ChatRole, InferenceDriver};
use crate::enclave::TenantDB;
use crate::router::{CognitiveRouter, KeyPool};
use base64::Engine as _;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

const MAX_MEDIA_DOWNLOAD_BYTES: usize = 90 * 1024 * 1024;

pub fn build_http_client(timeout_secs: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))
}

/// Validates user-supplied URLs for local AI engines, blocking cloud metadata IPs and embedded credentials.
pub fn validate_local_endpoint_url(raw_url: &str, allow_local: bool) -> Result<String, String> {
    let trimmed = raw_url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err("La URL del endpoint no puede estar vacía".to_string());
    }
    let parsed =
        reqwest::Url::parse(&trimmed).map_err(|_| "Formato de URL inválido".to_string())?;
    let scheme = parsed.scheme();
    if allow_local {
        if scheme != "http" && scheme != "https" {
            return Err("La URL debe usar http:// o https://".to_string());
        }
    } else if scheme != "https" {
        return Err(
            "Solo el administrador puede configurar endpoints HTTP locales; usá https://"
                .to_string(),
        );
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("No se permiten credenciales embebidas en la URL".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "La URL no contiene host".to_string())?
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();

    if matches!(
        host.as_str(),
        "metadata.google.internal" | "metadata" | "instance-data" | "kubernetes.default"
    ) || host.ends_with(".metadata.google.internal")
    {
        return Err("Endpoints de metadata cloud prohibidos".to_string());
    }
    if !allow_local
        && (host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || host.ends_with(".internal"))
    {
        return Err("Hosts locales prohibidos para este tenant".to_string());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(v4) => {
                let oct = v4.octets();
                let is_meta = v4.is_unspecified()
                    || v4.is_link_local()
                    || v4.is_multicast()
                    || v4.is_broadcast()
                    || (oct[0] == 100 && (oct[1] & 0xC0) == 64);
                if is_meta || (!allow_local && (v4.is_loopback() || v4.is_private())) {
                    return Err("Dirección IP no permitida para endpoint".to_string());
                }
            }
            IpAddr::V6(v6) => {
                let segs = v6.segments();
                let is_link_local = (segs[0] & 0xffc0) == 0xfe80;
                if v6.is_unspecified()
                    || v6.is_multicast()
                    || is_link_local
                    || (!allow_local && v6.is_loopback())
                {
                    return Err("Dirección IPv6 no permitida para endpoint".to_string());
                }
            }
        }
    }
    Ok(trimmed)
}

pub async fn resolve_provider_key(
    provider: &str,
    tenant_id: &str,
    key_pool: Option<&Arc<KeyPool>>,
) -> Option<String> {
    let pool = key_pool?;
    pool.get_raw_key_for_provider(provider, Some(tenant_id))
        .await
        .map(|entry| entry.api_key)
}

pub fn resolve_elevenlabs_key(db: &TenantDB) -> Option<String> {
    db.get_kv("elevenlabs_api_key")
        .ok()
        .flatten()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub async fn probe_provider_readiness(
    has_elevenlabs_key: bool,
    cfg: &VideoModuleConfig,
    tenant_id: &str,
    data_dir: &Path,
    key_pool: Option<&Arc<KeyPool>>,
) -> ProviderReadiness {
    let has_gemini_key = resolve_provider_key("gemini", tenant_id, key_pool)
        .await
        .is_some();
    let has_openai_key = resolve_provider_key("openai", tenant_id, key_pool)
        .await
        .is_some();
    let has_groq_key = resolve_provider_key("groq", tenant_id, key_pool)
        .await
        .is_some();
    let has_pexels_key = cfg
        .pexels_api_key
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_local_image_url = cfg
        .local_image_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_local_video_url = cfg
        .local_video_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_local_tts_url = cfg
        .local_tts_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let models_dir = data_dir.join("models");
    let has_local_whisper = models_dir.exists()
        && std::fs::read_dir(&models_dir)
            .ok()
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .any(|e| e.file_name().to_string_lossy().ends_with(".bin"))
            })
            .unwrap_or(false);

    ProviderReadiness {
        has_gemini_key,
        has_openai_key,
        has_groq_key,
        has_elevenlabs_key,
        has_pexels_key,
        has_local_image_url,
        has_local_video_url,
        has_local_tts_url,
        has_local_whisper,
    }
}

fn emit_scene_progress(
    cb: &Option<ProgressCallback>,
    project_id: &str,
    scene_id: &str,
    progress: f32,
    message: &str,
) {
    if let Some(callback) = cb {
        callback(VideoProgressEvent {
            project_id: project_id.to_string(),
            stage: "generate_assets".to_string(),
            progress,
            message: message.to_string(),
            scene_id: Some(scene_id.to_string()),
            done: false,
            error: None,
        });
    }
}

/// Wraps raw signed 16-bit little-endian mono PCM audio bytes with a valid 44-byte RIFF/WAV header.
pub fn wrap_pcm16_to_wav(pcm_bytes: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let byte_rate = sample_rate * (channels as u32) * 2;
    let block_align = channels * 2;
    let data_len = pcm_bytes.len() as u32;
    let riff_len = 36u32.saturating_add(data_len);

    let mut out = Vec::with_capacity(44 + pcm_bytes.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm_bytes);
    out
}

fn parse_sample_rate_from_mime(mime: &str) -> u32 {
    for part in mime.split(';') {
        let trimmed = part.trim();
        if let Some(val) = trimmed.strip_prefix("rate=") {
            if let Ok(r) = val.trim().parse::<u32>() {
                if (8_000..=96_000).contains(&r) {
                    return r;
                }
            }
        }
    }
    24_000
}

fn edge_voice_prosody(voice: &str) -> (&'static str, &'static str) {
    match voice {
        "es-AR-TomasNeural" => ("+3%", "-4Hz"),
        "es-AR-ElenaNeural" => ("+2%", "-2Hz"),
        v if v.contains("MultilingualNeural") => ("+3%", "+0Hz"),
        _ => ("+2%", "-1Hz"),
    }
}

async fn synthesize_edge_tts_file(
    voice_id: &str,
    trimmed: &str,
    project_dir: &Path,
    scene_id: &str,
    data_dir: &Path,
) -> Result<Option<String>, String> {
    let (py_exe, _) = resolve_python_for_video(data_dir).await.ok_or_else(|| {
        "Entorno Python no encontrado; instalá el motor automático de Video Studio.".to_string()
    })?;
    let scripts = ensure_helper_scripts(data_dir).await?;
    let script = scripts.join("tts_edge.py");

    let out_name = format!("{}_voice.mp3", scene_id);
    let out_path = project_dir.join(&out_name);
    let words_path = project_dir.join(format!("{}_words.json", scene_id));
    let spec_path = project_dir.join(format!("{}_tts_spec.json", scene_id));

    let chosen_voice = if voice_id.contains("Neural") {
        voice_id
    } else {
        "en-US-AndrewMultilingualNeural"
    };
    let (rate, pitch) = edge_voice_prosody(chosen_voice);
    let spec = json!({
        "text": trimmed,
        "voice": chosen_voice,
        "rate": rate,
        "pitch": pitch,
        "out": out_path.to_string_lossy(),
        "words_out": words_path.to_string_lossy(),
    });
    tokio::fs::write(&spec_path, spec.to_string())
        .await
        .map_err(|e| format!("Failed writing TTS spec: {}", e))?;

    let mut cmd = if py_exe == "py -3" {
        let mut c = Command::new("py");
        c.arg("-3");
        c
    } else {
        Command::new(&py_exe)
    };
    let out = cmd
        .env("PYTHONIOENCODING", "utf-8")
        .arg(&script)
        .arg("--spec")
        .arg(&spec_path)
        .output()
        .await
        .map_err(|e| format!("Error ejecutando Edge TTS: {}", e))?;
    let _ = tokio::fs::remove_file(&spec_path).await;

    if !out.status.success() || !out_path.exists() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!(
            "Edge TTS falló (verificá conexión a internet o instalá el motor): {}",
            err
        ));
    }
    Ok(Some(out_name))
}

async fn synthesize_gemini_tts_file(
    api_key: &str,
    voice_id: &str,
    trimmed: &str,
    project_dir: &Path,
    scene_id: &str,
) -> Result<String, (u16, String)> {
    let gemini_voice = if matches!(
        voice_id,
        "Charon" | "Orus" | "Fenrir" | "Kore" | "Aoede" | "Puck" | "Leda" | "Zephyr"
    ) {
        voice_id
    } else {
        "Charon"
    };
    let client = build_http_client(60).map_err(|e| (500u16, e))?;
    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-preview-tts:generateContent";
    let spoken_prompt = format!(
        "Say the following Spanish script naturally in a warm, clear, energetic modern commercial voiceover tone:\n{}",
        trimmed
    );

    let mut last_status = 500u16;
    let mut last_msg = String::new();

    for attempt in 0..2 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
        let resp = client
            .post(url)
            .header("x-goog-api-key", api_key)
            .json(&json!({
                "contents": [{ "parts": [{ "text": spoken_prompt }] }],
                "generationConfig": {
                    "responseModalities": ["AUDIO"],
                    "speechConfig": {
                        "voiceConfig": {
                            "prebuiltVoiceConfig": {
                                "voiceName": gemini_voice
                            }
                        }
                    }
                }
            }))
            .send()
            .await
            .map_err(|e| (503u16, format!("Error de red Gemini TTS: {}", e)))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            last_status = status;
            last_msg = format!("Gemini TTS HTTP {}: {}", status, body);
            if status == 429 && attempt == 0 {
                continue;
            }
            return Err((last_status, last_msg));
        }

        let val: Value = resp
            .json()
            .await
            .map_err(|e| (502u16, format!("JSON inválido Gemini TTS: {}", e)))?;
        if let Some(parts) = val
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
        {
            for part in parts {
                if let Some(inline) = part.get("inlineData") {
                    if let Some(b64) = inline.get("data").and_then(Value::as_str) {
                        let raw = base64::engine::general_purpose::STANDARD
                            .decode(b64)
                            .map_err(|e| (500u16, format!("Base64 inválido Gemini TTS: {}", e)))?;
                        let mime = inline
                            .get("mimeType")
                            .and_then(Value::as_str)
                            .unwrap_or("audio/L16;codec=pcm;rate=24000")
                            .to_ascii_lowercase();
                        let _ = tokio::fs::remove_file(
                            project_dir.join(format!("{}_words.json", scene_id)),
                        )
                        .await;

                        if mime.contains("mpeg") || mime.contains("mp3") {
                            let out_name = format!("{}_voice.mp3", scene_id);
                            tokio::fs::write(project_dir.join(&out_name), &raw)
                                .await
                                .map_err(|e| {
                                    (500u16, format!("Error guardando MP3 Gemini TTS: {}", e))
                                })?;
                            return Ok(out_name);
                        }
                        let wav_bytes = if mime.contains("wav") || raw.starts_with(b"RIFF") {
                            raw
                        } else {
                            let sr = parse_sample_rate_from_mime(&mime);
                            wrap_pcm16_to_wav(&raw, sr, 1)
                        };
                        let out_name = format!("{}_voice.wav", scene_id);
                        tokio::fs::write(project_dir.join(&out_name), &wav_bytes)
                            .await
                            .map_err(|e| {
                                (500u16, format!("Error guardando WAV Gemini TTS: {}", e))
                            })?;
                        return Ok(out_name);
                    }
                }
            }
        }
        return Err((
            502u16,
            "Gemini TTS no devolvió inlineData de audio".to_string(),
        ));
    }

    Err((last_status, last_msg))
}

/// Generates voice audio file for a single scene cleanly using the selected TTS engine.
#[allow(clippy::too_many_arguments)]
pub async fn generate_scene_voice(
    engine: &str,
    voice_id: &str,
    text: &str,
    project_dir: &Path,
    scene_id: &str,
    data_dir: &Path,
    tenant_id: &str,
    elevenlabs_key: Option<&str>,
    cfg: &VideoModuleConfig,
    key_pool: Option<&Arc<KeyPool>>,
) -> Result<Option<String>, String> {
    let trimmed = text.trim();
    if engine == "none" || trimmed.is_empty() {
        return Ok(None);
    }

    tokio::fs::create_dir_all(project_dir)
        .await
        .map_err(|e| format!("Failed to create project media dir: {}", e))?;

    match engine {
        "edge_tts" => {
            synthesize_edge_tts_file(voice_id, trimmed, project_dir, scene_id, data_dir).await
        }
        "gemini_tts" => {
            let api_key = resolve_provider_key("gemini", tenant_id, key_pool)
                .await
                .ok_or_else(|| {
                    "No tenés una API key de Gemini cargada en Proveedores para usar Gemini TTS Studio."
                        .to_string()
                })?;
            match synthesize_gemini_tts_file(&api_key, voice_id, trimmed, project_dir, scene_id)
                .await
            {
                Ok(fname) => Ok(Some(fname)),
                Err((status, err_msg)) => {
                    // Seamless resilience on free-tier quota or unavailable preview voice endpoints
                    if matches!(status, 400 | 403 | 404 | 429 | 503) {
                        warn!(
                            "Gemini TTS unavailable for scene '{}' ({}), falling back to Edge HD Multilingual",
                            scene_id, err_msg
                        );
                        synthesize_edge_tts_file(
                            "en-US-AndrewMultilingualNeural",
                            trimmed,
                            project_dir,
                            scene_id,
                            data_dir,
                        )
                        .await
                        .map_err(|e| {
                            format!("{} | Fallback Edge TTS también falló: {}", err_msg, e)
                        })
                    } else {
                        Err(err_msg)
                    }
                }
            }
        }
        "openai_tts" => {
            let api_key = resolve_provider_key("openai", tenant_id, key_pool)
                .await
                .ok_or_else(|| {
                    "No tenés una API key de OpenAI configurada en Proveedores para usar OpenAI TTS."
                        .to_string()
                })?;
            let client = build_http_client(45)?;
            let oai_voice = if matches!(
                voice_id,
                "alloy" | "echo" | "fable" | "onyx" | "nova" | "shimmer" | "ash" | "coral" | "sage"
            ) {
                voice_id
            } else {
                "onyx"
            };
            let _ =
                tokio::fs::remove_file(project_dir.join(format!("{}_words.json", scene_id))).await;
            let mut last_err = String::new();
            for model in ["gpt-4o-mini-tts", "tts-1-hd"] {
                let resp = client
                    .post("https://api.openai.com/v1/audio/speech")
                    .bearer_auth(&api_key)
                    .json(&json!({
                        "model": model,
                        "input": trimmed,
                        "voice": oai_voice,
                        "response_format": "mp3"
                    }))
                    .send()
                    .await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let bytes = r
                            .bytes()
                            .await
                            .map_err(|e| format!("Error leyendo audio OpenAI TTS: {}", e))?;
                        let out_name = format!("{}_voice.mp3", scene_id);
                        tokio::fs::write(project_dir.join(&out_name), &bytes)
                            .await
                            .map_err(|e| format!("Error guardando audio OpenAI: {}", e))?;
                        return Ok(Some(out_name));
                    }
                    Ok(r) => {
                        let st = r.status();
                        let body = r.text().await.unwrap_or_default();
                        last_err = format!("OpenAI {} HTTP {}: {}", model, st, body);
                    }
                    Err(e) => {
                        last_err = format!("Error de red OpenAI TTS: {}", e);
                    }
                }
            }
            Err(last_err)
        }
        "elevenlabs" => {
            let api_key = elevenlabs_key
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "No tenés API key de ElevenLabs cargada. Configurala en Ajustes de Voz / Video Studio."
                        .to_string()
                })?
                .to_string();
            let el_voice = if voice_id.trim().is_empty()
                || voice_id.contains("Neural")
                || voice_id == "onyx"
            {
                "EXAVITQu4vr4xnSDxMaL"
            } else {
                voice_id.trim()
            };
            let safe_vid: String = el_voice
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            let client = build_http_client(45)?;
            let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", safe_vid);
            let resp = client
                .post(&url)
                .header("xi-api-key", &api_key)
                .header("Accept", "audio/mpeg")
                .json(&json!({
                    "text": trimmed,
                    "model_id": "eleven_multilingual_v2",
                    "voice_settings": { "stability": 0.5, "similarity_boost": 0.75 }
                }))
                .send()
                .await
                .map_err(|e| format!("Error de red con ElevenLabs: {}", e))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!("ElevenLabs HTTP {}: {}", status, body));
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| format!("Error leyendo audio ElevenLabs: {}", e))?;
            let out_name = format!("{}_voice.mp3", scene_id);
            tokio::fs::write(project_dir.join(&out_name), &bytes)
                .await
                .map_err(|e| format!("Error guardando audio ElevenLabs: {}", e))?;
            Ok(Some(out_name))
        }
        "local_tts" => {
            if let Some(ref base_url) = cfg.local_tts_url {
                let url = validate_local_endpoint_url(base_url, true)?;
                let speech_url = if url.ends_with("/v1") {
                    format!("{}/audio/speech", url)
                } else if url.ends_with("/audio/speech") || url.ends_with("/api/tts") {
                    url
                } else {
                    format!("{}/v1/audio/speech", url)
                };
                let client = build_http_client(45)?;
                let resp = client
                    .post(&speech_url)
                    .json(&json!({
                        "model": "tts-1",
                        "input": trimmed,
                        "voice": voice_id,
                        "response_format": "mp3"
                    }))
                    .send()
                    .await
                    .map_err(|e| {
                        format!("Error conectando con TTS local ({}): {}", speech_url, e)
                    })?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(format!("TTS local devolvió HTTP {}: {}", status, body));
                }
                let ext = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(|ct| if ct.contains("wav") { "wav" } else { "mp3" })
                    .unwrap_or("mp3");
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| format!("Error leyendo bytes de TTS local: {}", e))?;
                let out_name = format!("{}_voice.{}", scene_id, ext);
                tokio::fs::write(project_dir.join(&out_name), &bytes)
                    .await
                    .map_err(|e| format!("Error guardando audio local: {}", e))?;
                return Ok(Some(out_name));
            }

            // Fallback to local espeak-ng / espeak CLI if installed
            let out_name = format!("{}_voice.wav", scene_id);
            let out_path = project_dir.join(&out_name);
            for bin in ["espeak-ng", "espeak"] {
                if let Ok(out) = Command::new(bin)
                    .args(["-v", "es", "-w"])
                    .arg(&out_path)
                    .arg(trimmed)
                    .output()
                    .await
                {
                    if out.status.success() && out_path.exists() {
                        return Ok(Some(out_name));
                    }
                }
            }
            Err("TTS local sin configurar: cargá la URL de tu servidor TTS local (Piper/Kokoro) en Ajustes de Video Studio o elegí Edge TTS (Gratis).".to_string())
        }
        other => Err(format!("Motor de voz desconocido: '{}'", other)),
    }
}

/// Generates or downloads visual media (image or video clip) for one scene.
/// Returns `(filename, "image" | "video")`.
#[allow(clippy::too_many_arguments)]
pub async fn generate_scene_visual(
    engine: &str,
    scene: &VideoScene,
    scene_idx: usize,
    total_scenes: usize,
    aspect_ratio: &str,
    project_id: &str,
    project_dir: &Path,
    data_dir: &Path,
    tenant_id: &str,
    cfg: &VideoModuleConfig,
    key_pool: Option<&Arc<KeyPool>>,
    progress_cb: &Option<ProgressCallback>,
) -> Result<(String, String), String> {
    tokio::fs::create_dir_all(project_dir)
        .await
        .map_err(|e| format!("Failed to create project dir: {}", e))?;

    let (w, h) = aspect_dimensions(aspect_ratio);
    let prompt_text = if scene.visual_prompt.trim().is_empty() {
        scene.title.clone()
    } else {
        scene.visual_prompt.clone()
    };

    #[allow(clippy::too_many_arguments)]
    async fn invoke_render_card_script(
        data_dir: &Path,
        project_dir: &Path,
        scene: &VideoScene,
        scene_idx: usize,
        total_scenes: usize,
        w: u32,
        h: u32,
        mode: &str,
        out_name: &str,
    ) -> Result<(), String> {
        let (py_exe, _) = resolve_python_for_video(data_dir).await.ok_or_else(|| {
            "Entorno Python no encontrado; ejecutá el instalador automático de Video Studio."
                .to_string()
        })?;
        let scripts = ensure_helper_scripts(data_dir).await?;
        let script = scripts.join("render_card.py");
        let out_path = project_dir.join(out_name);
        let spec_path = project_dir.join(format!("{}_{}_spec.json", scene.id, mode));

        let badge = if total_scenes > 1 {
            format!("{:02} / {:02} · AEGIS", scene_idx + 1, total_scenes)
        } else {
            "AEGIS STUDIO".to_string()
        };
        let headline = if scene.on_screen_text.trim().is_empty() {
            scene.title.trim()
        } else {
            scene.on_screen_text.trim()
        };
        let spec = json!({
            "width": w,
            "height": h,
            "mode": mode,
            "scene_idx": scene_idx,
            "total_scenes": total_scenes.max(1),
            "badge": badge,
            "title": scene.title,
            "text": headline,
            "visual_prompt": scene.visual_prompt,
            "out": out_path.to_string_lossy(),
        });
        tokio::fs::write(&spec_path, spec.to_string())
            .await
            .map_err(|e| format!("Failed writing card spec: {}", e))?;

        let mut cmd = if py_exe == "py -3" {
            let mut c = Command::new("py");
            c.arg("-3");
            c
        } else {
            Command::new(&py_exe)
        };
        let out = cmd
            .env("PYTHONIOENCODING", "utf-8")
            .arg(&script)
            .arg("--spec")
            .arg(&spec_path)
            .output()
            .await
            .map_err(|e| format!("Error generando placa animada: {}", e))?;
        let _ = tokio::fs::remove_file(&spec_path).await;

        if !out.status.success() || !out_path.exists() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(format!("Fallo generando placa gráfica: {}", err));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn maybe_write_hud_overlay(
        result: (String, String),
        data_dir: &Path,
        project_dir: &Path,
        scene: &VideoScene,
        scene_idx: usize,
        total_scenes: usize,
        w: u32,
        h: u32,
    ) -> (String, String) {
        let overlay_path = project_dir.join(format!("{}_overlay.png", scene.id));
        if result.1 == "image" {
            let overlay_name = format!("{}_overlay.png", scene.id);
            let _ = invoke_render_card_script(
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
                "overlay",
                &overlay_name,
            )
            .await;
        } else {
            let _ = tokio::fs::remove_file(&overlay_path).await;
        }
        result
    }

    match engine {
        "kinetic_card" => {
            let out_name = format!("{}_visual.png", scene.id);
            let _ =
                tokio::fs::remove_file(project_dir.join(format!("{}_overlay.png", scene.id))).await;
            invoke_render_card_script(
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
                "full",
                &out_name,
            )
            .await?;
            Ok((out_name, "image".to_string()))
        }
        "pollinations_ai" => {
            let res = generate_with_pollinations_ai(
                &prompt_text,
                aspect_ratio,
                scene_idx,
                project_dir,
                &scene.id,
            )
            .await?;
            Ok(maybe_write_hud_overlay(
                res,
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
            )
            .await)
        }
        "pexels_stock" | "pixabay_stock" => {
            let api_key = cfg
                .pexels_api_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "Configurá tu API key gratuita de Pixabay (o Pexels) en Claves & IA Local de Video Studio para usar Stock HD."
                        .to_string()
                })?;
            let query = if scene.stock_keywords.trim().is_empty() {
                scene.title.trim()
            } else {
                scene.stock_keywords.trim()
            };
            let is_pixabay = engine == "pixabay_stock"
                || api_key
                    .split_once('-')
                    .map(|(l, r)| {
                        !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()) && !r.is_empty()
                    })
                    .unwrap_or(false);
            if is_pixabay {
                let res = fetch_pixabay_stock_media(
                    api_key,
                    query,
                    aspect_ratio,
                    scene_idx,
                    project_dir,
                    &scene.id,
                )
                .await?;
                return Ok(maybe_write_hud_overlay(
                    res,
                    data_dir,
                    project_dir,
                    scene,
                    scene_idx,
                    total_scenes,
                    w,
                    h,
                )
                .await);
            }
            let orientation = match aspect_ratio {
                "16:9" => "landscape",
                "1:1" => "square",
                _ => "portrait",
            };
            let client = build_http_client(45)?;
            let search_url = format!(
                "https://api.pexels.com/videos/search?query={}&orientation={}&per_page=3",
                urlencoding::encode(query),
                orientation
            );
            let resp = client
                .get(&search_url)
                .header("Authorization", api_key)
                .send()
                .await
                .map_err(|e| format!("Error buscando stock en Pexels: {}", e))?;
            if !resp.status().is_success() {
                return Err(format!("Pexels API HTTP {}", resp.status()));
            }
            let body: Value = resp
                .json()
                .await
                .map_err(|e| format!("Respuesta inválida de Pexels: {}", e))?;

            let pick_idx = scene_idx % 3;
            let videos = body.get("videos").and_then(Value::as_array);
            let video_url = videos
                .and_then(|arr| arr.get(pick_idx).or_else(|| arr.first()))
                .and_then(|v| v.get("video_files").and_then(Value::as_array))
                .and_then(|files| {
                    files
                        .iter()
                        .find(|f| {
                            f.get("file_type").and_then(Value::as_str) == Some("video/mp4")
                                && f.get("width").and_then(Value::as_u64).unwrap_or(0) >= 720
                        })
                        .or_else(|| files.first())
                })
                .and_then(|f| f.get("link").and_then(Value::as_str))
                .map(str::to_string);

            if let Some(mp4_url) = video_url {
                let bytes = download_bounded_media(&client, &mp4_url, None).await?;
                let out_name = format!("{}_visual.mp4", scene.id);
                let _ =
                    tokio::fs::remove_file(project_dir.join(format!("{}_overlay.png", scene.id)))
                        .await;
                tokio::fs::write(project_dir.join(&out_name), &bytes)
                    .await
                    .map_err(|e| format!("Error guardando clip de Pexels: {}", e))?;
                return Ok((out_name, "video".to_string()));
            }

            // Photo fallback on Pexels
            let photo_url = format!(
                "https://api.pexels.com/v1/search?query={}&orientation={}&per_page=3",
                urlencoding::encode(query),
                orientation
            );
            let p_resp = client
                .get(&photo_url)
                .header("Authorization", api_key)
                .send()
                .await
                .map_err(|e| format!("Error buscando foto en Pexels: {}", e))?;
            let p_body: Value = p_resp.json().await.unwrap_or(Value::Null);
            let img_url = p_body
                .get("photos")
                .and_then(Value::as_array)
                .and_then(|arr| arr.get(pick_idx).or_else(|| arr.first()))
                .and_then(|p| p.get("src"))
                .and_then(|src| {
                    src.get("large2x")
                        .or_else(|| src.get("large"))
                        .and_then(Value::as_str)
                })
                .ok_or_else(|| {
                    format!(
                        "Pexels no encontró clips ni fotos para '{}'; editá los keywords de la escena.",
                        query
                    )
                })?;
            let bytes = download_bounded_media(&client, img_url, None).await?;
            let out_name = format!("{}_visual.jpg", scene.id);
            tokio::fs::write(project_dir.join(&out_name), &bytes)
                .await
                .map_err(|e| format!("Error guardando foto Pexels: {}", e))?;
            Ok(maybe_write_hud_overlay(
                (out_name, "image".to_string()),
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
            )
            .await)
        }
        "gemini_imagen" => {
            let api_key = resolve_provider_key("gemini", tenant_id, key_pool)
                .await
                .ok_or_else(|| {
                    "No tenés una API key de Gemini cargada en Proveedores para generar imágenes."
                        .to_string()
                })?;
            let res = generate_with_gemini_imagen(
                &api_key,
                &prompt_text,
                aspect_ratio,
                project_dir,
                &scene.id,
            )
            .await?;
            Ok(maybe_write_hud_overlay(
                res,
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
            )
            .await)
        }
        "openai_image" => {
            let api_key = resolve_provider_key("openai", tenant_id, key_pool)
                .await
                .ok_or_else(|| {
                    "No tenés una API key de OpenAI cargada en Proveedores para OpenAI Images."
                        .to_string()
                })?;
            let res = generate_with_openai_image(
                &api_key,
                &prompt_text,
                aspect_ratio,
                project_dir,
                &scene.id,
            )
            .await?;
            Ok(maybe_write_hud_overlay(
                res,
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
            )
            .await)
        }
        "google_veo" => {
            let api_key = resolve_provider_key("gemini", tenant_id, key_pool)
                .await
                .ok_or_else(|| {
                    "No tenés una API key de Gemini cargada en Proveedores para usar Google Veo."
                        .to_string()
                })?;
            emit_scene_progress(
                progress_cb,
                project_id,
                &scene.id,
                0.25,
                &format!(
                    "Generando clip con Google Veo para '{}' (puede demorar 30-90s)...",
                    scene.title
                ),
            );
            match generate_with_google_veo(
                &api_key,
                &prompt_text,
                aspect_ratio,
                project_id,
                &scene.id,
                project_dir,
                progress_cb,
            )
            .await
            {
                Ok(res) => {
                    let _ = tokio::fs::remove_file(
                        project_dir.join(format!("{}_overlay.png", scene.id)),
                    )
                    .await;
                    Ok(res)
                }
                Err(veo_err) => {
                    if veo_err.contains("HTTP 400")
                        || veo_err.contains("HTTP 403")
                        || veo_err.contains("HTTP 404")
                    {
                        warn!(
                            "Google Veo unavailable for scene '{}' ({}); falling back to Gemini Imagen",
                            scene.id, veo_err
                        );
                        let res = generate_with_gemini_imagen(
                            &api_key,
                            &prompt_text,
                            aspect_ratio,
                            project_dir,
                            &scene.id,
                        )
                        .await?;
                        Ok(maybe_write_hud_overlay(
                            res,
                            data_dir,
                            project_dir,
                            scene,
                            scene_idx,
                            total_scenes,
                            w,
                            h,
                        )
                        .await)
                    } else {
                        Err(veo_err)
                    }
                }
            }
        }
        "local_ai" => {
            let local_url = cfg
                .local_image_url
                .as_deref()
                .or(cfg.local_video_url.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "No configuraste una URL de IA Local (ej. ComfyUI http://127.0.0.1:8188 o Automatic1111 http://127.0.0.1:7860) en Ajustes de Video Studio."
                        .to_string()
                })?;
            let validated = validate_local_endpoint_url(local_url, true)?;
            let mode = cfg.local_image_mode.as_deref().unwrap_or("auto");
            let res = generate_with_local_ai(
                &validated,
                mode,
                &prompt_text,
                w,
                h,
                project_dir,
                &scene.id,
            )
            .await?;
            Ok(maybe_write_hud_overlay(
                res,
                data_dir,
                project_dir,
                scene,
                scene_idx,
                total_scenes,
                w,
                h,
            )
            .await)
        }
        other => Err(format!("Motor visual desconocido: '{}'", other)),
    }
}

async fn download_bounded_media(
    client: &Client,
    url: &str,
    google_api_key: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut req = client.get(url);
    if let Some(k) = google_api_key {
        req = req.header("x-goog-api-key", k);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Error descargando media ({}): {}", url, e))?;
    if !resp.status().is_success() {
        return Err(format!("Error HTTP descargando media: {}", resp.status()));
    }
    let mut buf = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("Error leyendo stream multimedia: {}", e))?;
        if buf.len() + bytes.len() > MAX_MEDIA_DOWNLOAD_BYTES {
            return Err("El archivo multimedia excede el límite de 90 MB".to_string());
        }
        buf.extend_from_slice(&bytes);
    }
    if buf.is_empty() {
        return Err("El servidor devolvió un archivo multimedia vacío".to_string());
    }
    Ok(buf)
}

async fn generate_with_pollinations_ai(
    prompt_text: &str,
    aspect_ratio: &str,
    scene_idx: usize,
    project_dir: &Path,
    scene_id: &str,
) -> Result<(String, String), String> {
    let (req_w, req_h) = match aspect_ratio {
        "16:9" => (1280u32, 720u32),
        "1:1" => (1024u32, 1024u32),
        _ => (720u32, 1280u32),
    };
    let short_prompt: String = prompt_text.trim().chars().take(280).collect();
    let full_prompt = format!(
        "{}, sleek dark-mode commercial aesthetic, high contrast cinematic lighting, no text, no watermark",
        if short_prompt.is_empty() {
            "futuristic neon technology workspace"
        } else {
            short_prompt.as_str()
        }
    );
    let seed = 1000u32 + (scene_idx as u32) * 137;
    let encoded = urlencoding::encode(&full_prompt);
    let urls = [
        format!(
            "https://image.pollinations.ai/prompt/{}?width={}&height={}&nologo=true&seed={}&model=flux",
            encoded, req_w, req_h, seed
        ),
        format!(
            "https://image.pollinations.ai/prompt/{}?width={}&height={}&nologo=true&seed={}",
            encoded, req_w, req_h, seed
        ),
    ];

    let client = build_http_client(40)?;
    let mut last_err = String::from("Pollinations AI no respondió");

    for url in urls {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let ctype = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !ctype.is_empty() && !ctype.starts_with("image/") {
                    last_err = format!("Pollinations devolvió tipo inesperado: {}", ctype);
                    continue;
                }
                match resp.bytes().await {
                    Ok(bytes) if bytes.len() >= 4096 => {
                        let out_name = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                            format!("{}_visual.png", scene_id)
                        } else {
                            format!("{}_visual.jpg", scene_id)
                        };
                        tokio::fs::write(project_dir.join(&out_name), &bytes)
                            .await
                            .map_err(|e| {
                                format!("Error guardando imagen de Pollinations: {}", e)
                            })?;
                        return Ok((out_name, "image".to_string()));
                    }
                    Ok(bytes) => {
                        last_err = format!(
                            "Imagen Pollinations demasiado pequeña ({} bytes)",
                            bytes.len()
                        );
                    }
                    Err(e) => {
                        last_err = format!("Error leyendo imagen Pollinations: {}", e);
                    }
                }
            }
            Ok(resp) => {
                last_err = format!("Pollinations AI HTTP {}", resp.status());
            }
            Err(e) => {
                last_err = format!("Error conectando a Pollinations AI: {}", e);
            }
        }
    }

    Err(last_err)
}

async fn fetch_pixabay_stock_media(
    api_key: &str,
    query: &str,
    aspect_ratio: &str,
    scene_idx: usize,
    project_dir: &Path,
    scene_id: &str,
) -> Result<(String, String), String> {
    let client = build_http_client(45)?;
    let orientation = match aspect_ratio {
        "16:9" => "horizontal",
        _ => "vertical",
    };

    let primary_q: String = query.trim().chars().take(80).collect();
    let mut queries = Vec::new();
    if !primary_q.is_empty() {
        queries.push(primary_q);
    }
    if queries
        .first()
        .map(|s| !s.eq_ignore_ascii_case("technology abstract"))
        .unwrap_or(true)
    {
        queries.push("technology abstract".to_string());
    }

    for q in &queries {
        let video_url = format!(
            "https://pixabay.com/api/videos/?key={}&q={}&per_page=5&safesearch=true",
            urlencoding::encode(api_key),
            urlencoding::encode(q)
        );
        let v_resp = client
            .get(&video_url)
            .send()
            .await
            .map_err(|e| format!("Error buscando video en Pixabay: {}", e))?;
        if !v_resp.status().is_success() {
            return Err(format!("Pixabay API HTTP {}", v_resp.status()));
        }
        let v_body: Value = v_resp
            .json()
            .await
            .map_err(|e| format!("Respuesta inválida de Pixabay: {}", e))?;
        if let Some(hits) = v_body.get("hits").and_then(Value::as_array) {
            if !hits.is_empty() {
                let chosen = &hits[scene_idx % hits.len()];
                let mp4_url = chosen
                    .get("videos")
                    .and_then(|v| {
                        v.get("medium")
                            .or_else(|| v.get("large"))
                            .or_else(|| v.get("small"))
                    })
                    .and_then(|tier| tier.get("url"))
                    .and_then(Value::as_str);
                if let Some(url_str) = mp4_url.filter(|s| !s.trim().is_empty()) {
                    let bytes = download_bounded_media(&client, url_str, None).await?;
                    let out_name = format!("{}_visual.mp4", scene_id);
                    let _ = tokio::fs::remove_file(
                        project_dir.join(format!("{}_overlay.png", scene_id)),
                    )
                    .await;
                    tokio::fs::write(project_dir.join(&out_name), &bytes)
                        .await
                        .map_err(|e| format!("Error guardando clip de Pixabay: {}", e))?;
                    return Ok((out_name, "video".to_string()));
                }
            }
        }

        // Photo fallback on Pixabay
        let photo_url = format!(
            "https://pixabay.com/api/?key={}&q={}&orientation={}&image_type=photo&per_page=5&safesearch=true",
            urlencoding::encode(api_key),
            urlencoding::encode(q),
            orientation
        );
        let p_resp = client
            .get(&photo_url)
            .send()
            .await
            .map_err(|e| format!("Error buscando foto en Pixabay: {}", e))?;
        if !p_resp.status().is_success() {
            return Err(format!("Pixabay API HTTP {}", p_resp.status()));
        }
        let p_body: Value = p_resp.json().await.unwrap_or(Value::Null);
        if let Some(hits) = p_body.get("hits").and_then(Value::as_array) {
            if !hits.is_empty() {
                let chosen = &hits[scene_idx % hits.len()];
                let img_url = chosen
                    .get("largeImageURL")
                    .or_else(|| chosen.get("webformatURL"))
                    .and_then(Value::as_str);
                if let Some(url_str) = img_url.filter(|s| !s.trim().is_empty()) {
                    let bytes = download_bounded_media(&client, url_str, None).await?;
                    let out_name = format!("{}_visual.jpg", scene_id);
                    tokio::fs::write(project_dir.join(&out_name), &bytes)
                        .await
                        .map_err(|e| format!("Error guardando foto de Pixabay: {}", e))?;
                    return Ok((out_name, "image".to_string()));
                }
            }
        }
    }

    Err(format!(
        "Pixabay no encontró clips ni fotos para '{}'; editá los keywords de la escena.",
        query
    ))
}

async fn generate_with_gemini_imagen(
    api_key: &str,
    prompt: &str,
    aspect_ratio: &str,
    project_dir: &Path,
    scene_id: &str,
) -> Result<(String, String), String> {
    let client = build_http_client(65)?;
    let imagen_aspect = match aspect_ratio {
        "16:9" => "16:9",
        "1:1" => "1:1",
        _ => "9:16",
    };
    let clean_art_prompt = format!(
        "Cinematic dark-mode modern commercial 3D/editorial visual artwork, dramatic studio volumetric lighting, \
        clean dark negative space near top and bottom margins for crisp text overlay, strict NO TEXT, NO LETTERS, NO WORDS, NO WATERMARK: {}",
        prompt
    );

    for model in ["imagen-4.0-generate-001", "imagen-3.0-generate-002"] {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:predict",
            model
        );
        let resp = client
            .post(&url)
            .header("x-goog-api-key", api_key)
            .json(&json!({
                "instances": [{ "prompt": clean_art_prompt }],
                "parameters": {
                    "sampleCount": 1,
                    "aspectRatio": imagen_aspect
                }
            }))
            .send()
            .await;

        if let Ok(r) = resp {
            if r.status().is_success() {
                if let Ok(val) = r.json::<Value>().await {
                    if let Some(b64) = val
                        .get("predictions")
                        .and_then(Value::as_array)
                        .and_then(|arr| arr.first())
                        .and_then(|p| p.get("bytesBase64Encoded"))
                        .and_then(Value::as_str)
                    {
                        let decoded = base64::engine::general_purpose::STANDARD
                            .decode(b64)
                            .map_err(|e| format!("Error decodificando Imagen Base64: {}", e))?;
                        let out_name = format!("{}_visual.png", scene_id);
                        tokio::fs::write(project_dir.join(&out_name), &decoded)
                            .await
                            .map_err(|e| format!("Error guardando imagen Gemini: {}", e))?;
                        return Ok((out_name, "image".to_string()));
                    }
                }
            } else {
                let code = r.status();
                let txt = r.text().await.unwrap_or_default();
                warn!("Gemini Imagen {} returned {}: {}", model, code, txt);
            }
        }
    }

    // Fallback to Gemini native image generation models with explicit aspectRatio
    let mut last_err = String::from("No se pudo generar imagen con los modelos Gemini disponibles");
    for model in [
        "gemini-2.5-flash-image",
        "gemini-3-pro-image-preview",
        "gemini-2.0-flash-exp-image-generation",
    ] {
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model
            );
            let resp = client
                .post(&url)
                .header("x-goog-api-key", api_key)
                .json(&json!({
                    "contents": [{ "parts": [{ "text": clean_art_prompt }] }],
                    "generationConfig": {
                        "responseModalities": ["IMAGE", "TEXT"],
                        "imageConfig": { "aspectRatio": imagen_aspect }
                    }
                }))
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    if let Ok(val) = r.json::<Value>().await {
                        if let Some(parts) = val
                            .get("candidates")
                            .and_then(Value::as_array)
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("content"))
                            .and_then(|c| c.get("parts"))
                            .and_then(Value::as_array)
                        {
                            for part in parts {
                                if let Some(inline) = part.get("inlineData") {
                                    if let Some(b64) = inline.get("data").and_then(Value::as_str) {
                                        let mime = inline
                                            .get("mimeType")
                                            .and_then(Value::as_str)
                                            .unwrap_or("image/png");
                                        let ext = if mime.contains("jpeg") || mime.contains("jpg") {
                                            "jpg"
                                        } else {
                                            "png"
                                        };
                                        let decoded = base64::engine::general_purpose::STANDARD
                                            .decode(b64)
                                            .map_err(|e| {
                                                format!("Error base64 Gemini Image: {}", e)
                                            })?;
                                        let out_name = format!("{}_visual.{}", scene_id, ext);
                                        tokio::fs::write(project_dir.join(&out_name), &decoded)
                                            .await
                                            .map_err(|e| {
                                                format!("Error guardando imagen: {}", e)
                                            })?;
                                        return Ok((out_name, "image".to_string()));
                                    }
                                }
                            }
                        }
                    }
                    break;
                }
                Ok(r) => {
                    let code = r.status();
                    let txt = r.text().await.unwrap_or_default();
                    last_err = format!("Gemini Imagen/Image HTTP {}: {}", code, txt);
                    if code.as_u16() == 429 && attempt == 0 {
                        continue;
                    }
                    break;
                }
                Err(e) => {
                    last_err = format!("Error de red Gemini Imagen: {}", e);
                    break;
                }
            }
        }
    }

    Err(last_err)
}

async fn generate_with_openai_image(
    api_key: &str,
    prompt: &str,
    aspect_ratio: &str,
    project_dir: &Path,
    scene_id: &str,
) -> Result<(String, String), String> {
    let client = build_http_client(75)?;
    let (gpt_size, dalle_size) = match aspect_ratio {
        "16:9" => ("1536x1024", "1792x1024"),
        "1:1" => ("1024x1024", "1024x1024"),
        _ => ("1024x1536", "1024x1792"),
    };

    let mut last_err = String::new();
    for (model, size) in [("gpt-image-1", gpt_size), ("dall-e-3", dalle_size)] {
        let resp = client
            .post("https://api.openai.com/v1/images/generations")
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "prompt": prompt,
                "n": 1,
                "size": size
            }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let val: Value = r
                    .json()
                    .await
                    .map_err(|e| format!("Respuesta JSON inválida OpenAI Image: {}", e))?;
                if let Some(first) = val
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|arr| arr.first())
                {
                    if let Some(b64) = first.get("b64_json").and_then(Value::as_str) {
                        let decoded = base64::engine::general_purpose::STANDARD
                            .decode(b64)
                            .map_err(|e| format!("Error decodificando imagen OpenAI: {}", e))?;
                        let out_name = format!("{}_visual.png", scene_id);
                        tokio::fs::write(project_dir.join(&out_name), &decoded)
                            .await
                            .map_err(|e| format!("Error guardando imagen OpenAI: {}", e))?;
                        return Ok((out_name, "image".to_string()));
                    }
                    if let Some(url) = first.get("url").and_then(Value::as_str) {
                        let bytes = download_bounded_media(&client, url, None).await?;
                        let out_name = format!("{}_visual.png", scene_id);
                        tokio::fs::write(project_dir.join(&out_name), &bytes)
                            .await
                            .map_err(|e| format!("Error guardando imagen OpenAI: {}", e))?;
                        return Ok((out_name, "image".to_string()));
                    }
                }
            }
            Ok(r) => {
                let st = r.status();
                let txt = r.text().await.unwrap_or_default();
                last_err = format!("OpenAI {} HTTP {}: {}", model, st, txt);
            }
            Err(e) => {
                last_err = format!("Error de red OpenAI Image: {}", e);
            }
        }
    }

    Err(last_err)
}

async fn generate_with_google_veo(
    api_key: &str,
    prompt: &str,
    aspect_ratio: &str,
    project_id: &str,
    scene_id: &str,
    project_dir: &Path,
    progress_cb: &Option<ProgressCallback>,
) -> Result<(String, String), String> {
    let client = build_http_client(60)?;
    let veo_aspect = match aspect_ratio {
        "16:9" => "16:9",
        _ => "9:16",
    };

    let mut op_name: Option<String> = None;
    let mut start_err = String::new();

    for model in ["veo-2.0-generate-001", "veo-3.0-generate-preview"] {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:predictLongRunning",
            model
        );
        let resp = client
            .post(&url)
            .header("x-goog-api-key", api_key)
            .json(&json!({
                "instances": [{ "prompt": prompt }],
                "parameters": {
                    "aspectRatio": veo_aspect,
                    "personGeneration": "allow_adult"
                }
            }))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let val: Value = r.json().await.unwrap_or(Value::Null);
                if let Some(name) = val.get("name").and_then(Value::as_str) {
                    op_name = Some(name.to_string());
                    info!("Started Google Veo job '{}' using model {}", name, model);
                    break;
                }
            }
            Ok(r) => {
                let st = r.status();
                let body = r.text().await.unwrap_or_default();
                start_err = format!(
                    "Google Veo ({}) devolvió HTTP {}. Verificá que tu clave Gemini tenga facturación/acceso a Veo habilitado (si no, podés usar Gemini Imagen, Stock o Placas Animadas). Detalle: {}",
                    model, st, body
                );
            }
            Err(e) => {
                start_err = format!("Error iniciando Google Veo: {}", e);
            }
        }
    }

    let op = op_name.ok_or(start_err)?;
    let poll_url = format!("https://generativelanguage.googleapis.com/v1beta/{}", op);

    for attempt in 0..45 {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let pct = 0.25 + ((attempt as f32 / 45.0) * 0.55);
        emit_scene_progress(
            progress_cb,
            project_id,
            scene_id,
            pct,
            &format!("Renderizando video en Google Veo ({}/45)...", attempt + 1),
        );

        let r = client
            .get(&poll_url)
            .header("x-goog-api-key", api_key)
            .send()
            .await
            .map_err(|e| format!("Error consultando estado de Google Veo: {}", e))?;
        let val: Value = r
            .json()
            .await
            .map_err(|e| format!("Respuesta inválida al consultar Google Veo: {}", e))?;

        if val.get("done").and_then(Value::as_bool) == Some(true) {
            if let Some(err) = val.get("error") {
                return Err(format!("Google Veo falló: {}", err));
            }
            let uri = val
                .pointer("/response/generateVideoResponse/generatedSamples/0/video/uri")
                .or_else(|| val.pointer("/response/generatedVideos/0/video/uri"))
                .and_then(Value::as_str)
                .ok_or_else(|| format!("Google Veo completó sin URI de video: {}", val))?;

            let bytes = download_bounded_media(&client, uri, Some(api_key)).await?;
            let out_name = format!("{}_visual.mp4", scene_id);
            tokio::fs::write(project_dir.join(&out_name), &bytes)
                .await
                .map_err(|e| format!("Error guardando MP4 de Google Veo: {}", e))?;
            return Ok((out_name, "video".to_string()));
        }
    }

    Err("Timeout esperando respuesta de Google Veo (>3 minutos).".to_string())
}

/// Supports local GPU inference backends cleanly:
/// - `comfyui` (or port 8188): /object_info/CheckpointLoaderSimple + /prompt + /history + /view
/// - `a1111` (or port 7860): /sdapi/v1/txt2img
/// - `openai_image`: /v1/images/generations
pub async fn generate_with_local_ai(
    base_url: &str,
    mode: &str,
    prompt: &str,
    width: u32,
    height: u32,
    project_dir: &Path,
    scene_id: &str,
) -> Result<(String, String), String> {
    let clean_base = base_url.trim_end_matches('/');
    let resolved_mode = if mode == "auto" {
        if clean_base.contains(":8188") || clean_base.contains("comfy") {
            "comfyui"
        } else if clean_base.contains(":7860") || clean_base.contains("sdapi") {
            "a1111"
        } else {
            "openai_image"
        }
    } else {
        mode
    };

    let client = build_http_client(120)?;
    let snap_w = ((width.min(1024) / 64).max(8)) * 64;
    let snap_h = ((height.min(1024) / 64).max(8)) * 64;

    match resolved_mode {
        "a1111" => {
            let resp = client
                .post(format!("{}/sdapi/v1/txt2img", clean_base))
                .json(&json!({
                    "prompt": prompt,
                    "negative_prompt": "blurry, watermark, low quality, deformed text",
                    "width": snap_w,
                    "height": snap_h,
                    "steps": 20,
                    "cfg_scale": 7.0
                }))
                .send()
                .await
                .map_err(|e| {
                    format!(
                        "No se pudo conectar con IA local Automatic1111 ({}): {}",
                        clean_base, e
                    )
                })?;
            if !resp.status().is_success() {
                return Err(format!("Automatic1111 devolvió HTTP {}", resp.status()));
            }
            let val: Value = resp
                .json()
                .await
                .map_err(|e| format!("JSON inválido desde Automatic1111: {}", e))?;
            let b64 = val
                .get("images")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
                .ok_or_else(|| "Automatic1111 no devolvió imágenes".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| format!("Error decodificando imagen local: {}", e))?;
            let out_name = format!("{}_visual.png", scene_id);
            tokio::fs::write(project_dir.join(&out_name), &bytes)
                .await
                .map_err(|e| format!("Error guardando imagen local: {}", e))?;
            Ok((out_name, "image".to_string()))
        }
        "comfyui" => {
            // Auto-discover first checkpoint loaded in ComfyUI
            let ckpt_resp = client
                .get(format!("{}/object_info/CheckpointLoaderSimple", clean_base))
                .send()
                .await
                .map_err(|e| format!("No se pudo conectar con ComfyUI en {}: {}", clean_base, e))?;
            if !ckpt_resp.status().is_success() {
                return Err(format!("ComfyUI devolvió HTTP {}", ckpt_resp.status()));
            }
            let ckpt_json: Value = ckpt_resp.json().await.unwrap_or(Value::Null);
            let ckpt_name = ckpt_json
                .pointer("/CheckpointLoaderSimple/input/required/ckpt_name/0/0")
                .and_then(Value::as_str)
                .unwrap_or("v1-5-pruned-emaonly.safetensors")
                .to_string();

            let seed = (uuid::Uuid::new_v4().as_u128() & 0x7FFF_FFFF) as u64;
            let workflow = json!({
                "3": {
                    "inputs": {
                        "seed": seed,
                        "steps": 20,
                        "cfg": 7.5,
                        "sampler_name": "euler",
                        "scheduler": "normal",
                        "denoise": 1,
                        "model": ["4", 0],
                        "positive": ["6", 0],
                        "negative": ["7", 0],
                        "latent_image": ["5", 0]
                    },
                    "class_type": "KSampler"
                },
                "4": {
                    "inputs": { "ckpt_name": ckpt_name },
                    "class_type": "CheckpointLoaderSimple"
                },
                "5": {
                    "inputs": { "width": snap_w, "height": snap_h, "batch_size": 1 },
                    "class_type": "EmptyLatentImage"
                },
                "6": {
                    "inputs": { "text": prompt, "clip": ["4", 1] },
                    "class_type": "CLIPTextEncode"
                },
                "7": {
                    "inputs": { "text": "blurry, lowres, watermark, ugly text", "clip": ["4", 1] },
                    "class_type": "CLIPTextEncode"
                },
                "8": {
                    "inputs": { "samples": ["3", 0], "vae": ["4", 2] },
                    "class_type": "VAEDecode"
                },
                "9": {
                    "inputs": { "filename_prefix": format!("aegis_{}", scene_id), "images": ["8", 0] },
                    "class_type": "SaveImage"
                }
            });

            let submit = client
                .post(format!("{}/prompt", clean_base))
                .json(&json!({ "prompt": workflow }))
                .send()
                .await
                .map_err(|e| format!("Error enviando workflow a ComfyUI: {}", e))?;
            if !submit.status().is_success() {
                return Err(format!("ComfyUI /prompt HTTP {}", submit.status()));
            }
            let submit_json: Value = submit
                .json()
                .await
                .map_err(|e| format!("JSON inválido de ComfyUI /prompt: {}", e))?;
            let prompt_id = submit_json
                .get("prompt_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "ComfyUI no devolvió prompt_id".to_string())?
                .to_string();

            for _ in 0..45 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let hist_r = client
                    .get(format!("{}/history/{}", clean_base, prompt_id))
                    .send()
                    .await
                    .map_err(|e| format!("Error consultando historial ComfyUI: {}", e))?;
                let hist_val: Value = hist_r.json().await.unwrap_or(Value::Null);
                if let Some(images) = hist_val
                    .pointer(&format!("/{}/outputs/9/images", prompt_id))
                    .and_then(Value::as_array)
                {
                    if let Some(first) = images.first() {
                        let filename = first.get("filename").and_then(Value::as_str).unwrap_or("");
                        let subfolder =
                            first.get("subfolder").and_then(Value::as_str).unwrap_or("");
                        let img_type = first
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("output");
                        if !filename.is_empty() {
                            let view_url = format!(
                                "{}/view?filename={}&subfolder={}&type={}",
                                clean_base,
                                urlencoding::encode(filename),
                                urlencoding::encode(subfolder),
                                urlencoding::encode(img_type)
                            );
                            let bytes = download_bounded_media(&client, &view_url, None).await?;
                            let out_name = format!("{}_visual.png", scene_id);
                            tokio::fs::write(project_dir.join(&out_name), &bytes)
                                .await
                                .map_err(|e| format!("Error guardando imagen ComfyUI: {}", e))?;
                            return Ok((out_name, "image".to_string()));
                        }
                    }
                }
            }
            Err("Timeout esperando imagen de ComfyUI local".to_string())
        }
        _ => {
            // OpenAI-compatible local image endpoint
            let endpoint = if clean_base.ends_with("/v1") {
                format!("{}/images/generations", clean_base)
            } else if clean_base.ends_with("/images/generations") {
                clean_base.to_string()
            } else {
                format!("{}/v1/images/generations", clean_base)
            };
            let resp = client
                .post(&endpoint)
                .json(&json!({
                    "prompt": prompt,
                    "n": 1,
                    "size": format!("{}x{}", snap_w, snap_h)
                }))
                .send()
                .await
                .map_err(|e| {
                    format!(
                        "No se pudo conectar con endpoint local ({}): {}",
                        endpoint, e
                    )
                })?;
            if !resp.status().is_success() {
                return Err(format!("Endpoint local devolvió HTTP {}", resp.status()));
            }
            let val: Value = resp
                .json()
                .await
                .map_err(|e| format!("JSON inválido desde endpoint local: {}", e))?;
            if let Some(first) = val
                .get("data")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
            {
                if let Some(b64) = first.get("b64_json").and_then(Value::as_str) {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(b64)
                        .map_err(|e| format!("Base64 inválido desde endpoint local: {}", e))?;
                    let out_name = format!("{}_visual.png", scene_id);
                    tokio::fs::write(project_dir.join(&out_name), &bytes)
                        .await
                        .map_err(|e| format!("Error guardando imagen local: {}", e))?;
                    return Ok((out_name, "image".to_string()));
                }
                if let Some(url) = first.get("url").and_then(Value::as_str) {
                    let validated_img_url = validate_local_endpoint_url(url, true)?;
                    let bytes = download_bounded_media(&client, &validated_img_url, None).await?;
                    let out_name = format!("{}_visual.png", scene_id);
                    tokio::fs::write(project_dir.join(&out_name), &bytes)
                        .await
                        .map_err(|e| format!("Error guardando imagen local: {}", e))?;
                    return Ok((out_name, "image".to_string()));
                }
            }
            Err(
                "El endpoint local no devolvió ninguna imagen en formato OpenAI `data[0]`"
                    .to_string(),
            )
        }
    }
}

/// Optional Whisper word-level timestamp transcription for a scene or full track.
pub async fn transcribe_audio_word_timings(
    subtitle_engine: &str,
    audio_path: &Path,
    data_dir: &Path,
    tenant_id: &str,
    key_pool: Option<&Arc<KeyPool>>,
) -> Result<Option<Vec<(f64, f64, String)>>, String> {
    if subtitle_engine == "scene_auto" || subtitle_engine == "disabled" {
        return Ok(None);
    }
    if subtitle_engine == "whisper_local" {
        // Check local whisper binary & model cleanly
        let models_dir = data_dir.join("models");
        let chosen_model = ["base", "tiny", "small", "medium", "large-v3"]
            .into_iter()
            .find(|id| {
                is_safe_whisper_model_id(id) && models_dir.join(format!("ggml-{}.bin", id)).exists()
            });
        let Some(model_id) = chosen_model else {
            return Ok(None);
        };
        let model_path = models_dir.join(format!("ggml-{}.bin", model_id));
        for bin in ["whisper-cli", "whisper"] {
            if let Ok(out) = Command::new(bin)
                .arg("-m")
                .arg(&model_path)
                .arg("-f")
                .arg(audio_path)
                .args(["-l", "auto", "-oj", "-np"])
                .output()
                .await
            {
                if out.status.success() {
                    let json_sidecar = audio_path.with_extension("wav.json");
                    let raw = if json_sidecar.exists() {
                        let txt = tokio::fs::read_to_string(&json_sidecar).await.ok();
                        let _ = tokio::fs::remove_file(&json_sidecar).await;
                        txt.unwrap_or_default()
                    } else {
                        String::from_utf8_lossy(&out.stdout).to_string()
                    };
                    if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
                        if let Some(segs) = parsed.get("transcription").and_then(Value::as_array) {
                            let mut words = Vec::new();
                            for seg in segs {
                                let start_ms = seg
                                    .pointer("/offsets/from")
                                    .and_then(Value::as_f64)
                                    .unwrap_or(0.0);
                                let end_ms = seg
                                    .pointer("/offsets/to")
                                    .and_then(Value::as_f64)
                                    .unwrap_or(start_ms + 500.0);
                                let txt =
                                    seg.get("text").and_then(Value::as_str).unwrap_or("").trim();
                                if !txt.is_empty() {
                                    words.push((
                                        start_ms / 1000.0,
                                        end_ms / 1000.0,
                                        txt.to_string(),
                                    ));
                                }
                            }
                            if !words.is_empty() {
                                return Ok(Some(words));
                            }
                        }
                    }
                }
            }
        }
        return Ok(None);
    }

    let (endpoint, model, api_key) = match subtitle_engine {
        "groq_whisper" => {
            let Some(k) = resolve_provider_key("groq", tenant_id, key_pool).await else {
                return Ok(None);
            };
            (
                "https://api.groq.com/openai/v1/audio/transcriptions",
                "whisper-large-v3-turbo",
                k,
            )
        }
        "openai_whisper" => {
            let Some(k) = resolve_provider_key("openai", tenant_id, key_pool).await else {
                return Ok(None);
            };
            (
                "https://api.openai.com/v1/audio/transcriptions",
                "whisper-1",
                k,
            )
        }
        _ => return Ok(None),
    };

    let bytes = tokio::fs::read(audio_path)
        .await
        .map_err(|e| format!("Error leyendo audio para Whisper: {}", e))?;
    let fname = audio_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("audio.mp3")
        .to_string();

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(fname)
        .mime_str("audio/mpeg")
        .map_err(|e| format!("Multipart MIME error: {}", e))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model)
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "word")
        .text("timestamp_granularities[]", "segment");

    let client = build_http_client(45)?;
    let resp = client
        .post(endpoint)
        .bearer_auth(&api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Error transcribiendo con Whisper: {}", e))?;

    if !resp.status().is_success() {
        return Ok(None);
    }
    let val: Value = resp.json().await.unwrap_or(Value::Null);
    if let Some(words_arr) = val.get("words").and_then(Value::as_array) {
        let mut out = Vec::new();
        for w in words_arr {
            let start = w.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = w.get("end").and_then(Value::as_f64).unwrap_or(start + 0.3);
            let word = w.get("word").and_then(Value::as_str).unwrap_or("").trim();
            if !word.is_empty() {
                out.push((start, end, word.to_string()));
            }
        }
        if !out.is_empty() {
            return Ok(Some(out));
        }
    }
    if let Some(segs) = val.get("segments").and_then(Value::as_array) {
        let mut out = Vec::new();
        for s in segs {
            let start = s.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = s.get("end").and_then(Value::as_f64).unwrap_or(start + 1.0);
            let text = s.get("text").and_then(Value::as_str).unwrap_or("").trim();
            if !text.is_empty() {
                out.push((start, end, text.to_string()));
            }
        }
        if !out.is_empty() {
            return Ok(Some(out));
        }
    }
    Ok(None)
}

/// Uses active CognitiveRouter LLM (including fallback_chain) to break a topic/script into viral scenes,
/// falling back cleanly to deterministic structured scene extraction if unavailable.
pub async fn generate_storyboard_with_llm(
    router: Option<&Arc<tokio::sync::RwLock<CognitiveRouter>>>,
    tenant_id: &str,
    title: &str,
    source_text: &str,
    aspect_ratio: &str,
) -> Vec<VideoScene> {
    let deterministic = split_text_to_scenes(title, None, source_text, None, None);

    if let Some(router_lock) = router {
        let decision_opt = {
            let r = router_lock.read().await;
            let mut pcb =
                crate::pcb::PCB::new("storyboard-gen".to_string(), 10, source_text.to_string());
            pcb.tenant_id = Some(tenant_id.to_string());
            pcb.task_type = crate::pcb::TaskType::Planning;
            r.decide(&pcb).await.ok()
        };

        if let Some(decision) = decision_opt {
            let sys = ChatMessage {
                role: ChatRole::System,
                content: Some(
                    "Sos un director creativo experto en Reels/TikTok/Shorts publicitarios en español. Devolvé ÚNICAMENTE un JSON válido con un array llamado `scenes`: \
                    [{\"title\":\"título corto 1-3 palabras\",\"narration_text\":\"locución natural y directa en español de 9 a 16 palabras lista para decir en voz alta\",\"on_screen_text\":\"titular impacto 3 a 5 palabras SIN puntos suspensivos\",\"visual_prompt\":\"cinematic sleek dark-mode commercial 3D/tech visual description in English, clean composition, NO TEXT inside image\",\"stock_keywords\":\"2-3 English keywords for Pexels\",\"motion_effect\":\"zoom_in|zoom_out|pan_left\",\"duration_hint_secs\":4.0}]. \
                    REGLAS ESTRICTAS: 1) Respetá EXACTAMENTE la cantidad de escenas y cada tema enumerado por el usuario (si pidió 4 escenas o listó Hook/Módulos/Privacidad/CTA, generá las 4 escenas diferenciadas). \
                    2) NUNCA pongas meta-instrucciones como 'Video promocional vertical', 'Creame un proyecto', 'preset free_builtin' o 'Tono moderno' en narration_text. \
                    3) Sin markdown extra ni explicaciones fuera del JSON."
                        .to_string(),
                ),
                ..Default::default()
            };
            let usr = ChatMessage {
                role: ChatRole::User,
                content: Some(format!(
                    "Título: {}\nFormato: {}\nGuion/Pedido:\n{}",
                    title, aspect_ratio, source_text
                )),
                ..Default::default()
            };

            let mut candidates: Vec<(String, String, String)> = Vec::new();
            candidates.push((
                decision.api_url.clone(),
                decision.api_key.clone(),
                decision.model_id.clone(),
            ));
            for fb in decision.fallback_chain.iter().take(3) {
                candidates.push((fb.api_url.clone(), fb.api_key.clone(), fb.model_id.clone()));
            }

            let http_client = Arc::new(Client::new());
            for (api_url, api_key, model_id) in candidates {
                let driver =
                    CloudProxyDriver::new(Arc::clone(&http_client), api_url, api_key, model_id);
                if let Ok(mut stream) = driver
                    .generate_stream(vec![sys.clone(), usr.clone()], None, None)
                    .await
                {
                    let mut raw = String::new();
                    while let Some(chunk) = stream.next().await {
                        if let Ok(s) = chunk {
                            raw.push_str(&s);
                        }
                    }
                    if let Some(parsed) = parse_llm_storyboard_json(&raw) {
                        let has_meta = parsed.iter().any(|s| {
                            let l = s.narration_text.to_lowercase();
                            l.starts_with("video promocional")
                                || l.contains("free_builtin")
                                || l.contains("tono moderno")
                        });
                        let preserves_count =
                            deterministic.len() < 3 || parsed.len() >= deterministic.len();
                        if !parsed.is_empty() && !has_meta && preserves_count {
                            return parsed;
                        }
                    }
                }
            }
        }
    }

    deterministic
}

fn parse_llm_storyboard_json(raw: &str) -> Option<Vec<VideoScene>> {
    let trimmed = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let val = serde_json::from_str::<Value>(trimmed).ok().or_else(|| {
        let start = trimmed.find('{').or_else(|| trimmed.find('['))?;
        let end = trimmed.rfind('}').or_else(|| trimmed.rfind(']'))?;
        serde_json::from_str::<Value>(&trimmed[start..=end]).ok()
    })?;

    let arr = if let Some(a) = val.as_array() {
        a
    } else {
        val.get("scenes")?.as_array()?
    };

    let mut out = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        let narration = item
            .get("narration_text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Escena")
            .trim()
            .to_string();
        if narration.is_empty() && title.is_empty() {
            continue;
        }
        out.push(VideoScene {
            id: new_id("scn"),
            order: idx,
            title,
            narration_text: narration.clone(),
            on_screen_text: item
                .get("on_screen_text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
            visual_prompt: item
                .get("visual_prompt")
                .and_then(Value::as_str)
                .unwrap_or(&narration)
                .trim()
                .to_string(),
            stock_keywords: item
                .get("stock_keywords")
                .and_then(Value::as_str)
                .unwrap_or("technology abstract minimal")
                .trim()
                .to_string(),
            duration_hint_secs: item
                .get("duration_hint_secs")
                .and_then(Value::as_f64)
                .unwrap_or(4.5)
                .clamp(2.5, 25.0),
            motion_effect: item
                .get("motion_effect")
                .and_then(Value::as_str)
                .unwrap_or("zoom_in")
                .to_string(),
            voice_override: None,
            visual_override: None,
            audio_file: None,
            visual_file: None,
            visual_kind: None,
            actual_audio_secs: None,
            status: "pending".to_string(),
            error: None,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn test_wrap_pcm16_to_wav_and_sample_rate_parser() {
        assert_eq!(
            parse_sample_rate_from_mime("audio/L16;codec=pcm;rate=24000"),
            24_000
        );
        assert_eq!(parse_sample_rate_from_mime("audio/pcm; rate=48000"), 48_000);
        assert_eq!(parse_sample_rate_from_mime("audio/pcm"), 24_000);

        let wav = wrap_pcm16_to_wav(&[0x01, 0x02, 0x03, 0x04], 24_000, 1);
        assert_eq!(wav.len(), 48);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 4);
    }

    #[test]
    fn test_validate_local_endpoint_url_policy() {
        assert!(validate_local_endpoint_url("http://127.0.0.1:8188/", true).is_ok());
        assert!(validate_local_endpoint_url("http://127.0.0.1:8188", false).is_err());
        assert!(validate_local_endpoint_url("https://api.example.com", false).is_ok());
        assert!(validate_local_endpoint_url("http://169.254.169.254/latest", true).is_err());
        assert!(validate_local_endpoint_url("http://user:pass@127.0.0.1:8188", true).is_err());
    }

    #[tokio::test]
    async fn test_local_ai_a1111_contract_mock_server() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let tiny_png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let payload = format!("{{\"images\":[\"{}\"]}}", tiny_png_b64);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let srv = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(),
                    payload
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });

        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let (fname, kind) = generate_with_local_ai(
            &base_url,
            "a1111",
            "cyberpunk studio",
            1080,
            1920,
            dir.path(),
            "scn-test",
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        assert_eq!(kind, "image");
        assert!(dir.path().join(&fname).exists());
        srv.abort();
        Ok(())
    }
}
