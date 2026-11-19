use crate::{
    citadel::CitadelAuthenticated, error::AegisHttpError, routes::engine::validate_engine_api_url,
    state::AppState,
};
use ank_core::enclave::TenantDB;
use ank_core::pr_manager::WorkspaceWsEvent;
use ank_core::video_studio::executor::{
    build_status_response, generate_assets_for_project, render_project_by_id,
    resolve_auto_preset_and_voice, update_module_config, VideoExecContext,
};
use ank_core::video_studio::providers::generate_storyboard_with_llm;
use ank_core::video_studio::runtime_deps::ensure_runtime_installed;
use ank_core::video_studio::store::{
    build_new_project, list_creator_scripts, load_state, new_id, now_iso, project_media_dir,
    save_state, validate_safe_segment,
};
use ank_core::video_studio::types::{
    ProgressCallback, RedactedVideoModuleConfig, VideoProgressEvent, VideoProject, VideoScene,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const MAX_SCENE_UPLOAD_BYTES: usize = 85 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/dependencies/install", post(install_dependencies))
        .route("/config", post(save_video_config))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/:project_id", delete(delete_project))
        .route(
            "/projects/:project_id/engines",
            post(update_project_engines),
        )
        .route(
            "/projects/:project_id/storyboard",
            post(regenerate_storyboard),
        )
        .route("/projects/:project_id/scenes", post(manage_scene))
        .route(
            "/projects/:project_id/scenes/:scene_id/media",
            post(upload_scene_media),
        )
        .route(
            "/projects/:project_id/generate-assets",
            post(start_generate_assets),
        )
        .route("/projects/:project_id/render", post(start_render_mp4))
        .route("/media/:project_id/:filename", get(serve_project_media))
}

async fn make_video_ctx(state: &AppState, tenant_id: &str) -> VideoExecContext {
    let key_pool = {
        let r = state.router.read().await;
        r.key_pool_arc()
    };
    let ws_tx = Arc::clone(&state.workspace_events);
    let tid = tenant_id.to_string();
    let cb: ProgressCallback = Arc::new(move |evt: VideoProgressEvent| {
        let _ = ws_tx.send(WorkspaceWsEvent {
            tenant_id: tid.clone(),
            payload: json!({
                "event": "video_studio_progress",
                "data": evt
            }),
        });
    });

    VideoExecContext {
        data_dir: state.config.data_dir.clone(),
        key_pool: Some(key_pool),
        router: Some(Arc::clone(&state.router)),
        progress_cb: Some(cb),
    }
}

fn emit_modules_updated(state: &AppState, tenant_id: &str, tool_name: &str) {
    let _ = state.workspace_events.send(WorkspaceWsEvent {
        tenant_id: tenant_id.to_string(),
        payload: json!({
            "event": "modules_updated",
            "data": { "module_id": "aegis.domain.video", "tool_name": tool_name }
        }),
    });
}

async fn get_status(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let status = build_status_response(&auth.tenant_id, &auth.session_key_hash, &ctx)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    Ok(Json(json!({ "ok": true, "status": status })))
}

#[derive(Deserialize)]
struct InstallDepsBody {
    #[serde(default)]
    force: bool,
}

async fn install_dependencies(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    body: Option<Json<InstallDepsBody>>,
) -> Result<Json<Value>, AegisHttpError> {
    let force = body.map(|b| b.force).unwrap_or(false);
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let data_dir = state.config.data_dir.clone();
    let ws_tx = Arc::clone(&state.workspace_events);
    let tid = auth.tenant_id.clone();

    tokio::spawn(async move {
        let _res = ensure_runtime_installed(&data_dir, force, ctx.progress_cb.clone()).await;
        let _ = ws_tx.send(WorkspaceWsEvent {
            tenant_id: tid,
            payload: json!({
                "event": "modules_updated",
                "data": { "module_id": "aegis.domain.video", "tool_name": "video_install_runtime" }
            }),
        });
    });

    Ok(Json(json!({
        "ok": true,
        "started": true,
        "message": "Instalación automática de FFmpeg y Edge-TTS iniciada en segundo plano."
    })))
}

async fn save_video_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, AegisHttpError> {
    // Enforce exact HTTP SSRF policy matching engine.rs for any local endpoint URLs
    if let Some(obj) = body.as_object_mut() {
        for key in [
            "local_image_url",
            "local_video_url",
            "local_tts_url",
            "local_stt_url",
        ] {
            if let Some(raw) = obj.get(key).and_then(Value::as_str).map(str::trim) {
                if !raw.is_empty() {
                    validate_engine_api_url(raw, auth.is_master)?;
                }
            }
        }
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let cfg =
        update_module_config(&db, &body, auth.is_master).map_err(AegisHttpError::BadRequest)?;
    emit_modules_updated(&state, &auth.tenant_id, "video_configure_engines");

    Ok(Json(json!({
        "ok": true,
        "config": RedactedVideoModuleConfig::from(&cfg)
    })))
}

async fn list_projects(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let state_data = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    let creator_scripts = list_creator_scripts(&db);

    Ok(Json(json!({
        "ok": true,
        "projects": state_data.projects,
        "creator_scripts": creator_scripts
    })))
}

#[derive(Deserialize)]
struct CreateProjectBody {
    title: String,
    #[serde(default)]
    prompt: String,
    creator_script_id: Option<String>,
    platform: Option<String>,
    aspect_ratio: Option<String>,
    preset: Option<String>,
    voice_engine: Option<String>,
    voice_id: Option<String>,
    visual_engine: Option<String>,
    hook_video_engine: Option<String>,
    subtitle_engine: Option<String>,
    subtitle_style: Option<String>,
    bgm_style: Option<String>,
    #[serde(default)]
    use_ai_storyboard: bool,
    #[serde(default)]
    auto_render: bool,
    scenes: Option<Vec<VideoScene>>,
}

async fn create_project(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(mut body): Json<CreateProjectBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    if matches!(
        body.preset.as_deref().map(str::trim),
        None | Some("") | Some("auto")
    ) {
        let lower = format!("{} {}", body.title, body.prompt).to_ascii_lowercase();
        let (resolved_preset, resolved_voice) =
            resolve_auto_preset_and_voice(&auth.tenant_id, &auth.session_key_hash, &ctx, &lower)
                .await;
        body.preset = Some(resolved_preset);
        if body
            .voice_id
            .as_deref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            body.voice_id = resolved_voice;
        }
    }

    let mut project = {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
        build_new_project(
            &db,
            body.title,
            body.prompt,
            body.creator_script_id,
            body.platform,
            body.aspect_ratio,
            body.preset,
            body.voice_engine,
            body.voice_id,
            body.visual_engine,
            body.hook_video_engine,
            body.subtitle_engine,
            body.subtitle_style,
            body.bgm_style,
            body.scenes,
        )
        .map_err(AegisHttpError::BadRequest)?
    };

    if body.use_ai_storyboard {
        let source = if project.prompt.trim().is_empty() {
            project.title.clone()
        } else {
            project.prompt.clone()
        };
        project.scenes = generate_storyboard_with_llm(
            Some(&state.router),
            &auth.tenant_id,
            &project.title,
            &source,
            &project.aspect_ratio,
        )
        .await;
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let mut vstate = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    vstate.projects.insert(0, project.clone());
    save_state(&db, &vstate).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    emit_modules_updated(&state, &auth.tenant_id, "video_create_project");

    if body.auto_render {
        let safe_pid = project.id.clone();
        let tid = auth.tenant_id.clone();
        let skey = auth.session_key_hash.clone();
        let state_clone = state.clone();
        tokio::spawn(async move {
            let _ = render_project_by_id(&tid, &skey, &safe_pid, true, &ctx).await;
            emit_modules_updated(&state_clone, &tid, "video_render_mp4");
        });
    }

    Ok(Json(
        json!({ "ok": true, "project": project, "auto_render_started": body.auto_render }),
    ))
}

async fn delete_project(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let mut vstate = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    vstate.projects.retain(|p| p.id != safe_pid);
    save_state(&db, &vstate).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    if let Ok(dir) = project_media_dir(&state.config.data_dir, &auth.tenant_id, &safe_pid) {
        if dir.exists() {
            let _ = tokio::fs::remove_dir_all(&dir).await;
        }
    }

    emit_modules_updated(&state, &auth.tenant_id, "video_delete_project");
    Ok(Json(json!({ "ok": true, "deleted": safe_pid })))
}

async fn update_project_engines(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("project_id".to_string(), json!(safe_pid));
    }
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let res = ank_core::video_studio::execute_video_tool(
        "video_configure_engines",
        &body.to_string(),
        &auth.tenant_id,
        &auth.session_key_hash,
        &ctx,
    )
    .await
    .map_err(AegisHttpError::BadRequest)?;

    emit_modules_updated(&state, &auth.tenant_id, "video_configure_engines");
    let parsed: Value = serde_json::from_str(&res).unwrap_or_else(|_| json!({ "ok": true }));
    Ok(Json(parsed))
}

#[derive(Deserialize)]
struct StoryboardBody {
    prompt: Option<String>,
}

async fn regenerate_storyboard(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
    body: Option<Json<StoryboardBody>>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let (title, source_text, aspect_ratio) = {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
        let vstate = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
        let proj = vstate
            .projects
            .iter()
            .find(|p| p.id == safe_pid)
            .ok_or_else(|| {
                AegisHttpError::BadRequest(format!("Proyecto '{}' no encontrado", safe_pid))
            })?;
        let prompt_override = body
            .and_then(|b| b.prompt.clone())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let src = prompt_override.unwrap_or_else(|| {
            if !proj.prompt.trim().is_empty() {
                proj.prompt.clone()
            } else {
                proj.scenes
                    .iter()
                    .map(|s| s.narration_text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        });
        (proj.title.clone(), src, proj.aspect_ratio.clone())
    };

    let scenes = generate_storyboard_with_llm(
        Some(&state.router),
        &auth.tenant_id,
        &title,
        &source_text,
        &aspect_ratio,
    )
    .await;

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let mut vstate = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    let pos = vstate
        .projects
        .iter()
        .position(|p| p.id == safe_pid)
        .ok_or_else(|| AegisHttpError::BadRequest("Proyecto no encontrado".into()))?;
    vstate.projects[pos].scenes = scenes;
    if !source_text.trim().is_empty() {
        vstate.projects[pos].prompt = source_text;
    }
    vstate.projects[pos].status = "draft".to_string();
    vstate.projects[pos].updated_at = now_iso();
    let updated: VideoProject = vstate.projects[pos].clone();
    save_state(&db, &vstate).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    emit_modules_updated(&state, &auth.tenant_id, "video_storyboard");

    Ok(Json(json!({ "ok": true, "project": updated })))
}

async fn manage_scene(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("project_id".to_string(), json!(safe_pid));
    }
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let res = ank_core::video_studio::execute_video_tool(
        "video_manage_scene",
        &body.to_string(),
        &auth.tenant_id,
        &auth.session_key_hash,
        &ctx,
    )
    .await
    .map_err(AegisHttpError::BadRequest)?;
    emit_modules_updated(&state, &auth.tenant_id, "video_manage_scene");
    let parsed: Value = serde_json::from_str(&res).unwrap_or_else(|_| json!({ "ok": true }));
    Ok(Json(parsed))
}

async fn upload_scene_media(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path((project_id, scene_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let safe_sid = validate_safe_segment(&scene_id).map_err(AegisHttpError::BadRequest)?;

    let mut file_bytes = None;
    let mut orig_name = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?
    {
        if field.name() == Some("file") {
            orig_name = field.file_name().map(str::to_string);
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
            if bytes.len() > MAX_SCENE_UPLOAD_BYTES {
                return Err(AegisHttpError::BadRequest(
                    "El archivo supera el límite de 85 MB".to_string(),
                ));
            }
            file_bytes = Some(bytes);
        }
    }

    let data = file_bytes.ok_or_else(|| AegisHttpError::BadRequest("Falta el archivo".into()))?;
    let name = orig_name.unwrap_or_else(|| "media.png".to_string());
    let ext = name
        .rsplit('.')
        .next()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());

    let (kind, normalized_ext) = match ext.as_str() {
        "mp4" | "mov" | "webm" | "mkv" => ("video", ext.as_str()),
        "png" | "jpg" | "jpeg" | "webp" => ("image", ext.as_str()),
        "mp3" | "wav" | "ogg" | "m4a" => ("audio", ext.as_str()),
        _ => {
            return Err(AegisHttpError::BadRequest(
                "Formato no soportado. Usá MP4/WEBM/MOV, PNG/JPG/WEBP o MP3/WAV.".into(),
            ))
        }
    };

    let media_dir = project_media_dir(&state.config.data_dir, &auth.tenant_id, &safe_pid)
        .map_err(AegisHttpError::BadRequest)?;
    tokio::fs::create_dir_all(&media_dir)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    let fname = format!("{}_upload_{}.{}", safe_sid, new_id("m"), normalized_ext);
    validate_safe_segment(&fname).map_err(AegisHttpError::BadRequest)?;
    tokio::fs::write(media_dir.join(&fname), &data)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("TenantDB error: {}", e)))?;
    let mut vstate = load_state(&db).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    let p_idx = vstate
        .projects
        .iter()
        .position(|p| p.id == safe_pid)
        .ok_or_else(|| AegisHttpError::BadRequest("Proyecto no encontrado".into()))?;
    let s_idx = vstate.projects[p_idx]
        .scenes
        .iter()
        .position(|s| s.id == safe_sid)
        .ok_or_else(|| AegisHttpError::BadRequest("Escena no encontrada".into()))?;

    let scene = &mut vstate.projects[p_idx].scenes[s_idx];
    if kind == "audio" {
        scene.audio_file = Some(fname.clone());
        scene.voice_override = Some("custom_upload".to_string());
    } else {
        scene.visual_file = Some(fname.clone());
        scene.visual_kind = Some(kind.to_string());
        scene.visual_override = Some("custom_upload".to_string());
        scene.status = "ready".to_string();
        scene.error = None;
    }
    vstate.projects[p_idx].updated_at = now_iso();
    let updated = vstate.projects[p_idx].clone();
    save_state(&db, &vstate).map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    emit_modules_updated(&state, &auth.tenant_id, "video_upload_media");

    Ok(Json(json!({
        "ok": true,
        "filename": fname,
        "kind": kind,
        "project": updated
    })))
}

#[derive(Deserialize)]
struct GenerateAssetsBody {
    scene_id: Option<String>,
}

async fn start_generate_assets(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
    body: Option<Json<GenerateAssetsBody>>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let scene_id = body.and_then(|b| b.scene_id.clone());
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let tid = auth.tenant_id.clone();
    let skey = auth.session_key_hash.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let _ =
            generate_assets_for_project(&tid, &skey, &safe_pid, scene_id.as_deref(), &ctx).await;
        emit_modules_updated(&state_clone, &tid, "video_generate_assets");
    });

    Ok(Json(json!({
        "ok": true,
        "started": true,
        "message": "Generando voz y visuales en segundo plano..."
    })))
}

#[derive(Deserialize)]
struct RenderBody {
    #[serde(default = "default_true")]
    auto_generate_missing: bool,
}

fn default_true() -> bool {
    true
}

async fn start_render_mp4(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(project_id): Path<String>,
    body: Option<Json<RenderBody>>,
) -> Result<Json<Value>, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let auto_gen = body.map(|b| b.auto_generate_missing).unwrap_or(true);
    let ctx = make_video_ctx(&state, &auth.tenant_id).await;
    let tid = auth.tenant_id.clone();
    let skey = auth.session_key_hash.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
        let _ = render_project_by_id(&tid, &skey, &safe_pid, auto_gen, &ctx).await;
        emit_modules_updated(&state_clone, &tid, "video_render_mp4");
    });

    Ok(Json(json!({
        "ok": true,
        "started": true,
        "message": "Exportando video MP4 con FFmpeg en segundo plano..."
    })))
}

fn mime_for_extension(fname: &str) -> &'static str {
    let lower = fname.to_ascii_lowercase();
    if lower.ends_with(".mp4") {
        "video/mp4"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else if lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if lower.ends_with(".wav") {
        "audio/wav"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".srt") || lower.ends_with(".ass") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

async fn serve_project_media(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    headers: HeaderMap,
    Path((project_id, filename)): Path<(String, String)>,
) -> Result<Response, AegisHttpError> {
    let safe_pid = validate_safe_segment(&project_id).map_err(AegisHttpError::BadRequest)?;
    let safe_fname = validate_safe_segment(&filename).map_err(AegisHttpError::BadRequest)?;

    let media_dir = project_media_dir(&state.config.data_dir, &auth.tenant_id, &safe_pid)
        .map_err(AegisHttpError::BadRequest)?;
    let file_path = media_dir.join(&safe_fname);

    let canon_dir = media_dir
        .canonicalize()
        .map_err(|_| AegisHttpError::NotFound("Proyecto sin archivos multimedia".into()))?;
    let canon_file = file_path
        .canonicalize()
        .map_err(|_| AegisHttpError::NotFound("Archivo multimedia no encontrado".into()))?;
    if !canon_file.starts_with(&canon_dir) {
        return Err(AegisHttpError::Forbidden("Acceso denegado".into()));
    }

    let meta = tokio::fs::metadata(&canon_file)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
    let total_len = meta.len();
    let content_type = mime_for_extension(&safe_fname);

    if let Some(range_val) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some(spec) = range_val.strip_prefix("bytes=") {
            let mut parts = spec.split('-');
            let start = parts
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let end_opt = parts.next().and_then(|s| s.parse::<u64>().ok());
            if total_len > 0 && start < total_len {
                let end = end_opt.unwrap_or(total_len - 1).min(total_len - 1);
                if start <= end {
                    let chunk_len = (end - start + 1).min(24 * 1024 * 1024) as usize;
                    let mut f = tokio::fs::File::open(&canon_file)
                        .await
                        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
                    f.seek(std::io::SeekFrom::Start(start))
                        .await
                        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
                    let mut buf = vec![0u8; chunk_len];
                    f.read_exact(&mut buf)
                        .await
                        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;
                    let actual_end = start + (chunk_len as u64) - 1;
                    return Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, actual_end, total_len),
                        )
                        .header(header::CONTENT_LENGTH, chunk_len.to_string())
                        .body(Body::from(buf))
                        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)));
                }
            }
        }
    }

    let bytes = tokio::fs::read(&canon_file)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::Error::from(e)))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, bytes.len().to_string())
        .body(Body::from(bytes))
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))
}
