pub mod executor;
pub mod providers;
pub mod renderer;
pub mod runtime_deps;
pub mod store;
pub mod types;

pub use executor::{
    build_status_response, execute_video_tool, generate_assets_for_project, render_project_by_id,
    update_module_config, VideoExecContext,
};
pub use types::{
    ProviderReadiness, RedactedVideoModuleConfig, RuntimeDependencyStatus, VideoModuleConfig,
    VideoProgressEvent, VideoProject, VideoScene, VideoStudioStatusResponse,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enclave::TenantDB;
    use anyhow::Context;
    use serde_json::{json, Value};

    #[tokio::test]
    async fn test_video_project_lifecycle_and_creator_script_bridge() -> anyhow::Result<()> {
        let env = crate::enclave::ScopedTestDataDir::new()?;
        let tenant = "vid_test_tenant";
        let skey = "vid_test_key";
        let db = TenantDB::open(tenant, skey).context("open TenantDB")?;

        // Clean prior keys if any
        let _ = db.set_kv("module_data:aegis.domain.video", "");
        let _ = db.set_kv("module_config:aegis.domain.video", "");

        // Seed a Creator Studio script to verify the 1-click Creator -> Video bridge
        db.set_kv(
            "module_data:aegis.domain.creator",
            &json!({
                "ideas": [],
                "scripts": [{
                    "id": "scr-demo1234",
                    "title": "IA Local vs Nube",
                    "platform": "tiktok_reels",
                    "hook_0_3s": "¿Todavía pagás suscripciones para automatizar tu negocio?",
                    "body_script": "Con Aegis corrés tus módulos en tu propia máquina. Control total de datos.",
                    "b_roll_notes": "Pantalla de código y terminal limpia",
                    "cta": "Probá Aegis Video Studio hoy."
                }],
                "calendar": []
            })
            .to_string(),
        )?;

        let ctx = VideoExecContext {
            data_dir: env.path().to_path_buf(),
            ..Default::default()
        };
        let create_res = execute_video_tool(
            "video_create_project",
            &json!({
                "title": "Demo desde Creator",
                "creator_script_id": "scr-demo1234",
                "aspect_ratio": "9:16",
                "preset": "hybrid_pro"
            })
            .to_string(),
            tenant,
            skey,
            &ctx,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        let parsed: Value = serde_json::from_str(&create_res)?;
        let proj = &parsed["project"];
        let pid = proj["id"].as_str().context("project id")?;
        assert_eq!(proj["voice_engine"], "edge_tts");
        assert_eq!(proj["visual_engine"], "gemini_imagen");
        assert_eq!(proj["hook_video_engine"], "google_veo");
        assert_eq!(proj["subtitle_engine"], "groq_whisper");
        let scenes = proj["scenes"].as_array().context("scenes")?;
        assert!(scenes.len() >= 3);

        // Configure secret Pexels key + validate it stays redacted in overview
        let cfg_res = execute_video_tool(
            "video_configure_engines",
            &json!({
                "pexels_api_key": "secret-pexels-123",
                "preset": "free_builtin"
            })
            .to_string(),
            tenant,
            skey,
            &ctx,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
        assert!(!cfg_res.contains("secret-pexels-123"));
        assert!(cfg_res.contains("\"has_pexels_key\":true"));

        let ov_res = execute_video_tool("video_get_overview", "{}", tenant, skey, &ctx)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        assert!(!ov_res.contains("secret-pexels-123"));
        assert!(ov_res.contains(pid));

        // Path traversal guard check
        assert!(store::validate_safe_segment("../escape").is_err());
        assert!(store::validate_safe_segment("vid-1234abcd").is_ok());

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires live network to download FFmpeg (~90MB) and synthesize Edge-TTS audio"]
    async fn test_live_free_video_render_pipeline() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let data_dir = dir.path().to_path_buf();
        let tenant = "vid_live_smoke";
        let skey = "vid_live_smoke_key";

        let _ = TenantDB::open(tenant, skey)?.set_kv("module_data:aegis.domain.video", "");

        let ctx = VideoExecContext {
            data_dir: data_dir.clone(),
            key_pool: None,
            router: None,
            progress_cb: None,
        };

        let rt = runtime_deps::ensure_runtime_installed(&data_dir, false, None)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        assert!(rt.ready_for_free_video, "runtime not ready: {:?}", rt);

        let create_res = execute_video_tool(
            "video_create_project",
            &json!({
                "title": "Aegis: Tu Sistema Operativo Cognitivo",
                "aspect_ratio": "9:16",
                "preset": "free_builtin",
                "voice_id": "es-AR-TomasNeural",
                "scenes": [
                    {
                        "id": "scn-hook",
                        "order": 0,
                        "title": "Hook",
                        "narration_text": "¿Seguís usando diez apps distintas para trabajar con inteligencia artificial?",
                        "on_screen_text": "¿DIEZ APPS PARA USAR IA?",
                        "visual_prompt": "Futuristic AI dashboard",
                        "stock_keywords": "technology dashboard",
                        "motion_effect": "zoom_in"
                    },
                    {
                        "id": "scn-body",
                        "order": 1,
                        "title": "Módulos a Medida",
                        "narration_text": "Con Aegis instalás solo los módulos que necesitás: programación, creador de videos o tu negocio.",
                        "on_screen_text": "SOLO LO QUE NECESITÁS",
                        "visual_prompt": "Modular microkernel blocks",
                        "stock_keywords": "software modular",
                        "motion_effect": "pan_left"
                    },
                    {
                        "id": "scn-cta",
                        "order": 2,
                        "title": "Cierre",
                        "narration_text": "Privacidad total, memoria cifrada y control desde el chat. Probá Aegis hoy.",
                        "on_screen_text": "PROBÁ AEGIS HOY",
                        "visual_prompt": "Aegis emblem",
                        "stock_keywords": "cyber security",
                        "motion_effect": "zoom_out"
                    }
                ]
            })
            .to_string(),
            tenant,
            skey,
            &ctx,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        let parsed: Value = serde_json::from_str(&create_res)?;
        let pid = parsed["project"]["id"].as_str().context("pid")?.to_string();

        let rendered = render_project_by_id(tenant, skey, &pid, true, &ctx)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        assert_eq!(rendered.status, "rendered");
        let media_dir =
            store::project_media_dir(&data_dir, tenant, &pid).map_err(|e| anyhow::anyhow!(e))?;
        let mp4_path = media_dir.join("output.mp4");
        let srt_path = media_dir.join("subtitles.srt");
        assert!(mp4_path.exists(), "missing output.mp4");
        assert!(srt_path.exists(), "missing subtitles.srt");

        let meta = std::fs::metadata(&mp4_path)?;
        assert!(
            meta.len() > 20_000,
            "output.mp4 unexpectedly tiny: {} bytes",
            meta.len()
        );

        let ffprobe = runtime_deps::resolve_ffprobe_path(&data_dir)
            .await
            .context("ffprobe missing")?;
        let dur = renderer::probe_media_duration(&ffprobe, &mp4_path)
            .await
            .context("probe duration failed")?;
        println!(
            "Rendered promo MP4: {} bytes, duration {:.2}s",
            meta.len(),
            dur
        );
        assert!(dur >= 8.0, "duration too short: {}", dur);

        Ok(())
    }
}
