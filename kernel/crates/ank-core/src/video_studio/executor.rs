use super::providers::{
    generate_scene_visual, generate_scene_voice, generate_storyboard_with_llm,
    probe_provider_readiness, resolve_elevenlabs_key, validate_local_endpoint_url,
};
use super::renderer::render_project_mp4;
use super::runtime_deps::{
    build_capability_advisor, current_install_progress, ensure_runtime_installed,
    inspect_runtime_status, resolve_ffmpeg_path, resolve_python_for_video,
};
use super::store::{
    build_new_project, list_creator_scripts, load_config, load_state, new_id, now_iso,
    parse_flexible_scenes, project_media_dir, save_config, save_state, validate_safe_segment,
};
use super::types::{
    preset_engines, voice_catalog, ProgressCallback, RedactedVideoModuleConfig, VideoModuleConfig,
    VideoProgressEvent, VideoProject, VideoScene, VideoStudioStatusResponse,
};
use crate::enclave::TenantDB;
use crate::router::{CognitiveRouter, KeyPool};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct VideoExecContext {
    pub data_dir: PathBuf,
    pub key_pool: Option<Arc<KeyPool>>,
    pub router: Option<Arc<RwLock<CognitiveRouter>>>,
    pub progress_cb: Option<ProgressCallback>,
}

impl Default for VideoExecContext {
    fn default() -> Self {
        let data_dir = std::env::var("AEGIS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self {
            data_dir,
            key_pool: None,
            router: None,
            progress_cb: None,
        }
    }
}

pub async fn build_status_response(
    tenant_id: &str,
    session_key: &str,
    ctx: &VideoExecContext,
) -> Result<VideoStudioStatusResponse, String> {
    let (cfg, has_elevenlabs_key, creator_scripts) = {
        let db = TenantDB::open(tenant_id, session_key)
            .map_err(|e| format!("Failed opening TenantDB: {}", e))?;
        let cfg = load_config(&db)?;
        let has_el = resolve_elevenlabs_key(&db).is_some();
        let scripts = list_creator_scripts(&db);
        (cfg, has_el, scripts)
    };
    let runtime = inspect_runtime_status(&ctx.data_dir).await;
    let providers = probe_provider_readiness(
        has_elevenlabs_key,
        &cfg,
        tenant_id,
        &ctx.data_dir,
        ctx.key_pool.as_ref(),
    )
    .await;
    let active_install = current_install_progress().await;
    let advisor = build_capability_advisor(&ctx.data_dir, &providers).await;

    Ok(VideoStudioStatusResponse {
        runtime,
        providers,
        config: RedactedVideoModuleConfig::from(&cfg),
        voices: voice_catalog(),
        creator_scripts,
        active_install,
        advisor: Some(advisor),
    })
}

pub fn update_module_config(
    db: &TenantDB,
    args: &Value,
    allow_local_urls: bool,
) -> Result<VideoModuleConfig, String> {
    let mut cfg = load_config(db)?;

    if let Some(p) = args
        .get("preset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_preset = p.to_string();
        if p != "custom" {
            let (v, vis, hook, sub) = preset_engines(p);
            cfg.default_voice_engine = v.to_string();
            cfg.default_visual_engine = vis.to_string();
            cfg.default_hook_video_engine = hook.to_string();
            cfg.default_subtitle_engine = sub.to_string();
        }
    }
    if let Some(v) = args
        .get("voice_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_voice_engine = v.to_string();
    }
    if let Some(vid) = args
        .get("voice_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_voice_id = vid.to_string();
    }
    if let Some(vis) = args
        .get("visual_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_visual_engine = vis.to_string();
    }
    if let Some(hv) = args
        .get("hook_video_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_hook_video_engine = hv.to_string();
    }
    if let Some(se) = args
        .get("subtitle_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_subtitle_engine = se.to_string();
    }
    if let Some(ss) = args
        .get("subtitle_style")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_subtitle_style = ss.to_string();
    }
    if let Some(bg) = args
        .get("bgm_style")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.default_bgm_style = bg.to_string();
    }
    if let Some(pk) = args.get("pexels_api_key").and_then(Value::as_str) {
        let trimmed = pk.trim();
        cfg.pexels_api_key = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(el) = args.get("elevenlabs_api_key").and_then(Value::as_str) {
        let trimmed = el.trim();
        if !trimmed.is_empty() {
            db.set_kv("elevenlabs_api_key", trimmed)
                .map_err(|e| format!("Failed saving ElevenLabs key: {}", e))?;
        }
    }
    if let Some(mode) = args
        .get("local_image_mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.local_image_mode = Some(mode.to_string());
    }

    for (field, target) in [
        ("local_image_url", &mut cfg.local_image_url),
        ("local_video_url", &mut cfg.local_video_url),
        ("local_tts_url", &mut cfg.local_tts_url),
        ("local_stt_url", &mut cfg.local_stt_url),
    ] {
        if let Some(raw) = args.get(field).and_then(Value::as_str) {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                *target = None;
            } else {
                let valid = validate_local_endpoint_url(trimmed, allow_local_urls)?;
                *target = Some(valid);
            }
        }
    }

    save_config(db, &cfg)?;
    Ok(cfg)
}

fn apply_project_engine_fields(proj: &mut VideoProject, args: &Value) {
    let mut voice_engine_changed = false;
    if let Some(p) = args
        .get("preset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.preset = p.to_string();
        if p != "custom" {
            let (v, vis, hook, sub) = preset_engines(p);
            if proj.voice_engine != v {
                voice_engine_changed = true;
            }
            proj.voice_engine = v.to_string();
            proj.visual_engine = vis.to_string();
            proj.hook_video_engine = hook.to_string();
            proj.subtitle_engine = sub.to_string();
        }
    }
    if let Some(v) = args
        .get("voice_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if proj.voice_engine != v {
            voice_engine_changed = true;
        }
        proj.voice_engine = v.to_string();
    }
    if let Some(v) = args
        .get("voice_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.voice_id = v.to_string();
    } else if voice_engine_changed {
        proj.voice_id = match proj.voice_engine.as_str() {
            "gemini_tts" => "Charon".to_string(),
            "openai_tts" => "onyx".to_string(),
            "elevenlabs" => "EXAVITQu4vr4xnSDxMaL".to_string(),
            "edge_tts" if !proj.voice_id.contains("Neural") => {
                "en-US-AndrewMultilingualNeural".to_string()
            }
            _ => proj.voice_id.clone(),
        };
    }
    if let Some(v) = args
        .get("visual_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.visual_engine = v.to_string();
    }
    if let Some(v) = args
        .get("hook_video_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.hook_video_engine = v.to_string();
    }
    if let Some(v) = args
        .get("subtitle_engine")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.subtitle_engine = v.to_string();
    }
    if let Some(v) = args
        .get("subtitle_style")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.subtitle_style = v.to_string();
    }
    if let Some(v) = args
        .get("bgm_style")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.bgm_style = v.to_string();
    }
    if let Some(v) = args
        .get("aspect_ratio")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.aspect_ratio = v.to_string();
    }
    if let Some(v) = args
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.title = v.to_string();
    }
    if let Some(v) = args
        .get("platform")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        proj.platform = v.to_string();
    }
    proj.updated_at = now_iso();
}

fn push_unique_fallback(list: &mut Vec<String>, note: String) {
    if !list.iter().any(|existing| existing == &note) {
        list.push(note);
    }
}

fn summarize_fallback_reason(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("429") || lower.contains("quota") || lower.contains("resource_exhausted") {
        "límite de cuota / plan gratuito".to_string()
    } else if lower.contains("400")
        || lower.contains("403")
        || lower.contains("billing")
        || lower.contains("facturación")
    {
        "requiere plan Pago / Tier 1 en la API".to_string()
    } else if lower.contains("no tenés")
        || lower.contains("missing")
        || lower.contains("no está configurad")
        || lower.contains("falta")
    {
        "API key o endpoint no configurado".to_string()
    } else if lower.contains("127.0.0.1")
        || lower.contains("connection")
        || lower.contains("refused")
        || lower.contains("timed out")
    {
        "motor local o red no disponible".to_string()
    } else {
        "servicio no disponible".to_string()
    }
}

async fn ensure_python_edge_pillow_ready(
    data_dir: &Path,
    progress_cb: &Option<ProgressCallback>,
) -> Result<(), String> {
    if resolve_python_for_video(data_dir).await.is_none() {
        ensure_runtime_installed(data_dir, false, progress_cb.clone()).await?;
        return Ok(());
    }
    let status = inspect_runtime_status(data_dir).await;
    if !status.edge_tts_ready || !status.pillow_ready {
        ensure_runtime_installed(data_dir, false, progress_cb.clone()).await?;
    }
    Ok(())
}

pub async fn generate_assets_for_project(
    tenant_id: &str,
    session_key: &str,
    project_id: &str,
    only_scene_id: Option<&str>,
    ctx: &VideoExecContext,
) -> Result<VideoProject, String> {
    let safe_pid = validate_safe_segment(project_id)?;

    // Ensure Python + edge-tts/pillow exist automatically if needed by this project (including fallback/HUD overlays)
    let needs_python_runtime = {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        let state = load_state(&db)?;
        let proj = state
            .projects
            .iter()
            .find(|p| p.id == safe_pid)
            .ok_or_else(|| format!("Proyecto '{}' no encontrado.", safe_pid))?;
        let uses_edge = matches!(proj.voice_engine.as_str(), "edge_tts" | "gemini_tts")
            || proj.scenes.iter().any(|s| {
                matches!(
                    s.voice_override.as_deref(),
                    Some("edge_tts") | Some("gemini_tts")
                )
            });
        let uses_card_or_hud = matches!(
            proj.visual_engine.as_str(),
            "kinetic_card" | "gemini_imagen" | "openai_image" | "google_veo"
        ) || proj.scenes.iter().any(|s| {
            matches!(
                s.visual_override.as_deref(),
                Some("kinetic_card")
                    | Some("gemini_imagen")
                    | Some("openai_image")
                    | Some("google_veo")
            )
        });
        uses_edge || uses_card_or_hud
    };

    if needs_python_runtime {
        ensure_python_edge_pillow_ready(&ctx.data_dir, &ctx.progress_cb).await?;
    }

    let (mut project, cfg, elevenlabs_key) = {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        let state = load_state(&db)?;
        let cfg = load_config(&db)?;
        let el_key = resolve_elevenlabs_key(&db);
        let proj = state
            .projects
            .into_iter()
            .find(|p| p.id == safe_pid)
            .ok_or_else(|| format!("Proyecto '{}' no encontrado.", safe_pid))?;
        (proj, cfg, el_key)
    };

    if only_scene_id.is_none() {
        project.fallbacks_applied.clear();
    }

    let media_dir = project_media_dir(&ctx.data_dir, tenant_id, &project.id)?;
    tokio::fs::create_dir_all(&media_dir)
        .await
        .map_err(|e| format!("Error creando carpeta de media: {}", e))?;

    let total = project.scenes.len();
    let mut had_errors = false;

    for idx in 0..total {
        let scene_id = project.scenes[idx].id.clone();
        if let Some(target_sid) = only_scene_id {
            if scene_id != target_sid.trim() {
                continue;
            }
        }

        let eff_voice = project.scenes[idx]
            .voice_override
            .as_deref()
            .filter(|s| !s.is_empty() && *s != "inherit")
            .unwrap_or(&project.voice_engine)
            .to_string();

        let eff_visual = project.scenes[idx]
            .visual_override
            .as_deref()
            .filter(|s| !s.is_empty() && *s != "inherit")
            .unwrap_or_else(|| {
                if idx == 0 && project.hook_video_engine != "inherit" {
                    &project.hook_video_engine
                } else {
                    &project.visual_engine
                }
            })
            .to_string();

        if let Some(ref cb) = ctx.progress_cb {
            cb(VideoProgressEvent {
                project_id: project.id.clone(),
                stage: "generate_assets".to_string(),
                progress: (idx as f32 / total.max(1) as f32) * 0.9,
                message: format!(
                    "Generando escena {}/{}: '{}'...",
                    idx + 1,
                    total,
                    project.scenes[idx].title
                ),
                scene_id: Some(scene_id.clone()),
                done: false,
                error: None,
            });
        }

        // 1. Voice (universal resilience: adapt to Edge HD Multilingual if cloud/local voice fails)
        let voice_res = if project.scenes[idx].voice_override.as_deref() == Some("custom_upload") {
            Ok(project.scenes[idx].audio_file.clone())
        } else {
            let primary_res = generate_scene_voice(
                &eff_voice,
                &project.voice_id,
                &project.scenes[idx].narration_text,
                &media_dir,
                &scene_id,
                &ctx.data_dir,
                tenant_id,
                elevenlabs_key.as_deref(),
                &cfg,
                ctx.key_pool.as_ref(),
            )
            .await;

            match primary_res {
                Ok(Some(ref fname)) if eff_voice == "gemini_tts" && fname.ends_with(".mp3") => {
                    push_unique_fallback(
                        &mut project.fallbacks_applied,
                        "Voz Gemini Studio (límite/cuota en Free Tier) -> adaptada automáticamente a Edge HD Multilingual.".to_string(),
                    );
                    primary_res
                }
                Ok(v) => Ok(v),
                Err(err_msg) if eff_voice != "edge_tts" && eff_voice != "none" => {
                    let reason = summarize_fallback_reason(&err_msg);
                    tracing::warn!(
                        "Voice engine '{}' failed for scene '{}' ({}), falling back to Edge HD Multilingual",
                        eff_voice,
                        scene_id,
                        err_msg
                    );
                    if let Err(rt_err) =
                        ensure_python_edge_pillow_ready(&ctx.data_dir, &ctx.progress_cb).await
                    {
                        Err(format!("{} | Runtime fallback falló: {}", err_msg, rt_err))
                    } else {
                        let fallback_voice = if project.voice_id.contains("Neural") {
                            project.voice_id.as_str()
                        } else {
                            "en-US-AndrewMultilingualNeural"
                        };
                        match generate_scene_voice(
                            "edge_tts",
                            fallback_voice,
                            &project.scenes[idx].narration_text,
                            &media_dir,
                            &scene_id,
                            &ctx.data_dir,
                            tenant_id,
                            None,
                            &cfg,
                            None,
                        )
                        .await
                        {
                            Ok(v) => {
                                push_unique_fallback(
                                    &mut project.fallbacks_applied,
                                    format!(
                                        "Voz '{}' no disponible ({}) -> adaptada automáticamente a Edge HD ({}).",
                                        eff_voice, reason, fallback_voice
                                    ),
                                );
                                Ok(v)
                            }
                            Err(fb_err) => {
                                Err(format!("{} | Fallback Edge TTS: {}", err_msg, fb_err))
                            }
                        }
                    }
                }
                Err(e) => Err(e),
            }
        };

        let mut scene_err: Option<String> = None;
        match voice_res {
            Ok(audio_opt) => {
                project.scenes[idx].audio_file = audio_opt;
            }
            Err(e) => {
                had_errors = true;
                scene_err = Some(e);
            }
        }

        // 2. Visual (skip regenerating if user explicitly uploaded custom media; universal fallback to Motion Compositor 2.0)
        let is_custom_upload = project.scenes[idx].visual_override.as_deref()
            == Some("custom_upload")
            && project.scenes[idx]
                .visual_file
                .as_ref()
                .map(|f| media_dir.join(f).exists())
                .unwrap_or(false);

        if !is_custom_upload {
            let scene_snapshot = project.scenes[idx].clone();
            let vis_res = match generate_scene_visual(
                &eff_visual,
                &scene_snapshot,
                idx,
                total,
                &project.aspect_ratio,
                &project.id,
                &media_dir,
                &ctx.data_dir,
                tenant_id,
                &cfg,
                ctx.key_pool.as_ref(),
                &ctx.progress_cb,
            )
            .await
            {
                Ok(ok) => Ok(ok),
                Err(e) if eff_visual != "kinetic_card" => {
                    let reason = summarize_fallback_reason(&e);
                    tracing::warn!(
                        "Visual engine '{}' failed for scene '{}' ({}), falling back gracefully",
                        eff_visual,
                        scene_id,
                        e
                    );
                    if let Err(rt_err) =
                        ensure_python_edge_pillow_ready(&ctx.data_dir, &ctx.progress_cb).await
                    {
                        Err(format!("{} | Runtime placa falló: {}", e, rt_err))
                    } else {
                        let mut stage1_res = None;
                        if eff_visual != "pollinations_ai" {
                            if let Ok(pol_ok) = generate_scene_visual(
                                "pollinations_ai",
                                &scene_snapshot,
                                idx,
                                total,
                                &project.aspect_ratio,
                                &project.id,
                                &media_dir,
                                &ctx.data_dir,
                                tenant_id,
                                &cfg,
                                ctx.key_pool.as_ref(),
                                &ctx.progress_cb,
                            )
                            .await
                            {
                                push_unique_fallback(
                                    &mut project.fallbacks_applied,
                                    format!(
                                        "Visual '{}' no disponible ({}) -> adaptado a Pollinations Flux IA (fondos generativos gratis sin API key).",
                                        eff_visual, reason
                                    ),
                                );
                                stage1_res = Some(pol_ok);
                            }
                        }

                        if let Some(ok) = stage1_res {
                            Ok(ok)
                        } else {
                            match generate_scene_visual(
                                "kinetic_card",
                                &scene_snapshot,
                                idx,
                                total,
                                &project.aspect_ratio,
                                &project.id,
                                &media_dir,
                                &ctx.data_dir,
                                tenant_id,
                                &cfg,
                                ctx.key_pool.as_ref(),
                                &ctx.progress_cb,
                            )
                            .await
                            {
                                Ok(ok) => {
                                    push_unique_fallback(
                                        &mut project.fallbacks_applied,
                                        format!(
                                            "Visual '{}' no disponible ({}) -> adaptado a Motion Compositor 2.0 integrado.",
                                            eff_visual, reason
                                        ),
                                    );
                                    Ok(ok)
                                }
                                Err(fallback_e) => {
                                    Err(format!("{} | Fallback placa: {}", e, fallback_e))
                                }
                            }
                        }
                    }
                }
                Err(e) => Err(e),
            };
            match vis_res {
                Ok((fname, kind)) => {
                    project.scenes[idx].visual_file = Some(fname);
                    project.scenes[idx].visual_kind = Some(kind);
                }
                Err(e) => {
                    had_errors = true;
                    scene_err = Some(match scene_err {
                        Some(prev) => format!("{} | {}", prev, e),
                        None => e,
                    });
                }
            }
        }

        if let Some(err_txt) = scene_err {
            project.scenes[idx].status = "error".to_string();
            project.scenes[idx].error = Some(err_txt);
        } else {
            project.scenes[idx].status = "ready".to_string();
            project.scenes[idx].error = None;
        }
    }

    let all_ready = project
        .scenes
        .iter()
        .all(|s| s.status == "ready" && s.visual_file.is_some());

    project.status = if all_ready && !had_errors {
        "assets_ready".to_string()
    } else if had_errors {
        "error".to_string()
    } else {
        "draft".to_string()
    };
    project.last_error = if had_errors {
        project.scenes.iter().find_map(|s| s.error.clone())
    } else {
        None
    };
    project.updated_at = now_iso();

    {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        let mut state = load_state(&db)?;
        if let Some(pos) = state.projects.iter().position(|p| p.id == project.id) {
            state.projects[pos] = project.clone();
        } else {
            state.projects.push(project.clone());
        }
        save_state(&db, &state)?;
    }

    if let Some(ref cb) = ctx.progress_cb {
        cb(VideoProgressEvent {
            project_id: project.id.clone(),
            stage: "generate_assets".to_string(),
            progress: 1.0,
            message: if had_errors {
                "Generación terminada con errores en alguna escena.".to_string()
            } else {
                "Todos los recursos de escenas están listos.".to_string()
            },
            scene_id: None,
            done: true,
            error: project.last_error.clone(),
        });
    }

    Ok(project)
}

pub async fn render_project_by_id(
    tenant_id: &str,
    session_key: &str,
    project_id: &str,
    auto_generate_missing: bool,
    ctx: &VideoExecContext,
) -> Result<VideoProject, String> {
    let safe_pid = validate_safe_segment(project_id)?;

    // Seamless auto-install if FFmpeg is missing so the user doesn't struggle
    if resolve_ffmpeg_path(&ctx.data_dir).await.is_none() {
        ensure_runtime_installed(&ctx.data_dir, false, ctx.progress_cb.clone()).await?;
    }

    if auto_generate_missing {
        let needs_gen = {
            let db =
                TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
            let st = load_state(&db)?;
            let proj = st
                .projects
                .iter()
                .find(|p| p.id == safe_pid)
                .ok_or_else(|| format!("Proyecto '{}' no encontrado.", safe_pid))?;
            proj.scenes
                .iter()
                .any(|s| s.status != "ready" || s.visual_file.is_none())
        };
        if needs_gen {
            let updated =
                generate_assets_for_project(tenant_id, session_key, &safe_pid, None, ctx).await?;
            if let Some(err) = updated.last_error {
                return Err(err);
            }
        }
    }

    let mut project = {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        let state = load_state(&db)?;
        state
            .projects
            .into_iter()
            .find(|p| p.id == safe_pid)
            .ok_or_else(|| format!("Proyecto '{}' no encontrado.", safe_pid))?
    };

    let media_dir = project_media_dir(&ctx.data_dir, tenant_id, &project.id)?;
    if let Err(err) = render_project_mp4(
        &mut project,
        &media_dir,
        &ctx.data_dir,
        tenant_id,
        ctx.key_pool.as_ref(),
        ctx.progress_cb.clone(),
    )
    .await
    {
        project.status = "error".to_string();
        project.last_error = Some(err.clone());
        project.updated_at = now_iso();
        if let Ok(db) = TenantDB::open(tenant_id, session_key) {
            if let Ok(mut st) = load_state(&db) {
                if let Some(pos) = st.projects.iter().position(|p| p.id == project.id) {
                    st.projects[pos] = project.clone();
                    let _ = save_state(&db, &st);
                }
            }
        }
        if let Some(ref cb) = ctx.progress_cb {
            cb(VideoProgressEvent {
                project_id: project.id.clone(),
                stage: "render_mp4".to_string(),
                progress: 0.0,
                message: "Error durante la exportación MP4".to_string(),
                scene_id: None,
                done: true,
                error: Some(err.clone()),
            });
        }
        return Err(err);
    }

    project.updated_at = now_iso();
    let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
    let mut state = load_state(&db)?;
    if let Some(pos) = state.projects.iter().position(|p| p.id == project.id) {
        state.projects[pos] = project.clone();
    }
    save_state(&db, &state)?;

    Ok(project)
}

async fn build_advisor_for_tenant(
    tenant_id: &str,
    session_key: &str,
    ctx: &VideoExecContext,
) -> super::types::VideoCapabilityAdvisor {
    let (cfg, has_elevenlabs_key) = match TenantDB::open(tenant_id, session_key) {
        Ok(db) => {
            let cfg = load_config(&db).unwrap_or_default();
            let has_el = resolve_elevenlabs_key(&db).is_some();
            (cfg, has_el)
        }
        Err(_) => (VideoModuleConfig::default(), false),
    };
    let providers = probe_provider_readiness(
        has_elevenlabs_key,
        &cfg,
        tenant_id,
        &ctx.data_dir,
        ctx.key_pool.as_ref(),
    )
    .await;
    build_capability_advisor(&ctx.data_dir, &providers).await
}

pub async fn resolve_auto_preset_and_voice(
    tenant_id: &str,
    session_key: &str,
    ctx: &VideoExecContext,
    prompt_lower: &str,
) -> (String, Option<String>) {
    if prompt_lower.contains("free_builtin") {
        return (
            "free_builtin".to_string(),
            Some("en-US-AndrewMultilingualNeural".to_string()),
        );
    }
    if prompt_lower.contains("gemini_studio") {
        return ("gemini_studio".to_string(), Some("Charon".to_string()));
    }
    if prompt_lower.contains("hybrid_pro") {
        return ("hybrid_pro".to_string(), None);
    }
    if prompt_lower.contains("local_ai") {
        return ("local_ai".to_string(), None);
    }
    if let Ok(db) = TenantDB::open(tenant_id, session_key) {
        let cfg = load_config(&db).unwrap_or_default();
        let has_el = resolve_elevenlabs_key(&db).is_some();
        let readiness = probe_provider_readiness(
            has_el,
            &cfg,
            tenant_id,
            &ctx.data_dir,
            ctx.key_pool.as_ref(),
        )
        .await;
        if readiness.has_gemini_key {
            return ("gemini_studio".to_string(), Some("Charon".to_string()));
        }
        if readiness.has_openai_key || readiness.has_elevenlabs_key || readiness.has_pexels_key {
            return ("hybrid_pro".to_string(), None);
        }
    }
    (
        "free_builtin".to_string(),
        Some("en-US-AndrewMultilingualNeural".to_string()),
    )
}

fn infer_voice_id_hint(raw_prompt: &str) -> Option<String> {
    let lower = raw_prompt.to_ascii_lowercase();
    for known in [
        "en-US-AndrewMultilingualNeural",
        "en-US-AvaMultilingualNeural",
        "en-US-BrianMultilingualNeural",
        "en-US-EmmaMultilingualNeural",
        "es-AR-ElenaNeural",
        "es-AR-TomasNeural",
        "es-MX-JorgeNeural",
        "es-MX-DaliaNeural",
        "es-ES-XimenaNeural",
        "es-ES-AlvaroNeural",
        "Charon",
        "Orus",
        "Fenrir",
        "Kore",
        "Aoede",
        "Puck",
    ] {
        if lower.contains(&known.to_ascii_lowercase()) {
            return Some(known.to_string());
        }
    }
    None
}

async fn create_and_optionally_render_project(
    args: &Value,
    default_auto_render: bool,
    default_auto_preset: bool,
    tenant_id: &str,
    session_key: &str,
    ctx: &VideoExecContext,
) -> Result<Value, String> {
    let raw_prompt = args
        .get("prompt")
        .or_else(|| args.get("script_or_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() && !raw_prompt.trim().is_empty() {
        title = raw_prompt
            .trim()
            .chars()
            .take(56)
            .collect::<String>()
            .trim_end_matches(&['.', ',', ':', ';'][..])
            .to_string();
    }
    let creator_script_id = args
        .get("creator_script_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let explicit_scenes = args
        .get("scenes")
        .and_then(|v| parse_flexible_scenes(v, &title))
        .filter(|v| !v.is_empty());

    let prompt_lower = raw_prompt.to_ascii_lowercase();
    let mut preset_arg = args
        .get("preset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mut voice_id_arg = args
        .get("voice_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| infer_voice_id_hint(&raw_prompt));

    if matches!(preset_arg.as_deref(), None | Some("auto")) && default_auto_preset {
        let (auto_preset, auto_voice) =
            resolve_auto_preset_and_voice(tenant_id, session_key, ctx, &prompt_lower).await;
        preset_arg = Some(auto_preset);
        if voice_id_arg.is_none() {
            voice_id_arg = auto_voice;
        }
    }

    let aspect_ratio_arg = args
        .get("aspect_ratio")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            if prompt_lower.contains("16:9") || prompt_lower.contains("horizontal") {
                Some("16:9".to_string())
            } else if prompt_lower.contains("1:1") || prompt_lower.contains("cuadrado") {
                Some("1:1".to_string())
            } else {
                None
            }
        });

    let use_llm_storyboard = explicit_scenes.is_none()
        && creator_script_id.is_none()
        && args
            .get("use_ai_storyboard")
            .and_then(Value::as_bool)
            .unwrap_or(true);

    let mut project = {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        build_new_project(
            &db,
            title,
            raw_prompt,
            creator_script_id,
            args.get("platform")
                .and_then(Value::as_str)
                .map(str::to_string),
            aspect_ratio_arg,
            preset_arg,
            args.get("voice_engine")
                .and_then(Value::as_str)
                .map(str::to_string),
            voice_id_arg,
            args.get("visual_engine")
                .and_then(Value::as_str)
                .map(str::to_string),
            args.get("hook_video_engine")
                .and_then(Value::as_str)
                .map(str::to_string),
            args.get("subtitle_engine")
                .and_then(Value::as_str)
                .map(str::to_string),
            args.get("subtitle_style")
                .and_then(Value::as_str)
                .map(str::to_string),
            args.get("bgm_style")
                .and_then(Value::as_str)
                .map(str::to_string),
            explicit_scenes,
        )?
    };

    if use_llm_storyboard {
        let source = if project.prompt.trim().is_empty() {
            project.title.clone()
        } else {
            project.prompt.clone()
        };
        project.scenes = generate_storyboard_with_llm(
            ctx.router.as_ref(),
            tenant_id,
            &project.title,
            &source,
            &project.aspect_ratio,
        )
        .await;
    }

    {
        let db = TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
        let mut state = load_state(&db)?;
        state.projects.insert(0, project.clone());
        save_state(&db, &state)?;
    }

    let auto_render = args
        .get("auto_render")
        .and_then(Value::as_bool)
        .unwrap_or(default_auto_render);

    if !auto_render {
        return Ok(json!({ "ok": true, "project": project }));
    }

    ensure_runtime_installed(&ctx.data_dir, false, ctx.progress_cb.clone()).await?;
    let assets_proj =
        generate_assets_for_project(tenant_id, session_key, &project.id, None, ctx).await?;
    if let Some(err) = assets_proj.last_error {
        return Err(err);
    }
    let rendered = render_project_by_id(tenant_id, session_key, &project.id, false, ctx).await?;
    let advisor = build_advisor_for_tenant(tenant_id, session_key, ctx).await;
    let token = format!("[AEGIS_VIDEO_CARD:{}]", rendered.id);

    Ok(json!({
        "ok": true,
        "status": "rendered",
        "chat_card_token": token,
        "chat_embed_tag": token,
        "output_mp4_url": format!("/api/video/media/{}/output.mp4", rendered.id),
        "download_api": format!("/api/video/media/{}/output.mp4", rendered.id),
        "subtitles_srt_url": rendered.output_srt.as_ref().map(|_| format!("/api/video/media/{}/subtitles.srt", rendered.id)),
        "fallbacks_applied": rendered.fallbacks_applied,
        "advisor": advisor,
        "ui_hint": format!("Incluí exactamente [AEGIS_VIDEO_CARD:{}] en tu respuesta para mostrar el reproductor y botón de descarga.", rendered.id),
        "project": rendered
    }))
}

/// Executes any tool exposed by `aegis.domain.video` both from Chat agents and HTTP module dispatch.
pub async fn execute_video_tool(
    tool_name: &str,
    args_json: &str,
    tenant_id: &str,
    session_key: &str,
    ctx: &VideoExecContext,
) -> Result<String, String> {
    let args: Value = if args_json.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(args_json)
            .map_err(|e| format!("Invalid tool arguments JSON: {}", e))?
    };

    match tool_name {
        "video_get_overview" => {
            let state = {
                let db = TenantDB::open(tenant_id, session_key)
                    .map_err(|e| format!("DB error: {}", e))?;
                load_state(&db)?
            };
            let mut status = build_status_response(tenant_id, session_key, ctx).await?;
            // Redact host filesystem path from chat tool payloads so weak LLMs don't instruct users to dig in OS directories.
            status.runtime.runtime_dir = "managed://video_studio/runtime".to_string();
            let pid = args.get("project_id").and_then(Value::as_str);
            let filtered: Vec<Value> = state
                .projects
                .into_iter()
                .filter(|p| pid.map(|target| p.id == target).unwrap_or(true))
                .map(|p| {
                    let mut v = serde_json::to_value(&p).unwrap_or(Value::Null);
                    if p.output_mp4.is_some() {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "chat_card_token".to_string(),
                                json!(format!("[AEGIS_VIDEO_CARD:{}]", p.id)),
                            );
                            obj.insert(
                                "download_api".to_string(),
                                json!(format!("/api/video/media/{}/output.mp4", p.id)),
                            );
                        }
                    }
                    v
                })
                .collect();
            Ok(json!({
                "projects": filtered,
                "status": status,
                "ui_hint": "Si hay un proyecto renderizado, incluí su chat_card_token ([AEGIS_VIDEO_CARD:<id>]) en tu mensaje; nunca digas que no se puede descargar ni muestres rutas locales."
            })
            .to_string())
        }
        "video_install_runtime" => {
            let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
            let st =
                ensure_runtime_installed(&ctx.data_dir, force, ctx.progress_cb.clone()).await?;
            Ok(json!({ "ok": true, "runtime": st }).to_string())
        }
        "video_configure_engines" => {
            let db =
                TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
            if let Some(pid) = args
                .get("project_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let mut state = load_state(&db)?;
                let pos = state
                    .projects
                    .iter()
                    .position(|p| p.id == pid)
                    .ok_or_else(|| format!("Proyecto '{}' no encontrado.", pid))?;
                apply_project_engine_fields(&mut state.projects[pos], &args);
                let updated = state.projects[pos].clone();
                save_state(&db, &state)?;
                return Ok(json!({ "ok": true, "project": updated }).to_string());
            }
            let allow_local = tenant_id == "root" || cfg!(debug_assertions);
            let cfg = update_module_config(&db, &args, allow_local)?;
            Ok(json!({
                "ok": true,
                "config": RedactedVideoModuleConfig::from(&cfg)
            })
            .to_string())
        }
        "video_produce_auto" => {
            let out = create_and_optionally_render_project(
                &args,
                true,
                true,
                tenant_id,
                session_key,
                ctx,
            )
            .await?;
            Ok(out.to_string())
        }
        "video_create_project" => {
            let out = create_and_optionally_render_project(
                &args,
                false,
                false,
                tenant_id,
                session_key,
                ctx,
            )
            .await?;
            Ok(out.to_string())
        }
        "video_manage_scene" => {
            let action = args
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("update");
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "'project_id' requerido".to_string())?;
            let db =
                TenantDB::open(tenant_id, session_key).map_err(|e| format!("DB error: {}", e))?;
            let mut state = load_state(&db)?;
            let pos = state
                .projects
                .iter()
                .position(|p| p.id == project_id)
                .ok_or_else(|| format!("Proyecto '{}' no encontrado.", project_id))?;
            let proj = &mut state.projects[pos];

            match action {
                "add" => {
                    let scn = VideoScene {
                        id: new_id("scn"),
                        order: proj.scenes.len(),
                        title: args
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("Nueva Escena")
                            .trim()
                            .to_string(),
                        narration_text: args
                            .get("narration_text")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                        on_screen_text: args
                            .get("on_screen_text")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                        visual_prompt: args
                            .get("visual_prompt")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                        stock_keywords: args
                            .get("stock_keywords")
                            .and_then(Value::as_str)
                            .unwrap_or("technology minimal")
                            .trim()
                            .to_string(),
                        duration_hint_secs: args
                            .get("duration_hint_secs")
                            .and_then(Value::as_f64)
                            .unwrap_or(4.5),
                        motion_effect: args
                            .get("motion_effect")
                            .and_then(Value::as_str)
                            .unwrap_or("zoom_in")
                            .to_string(),
                        voice_override: args
                            .get("voice_override")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        visual_override: args
                            .get("visual_override")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        audio_file: None,
                        visual_file: None,
                        visual_kind: None,
                        actual_audio_secs: None,
                        status: "pending".to_string(),
                        error: None,
                    };
                    proj.scenes.push(scn);
                }
                "update" => {
                    let sid = args
                        .get("scene_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "'scene_id' requerido para update".to_string())?;
                    let sidx = proj
                        .scenes
                        .iter()
                        .position(|s| s.id == sid)
                        .ok_or_else(|| format!("Escena '{}' no encontrada.", sid))?;
                    let sc = &mut proj.scenes[sidx];
                    if let Some(t) = args.get("title").and_then(Value::as_str) {
                        sc.title = t.trim().to_string();
                    }
                    if let Some(n) = args.get("narration_text").and_then(Value::as_str) {
                        sc.narration_text = n.trim().to_string();
                    }
                    if let Some(o) = args.get("on_screen_text").and_then(Value::as_str) {
                        sc.on_screen_text = o.trim().to_string();
                    }
                    if let Some(vp) = args.get("visual_prompt").and_then(Value::as_str) {
                        sc.visual_prompt = vp.trim().to_string();
                    }
                    if let Some(kw) = args.get("stock_keywords").and_then(Value::as_str) {
                        sc.stock_keywords = kw.trim().to_string();
                    }
                    if let Some(m) = args.get("motion_effect").and_then(Value::as_str) {
                        sc.motion_effect = m.to_string();
                    }
                    if let Some(d) = args.get("duration_hint_secs").and_then(Value::as_f64) {
                        sc.duration_hint_secs = d.clamp(2.0, 30.0);
                    }
                    if let Some(vo) = args.get("voice_override").and_then(Value::as_str) {
                        sc.voice_override = if vo == "inherit" || vo.is_empty() {
                            None
                        } else {
                            Some(vo.to_string())
                        };
                    }
                    if let Some(viso) = args.get("visual_override").and_then(Value::as_str) {
                        sc.visual_override = if viso == "inherit" || viso.is_empty() {
                            None
                        } else {
                            Some(viso.to_string())
                        };
                    }
                    sc.status = "pending".to_string();
                }
                "delete" => {
                    let sid = args
                        .get("scene_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "'scene_id' requerido para delete".to_string())?;
                    proj.scenes.retain(|s| s.id != sid);
                }
                "move" => {
                    let sid = args
                        .get("scene_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "'scene_id' requerido para move".to_string())?;
                    let new_idx =
                        args.get("new_index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    if let Some(old_idx) = proj.scenes.iter().position(|s| s.id == sid) {
                        let item = proj.scenes.remove(old_idx);
                        let target = new_idx.min(proj.scenes.len());
                        proj.scenes.insert(target, item);
                    }
                }
                other => return Err(format!("Acción desconocida para scene: '{}'", other)),
            }

            for (i, s) in proj.scenes.iter_mut().enumerate() {
                s.order = i;
            }
            proj.updated_at = now_iso();
            let updated = proj.clone();
            save_state(&db, &state)?;
            Ok(json!({ "ok": true, "project": updated }).to_string())
        }
        "video_generate_assets" => {
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "'project_id' requerido".to_string())?;
            let scene_id = args.get("scene_id").and_then(Value::as_str);
            let proj =
                generate_assets_for_project(tenant_id, session_key, project_id, scene_id, ctx)
                    .await?;
            Ok(json!({ "ok": proj.last_error.is_none(), "project": proj }).to_string())
        }
        "video_render_mp4" => {
            let project_id = args
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "'project_id' requerido".to_string())?;
            let auto_gen = args
                .get("auto_generate_missing")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let proj =
                render_project_by_id(tenant_id, session_key, project_id, auto_gen, ctx).await?;
            let advisor = build_advisor_for_tenant(tenant_id, session_key, ctx).await;
            let token = format!("[AEGIS_VIDEO_CARD:{}]", proj.id);
            Ok(json!({
                "ok": true,
                "status": "rendered",
                "chat_card_token": token,
                "chat_embed_tag": token,
                "output_mp4_url": format!("/api/video/media/{}/output.mp4", proj.id),
                "download_api": format!("/api/video/media/{}/output.mp4", proj.id),
                "fallbacks_applied": proj.fallbacks_applied,
                "advisor": advisor,
                "ui_hint": format!("Incluí exactamente [AEGIS_VIDEO_CARD:{}] en tu respuesta para mostrar el reproductor y botón de descarga.", proj.id),
                "project": proj
            })
            .to_string())
        }
        other => Err(format!(
            "Tool '{}' no implementada en aegis.domain.video",
            other
        )),
    }
}
