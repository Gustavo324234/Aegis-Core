use super::providers::transcribe_audio_word_timings;
use super::runtime_deps::{resolve_ffmpeg_path, resolve_ffprobe_path};
use super::types::{aspect_dimensions, ProgressCallback, VideoProgressEvent, VideoProject};
use crate::router::KeyPool;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct AssCue {
    pub start: f64,
    pub end: f64,
    pub ass_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EdgeWordTiming {
    start: f64,
    end: f64,
    text: String,
}

fn emit_render_progress(
    cb: &Option<ProgressCallback>,
    project_id: &str,
    progress: f32,
    message: &str,
    done: bool,
    error: Option<String>,
) {
    if let Some(callback) = cb {
        callback(VideoProgressEvent {
            project_id: project_id.to_string(),
            stage: "render_mp4".to_string(),
            progress,
            message: message.to_string(),
            scene_id: None,
            done,
            error,
        });
    }
}

pub async fn probe_media_duration(ffprobe_exe: &str, media_path: &Path) -> Option<f64> {
    let out = Command::new(ffprobe_exe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(media_path)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|d| *d > 0.05 && d.is_finite())
}

async fn load_edge_word_timings(
    project_dir: &Path,
    scene_id: &str,
) -> Option<Vec<(f64, f64, String)>> {
    let words_path = project_dir.join(format!("{}_words.json", scene_id));
    if !words_path.exists() {
        return None;
    }
    let raw = tokio::fs::read_to_string(&words_path).await.ok()?;
    let parsed: Vec<EdgeWordTiming> = serde_json::from_str(&raw).ok()?;
    let words: Vec<(f64, f64, String)> = parsed
        .into_iter()
        .filter(|w| !w.text.trim().is_empty())
        .map(|w| (w.start.max(0.0), w.end.max(w.start + 0.08), w.text))
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words)
    }
}

fn clean_ass_word(raw: &str) -> String {
    raw.replace('\\', "").replace('{', "(").replace('}', ")")
}

fn ass_highlight_palette(style: &str) -> (&'static str, &'static str) {
    // Returns (base_color, active_word_highlight_color) in ASS &HAABBGGRR&
    match style {
        "amber_highlight" => ("&H00FFFFFF&", "&H003CE6FF&"),
        "clean_bottom" => ("&H00F1F5F9&", "&H00EEE622&"),
        _ => ("&H00FFFFFF&", "&H00EED322&"), // tiktok_bold: crisp white + bright cyan highlight
    }
}

fn group_words_into_cues_and_karaoke(
    words: &[(f64, f64, String)],
    offset_secs: f64,
    spoken_secs: f64,
    style: &str,
) -> (Vec<SubtitleCue>, Vec<AssCue>) {
    let mut srt_cues = Vec::new();
    let mut ass_cues = Vec::new();
    let uppercase = style == "tiktok_bold" || style == "amber_highlight";
    let (base_col, hi_col) = ass_highlight_palette(style);

    let mut idx = 0;
    while idx < words.len() {
        let mut take = 0usize;
        let mut char_acc = 0usize;
        while idx + take < words.len() && take < 3 {
            let w_len = words[idx + take].2.chars().count();
            if take > 0 && char_acc + 1 + w_len > 22 {
                break;
            }
            char_acc += if take > 0 { 1 + w_len } else { w_len };
            take += 1;
        }
        take = take.max(1);
        let slice = &words[idx..idx + take];
        let g_start = offset_secs + slice[0].0.min(spoken_secs);
        let g_end =
            (offset_secs + slice[slice.len() - 1].1.min(spoken_secs + 0.15)).max(g_start + 0.25);

        let group_text = slice
            .iter()
            .map(|(_, _, w)| w.trim())
            .collect::<Vec<_>>()
            .join(" ");
        if !group_text.is_empty() {
            srt_cues.push(SubtitleCue {
                start: g_start,
                end: g_end,
                text: group_text,
            });
        }

        // Word-by-word active highlight ASS dialogues inside this phrase group
        for (j, (ws, we, _)) in slice.iter().enumerate() {
            let w_start = offset_secs + (*ws).min(spoken_secs);
            let next_start = if j + 1 < slice.len() {
                offset_secs + slice[j + 1].0.min(spoken_secs)
            } else {
                offset_secs + (*we).min(spoken_secs + 0.15)
            };
            let w_end = next_start.max(w_start + 0.10);

            let line = slice
                .iter()
                .enumerate()
                .map(|(k, (_, _, wtxt))| {
                    let clean = clean_ass_word(wtxt.trim());
                    let display = if uppercase {
                        clean.to_uppercase()
                    } else {
                        clean
                    };
                    if k == j {
                        format!(
                            "{{\\c{}\\fscx108\\fscy108}}{}{{\\c{}\\fscx100\\fscy100}}",
                            hi_col, display, base_col
                        )
                    } else {
                        display
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            if !line.trim().is_empty() {
                ass_cues.push(AssCue {
                    start: w_start,
                    end: w_end,
                    ass_text: line,
                });
            }
        }
        idx += take;
    }

    (srt_cues, ass_cues)
}

fn build_proportional_words(text: &str, spoken_secs: f64) -> Vec<(f64, f64, String)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let total_chars: usize = tokens.iter().map(|w| w.chars().count().max(1)).sum();
    let usable = spoken_secs.max(1.0);
    let mut cursor = 0.0f64;
    let mut out = Vec::with_capacity(tokens.len());

    for (i, tok) in tokens.iter().enumerate() {
        let weight = (tok.chars().count().max(1) as f64) / (total_chars.max(1) as f64);
        let dur = (usable * weight).max(0.10);
        let end = if i + 1 == tokens.len() {
            usable.max(cursor + 0.12)
        } else {
            cursor + dur
        };
        out.push((cursor, end, (*tok).to_string()));
        cursor = end;
    }
    out
}

fn format_srt_time(secs: f64) -> String {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

fn format_ass_time(secs: f64) -> String {
    let total_cs = (secs.max(0.0) * 100.0).round() as u64;
    let h = total_cs / 360_000;
    let m = (total_cs % 360_000) / 6000;
    let s = (total_cs % 6000) / 100;
    let cs = total_cs % 100;
    format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
}

fn render_srt_content(cues: &[SubtitleCue]) -> String {
    let mut out = String::new();
    for (idx, cue) in cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            idx + 1,
            format_srt_time(cue.start),
            format_srt_time(cue.end),
            cue.text.trim()
        ));
    }
    out
}

fn render_ass_content(cues: &[AssCue], width: u32, height: u32, style: &str) -> String {
    let font_size = match (width, style) {
        (w, "clean_bottom") if w <= 1080 => 50,
        (w, _) if w <= 1080 => 68,
        (_, "clean_bottom") => 46,
        _ => 58,
    };
    let margin_v = if height >= 1600 { 300 } else { 96 };
    let (primary_col, _) = ass_highlight_palette(style);
    let (outline_col, outline_w, back_col) = match style {
        "amber_highlight" => ("&H00000000", 6, "&H90000000"),
        "clean_bottom" => ("&H00101010", 4, "&H70000000"),
        _ => ("&H00140A05", 7, "&H96000000"),
    };

    let mut ass = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {width}\nPlayResY: {height}\nWrapStyle: 0\nScaledBorderAndShadow: yes\n\n\
        [V4+ Styles]\n\
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
        Style: Default,Segoe UI,{font_size},{primary_col},&H000000FF,{outline_col},{back_col},-1,0,0,0,100,100,0,0,1,{outline_w},2,2,64,64,{margin_v},1\n\n\
        [Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    );

    for cue in cues {
        ass.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            format_ass_time(cue.start),
            format_ass_time(cue.end),
            cue.ass_text
        ));
    }
    ass
}

fn bgm_filter_spec(bgm_style: &str, total_dur: f64) -> Option<(String, String)> {
    let style = bgm_style.trim();
    if style == "none" || total_dur <= 0.5 {
        return None;
    }
    let fade_out_st = (total_dur - 0.8).max(0.2);
    let (src_expr, tremolo, gain) = if style == "cinematic_pad" {
        (
            format!(
                "aevalsrc=0.35*sin(2*PI*110.00*t)+0.25*sin(2*PI*164.81*t)+0.20*sin(2*PI*220.00*t)+0.20*sin(2*PI*261.63*t):s=44100:d={:.3}",
                total_dur + 0.5
            ),
            "tremolo=f=0.8:d=0.25",
            "-25dB",
        )
    } else {
        (
            format!(
                "aevalsrc=0.35*sin(2*PI*130.81*t)+0.25*sin(2*PI*196.00*t)+0.20*sin(2*PI*246.94*t)+0.20*sin(2*PI*293.66*t):s=44100:d={:.3}",
                total_dur + 0.5
            ),
            "tremolo=f=1.6:d=0.35",
            "-26dB",
        )
    };
    let chain = format!(
        "lowpass=f=620,highpass=f=70,{},volume={},afade=t=in:d=0.4,afade=t=out:st={:.3}:d=0.8",
        tremolo, gain, fade_out_st
    );
    Some((src_expr, chain))
}

struct PreparedSceneTiming {
    seg_name: String,
    spoken_secs: f64,
    actual_seg_dur: f64,
    word_timings: Vec<(f64, f64, String)>,
}

pub async fn render_project_mp4(
    project: &mut VideoProject,
    project_dir: &Path,
    data_dir: &Path,
    tenant_id: &str,
    key_pool: Option<&Arc<KeyPool>>,
    progress_cb: Option<ProgressCallback>,
) -> Result<(), String> {
    let (ffmpeg_exe, _) = resolve_ffmpeg_path(data_dir).await.ok_or_else(|| {
        "FFmpeg no está instalado; ejecutá 'Instalar motor' para configurarlo automáticamente."
            .to_string()
    })?;
    let ffprobe_exe = resolve_ffprobe_path(data_dir).await.ok_or_else(|| {
        "FFprobe no está instalado; ejecutá 'Instalar motor' para configurarlo automáticamente."
            .to_string()
    })?;

    if project.scenes.is_empty() {
        return Err("El proyecto no tiene escenas para renderizar.".to_string());
    }

    let (target_w, target_h) = aspect_dimensions(&project.aspect_ratio);
    tokio::fs::create_dir_all(project_dir)
        .await
        .map_err(|e| format!("Failed creating project dir: {}", e))?;

    emit_render_progress(
        &progress_cb,
        &project.id,
        0.08,
        "Midiendo duración exacta y animando escenas...",
        false,
        None,
    );

    let total = project.scenes.len();
    let mut prepared: Vec<PreparedSceneTiming> = Vec::with_capacity(total);
    let wave_w = ((target_w as f64) * 0.72).round() as u32;
    let wave_h = if target_h >= 1600 { 68u32 } else { 52u32 };

    for (idx, scene) in project.scenes.iter_mut().enumerate() {
        let visual_name = scene.visual_file.clone().ok_or_else(|| {
            format!(
                "La escena {} ('{}') no tiene recurso visual generado todavía.",
                idx + 1,
                scene.title
            )
        })?;
        let visual_path = project_dir.join(&visual_name);
        if !visual_path.exists() {
            return Err(format!(
                "No se encontró el archivo visual '{}' de la escena '{}'.",
                visual_name, scene.title
            ));
        }

        let audio_path_opt = scene
            .audio_file
            .as_ref()
            .map(|f| project_dir.join(f))
            .filter(|p| p.exists());

        let spoken_secs = if let Some(ref ap) = audio_path_opt {
            probe_media_duration(&ffprobe_exe, ap)
                .await
                .unwrap_or(scene.duration_hint_secs.max(3.0))
        } else {
            scene.duration_hint_secs.clamp(2.5, 20.0)
        };
        scene.actual_audio_secs = Some(spoken_secs);
        let raw_seg_dur = if audio_path_opt.is_some() {
            (spoken_secs + 0.45).max(2.2)
        } else {
            spoken_secs.max(2.2)
        };
        let frames = ((raw_seg_dur * 30.0).ceil() as u32).max(66);
        let exact_seg_dur = (frames as f64) / 30.0;

        // Gather word-level timings (Whisper -> Edge WordBoundary JSON -> proportional fallback)
        let word_timings = if project.subtitle_engine != "disabled" {
            let sub_source = if !scene.narration_text.trim().is_empty() {
                scene.narration_text.as_str()
            } else {
                scene.on_screen_text.as_str()
            };
            let whisper_cues = if let Some(ref ap) = audio_path_opt {
                transcribe_audio_word_timings(
                    &project.subtitle_engine,
                    ap,
                    data_dir,
                    tenant_id,
                    key_pool,
                )
                .await
                .ok()
                .flatten()
            } else {
                None
            };
            if let Some(words) = whisper_cues {
                words
            } else if let Some(edge_words) = load_edge_word_timings(project_dir, &scene.id).await {
                edge_words
            } else {
                build_proportional_words(sub_source, spoken_secs)
            }
        } else {
            Vec::new()
        };

        let pct = 0.14 + (idx as f32 / total.max(1) as f32) * 0.56;
        emit_render_progress(
            &progress_cb,
            &project.id,
            pct,
            &format!(
                "Renderizando escena {}/{}: {}...",
                idx + 1,
                total,
                scene.title
            ),
            false,
            None,
        );

        let seg_name = format!("_seg_{:03}.mp4", idx);
        let seg_path = project_dir.join(&seg_name);
        let is_video = scene.visual_kind.as_deref() == Some("video");
        let overlay_path = project_dir.join(format!("{}_overlay.png", scene.id));
        let has_overlay = !is_video && overlay_path.exists();

        let mut cmd = Command::new(&ffmpeg_exe);
        cmd.arg("-y");

        if is_video {
            cmd.args(["-stream_loop", "-1", "-i"]).arg(&visual_path);
        } else if scene.motion_effect == "static" {
            cmd.args(["-loop", "1", "-i"]).arg(&visual_path);
        } else {
            cmd.arg("-i").arg(&visual_path);
        }

        if let Some(ref ap) = audio_path_opt {
            cmd.arg("-i").arg(ap);
        } else {
            cmd.args([
                "-f",
                "lavfi",
                "-i",
                "anullsrc=channel_layout=stereo:sample_rate=44100",
            ]);
        }

        if has_overlay {
            cmd.args(["-loop", "1", "-i"]).arg(&overlay_path);
        }

        let base_vf = if is_video {
            format!(
                "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,fps=30",
                w = target_w,
                h = target_h
            )
        } else {
            let up_w = target_w + (target_w / 4);
            let up_h = target_h + (target_h / 4);
            match scene.motion_effect.as_str() {
                "static" => format!(
                    "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,fps=30",
                    w = target_w,
                    h = target_h
                ),
                "zoom_out" => format!(
                    "scale={uw}:{uh}:force_original_aspect_ratio=increase:flags=bicubic,crop={uw}:{uh},\
                    zoompan=z='max(1.0,1.16-0.16*on/{frames})':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d={frames}:s={w}x{h}:fps=30,setsar=1",
                    uw = up_w,
                    uh = up_h,
                    frames = frames,
                    w = target_w,
                    h = target_h
                ),
                "pan_left" => format!(
                    "scale={uw}:{uh}:force_original_aspect_ratio=increase:flags=bicubic,crop={uw}:{uh},\
                    zoompan=z='1.12':x='max(0,(iw-iw/zoom)*(1-on/{frames}))':y='ih/2-(ih/zoom/2)':d={frames}:s={w}x{h}:fps=30,setsar=1",
                    uw = up_w,
                    uh = up_h,
                    frames = frames,
                    w = target_w,
                    h = target_h
                ),
                _ => format!(
                    "scale={uw}:{uh}:force_original_aspect_ratio=increase:flags=bicubic,crop={uw}:{uh},\
                    zoompan=z='min(1.16,1.0+0.16*on/{frames})':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d={frames}:s={w}x{h}:fps=30,setsar=1",
                    uw = up_w,
                    uh = up_h,
                    frames = frames,
                    w = target_w,
                    h = target_h
                ),
            }
        };

        let mut fc_parts = Vec::new();
        fc_parts.push(format!("[0:v]{}[bg0]", base_vf));
        let mut last_v = "bg0";
        if has_overlay {
            fc_parts.push(format!(
                "[2:v]scale={}:{}[hud];[{}][hud]overlay=0:0:shortest=1[bg_hud]",
                target_w, target_h, last_v
            ));
            last_v = "bg_hud";
        }

        if audio_path_opt.is_some() {
            // Warm studio vocal mastering EQ + de-sibilizer + compression, split to drive live spectrum wave
            fc_parts.push(format!(
                "[1:a]highpass=f=75,equalizer=f=190:t=q:w=1.1:g=2.8,equalizer=f=3600:t=q:w=1.8:g=-4.0,equalizer=f=6800:t=q:w=2.0:g=-3.0,lowpass=f=10500,acompressor=threshold=-18dB:ratio=2.8:attack=10:release=140:makeup=2,aresample=44100,apad=whole_dur={:.3},asplit=2[aout][awave]",
                exact_seg_dur
            ));
            fc_parts.push(format!(
                "[awave]showfreqs=s={}x{}:mode=bar:fscale=log:rate=30:colors=0x22d3ee|0xa855f7,format=rgba,colorchannelmixer=aa=0.88[wave];[{}][wave]overlay=(W-w)/2:H*0.875:shortest=1,format=yuv420p[vout]",
                wave_w, wave_h, last_v
            ));
        } else {
            fc_parts.push(format!(
                "[1:a]aresample=44100,apad=whole_dur={:.3}[aout]",
                exact_seg_dur
            ));
            fc_parts.push(format!("[{}]format=yuv420p[vout]", last_v));
        }

        let filter_complex = fc_parts.join(";");
        cmd.args([
            "-filter_complex",
            &filter_complex,
            "-map",
            "[vout]",
            "-map",
            "[aout]",
            "-frames:v",
            &frames.to_string(),
            "-t",
            &format!("{:.3}", exact_seg_dur),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-ar",
            "44100",
            "-ac",
            "2",
        ])
        .arg(&seg_path);

        let out = cmd
            .output()
            .await
            .map_err(|e| format!("Error invocando FFmpeg en escena {}: {}", idx + 1, e))?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "FFmpeg falló en escena {} ('{}'): {}",
                idx + 1,
                scene.title,
                err.lines()
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }

        let actual_seg_dur = probe_media_duration(&ffprobe_exe, &seg_path)
            .await
            .unwrap_or(exact_seg_dur);

        prepared.push(PreparedSceneTiming {
            seg_name,
            spoken_secs,
            actual_seg_dur,
            word_timings,
        });
    }

    emit_render_progress(
        &progress_cb,
        &project.id,
        0.75,
        "Uniendo escenas con transiciones, mezcla musical y subtítulos dinámicos...",
        false,
        None,
    );

    let trans_dur = if total >= 2 && prepared.iter().all(|s| s.actual_seg_dur >= 1.5) {
        0.35f64
    } else {
        0.0f64
    };

    let mut all_srt_cues: Vec<SubtitleCue> = Vec::new();
    let mut all_ass_cues: Vec<AssCue> = Vec::new();
    let mut timeline_cursor = 0.0f64;

    for (idx, item) in prepared.iter().enumerate() {
        if project.subtitle_engine != "disabled" && !item.word_timings.is_empty() {
            let (srt_cues, ass_cues) = group_words_into_cues_and_karaoke(
                &item.word_timings,
                timeline_cursor,
                item.spoken_secs,
                &project.subtitle_style,
            );
            all_srt_cues.extend(srt_cues);
            all_ass_cues.extend(ass_cues);
        }
        let step_advance = if idx + 1 < total {
            (item.actual_seg_dur - trans_dur).max(0.2)
        } else {
            item.actual_seg_dur
        };
        timeline_cursor += step_advance;
    }

    let has_subs = project.subtitle_engine != "disabled" && !all_ass_cues.is_empty();
    if has_subs {
        let srt_txt = render_srt_content(&all_srt_cues);
        let ass_txt =
            render_ass_content(&all_ass_cues, target_w, target_h, &project.subtitle_style);
        tokio::fs::write(project_dir.join("subtitles.srt"), srt_txt)
            .await
            .map_err(|e| format!("Failed writing SRT: {}", e))?;
        tokio::fs::write(project_dir.join("subtitles.ass"), ass_txt)
            .await
            .map_err(|e| format!("Failed writing ASS: {}", e))?;
    }

    let mut final_cmd = Command::new(&ffmpeg_exe);
    final_cmd.current_dir(project_dir).arg("-y");
    for item in &prepared {
        final_cmd.arg("-i").arg(&item.seg_name);
    }

    let bgm_opt = bgm_filter_spec(&project.bgm_style, timeline_cursor);
    if let Some((ref bgm_src, _)) = bgm_opt {
        final_cmd.args(["-f", "lavfi", "-i", bgm_src]);
    }

    let mut final_fc = Vec::new();
    let video_label = if total == 1 {
        "0:v".to_string()
    } else {
        let transitions = ["smoothleft", "wipeup", "slideleft", "dissolve"];
        let mut prev = "0:v".to_string();
        let mut offset = (prepared[0].actual_seg_dur - trans_dur).max(0.1);
        for k in 1..total {
            let out_lbl = format!("vx{}", k);
            let t_name = transitions[(k - 1) % transitions.len()];
            final_fc.push(format!(
                "[{}][{}:v]xfade=transition={}:duration={:.3}:offset={:.3}[{}]",
                prev, k, t_name, trans_dur, offset, out_lbl
            ));
            prev = out_lbl;
            if k + 1 < total {
                offset += (prepared[k].actual_seg_dur - trans_dur).max(0.1);
            }
        }
        prev
    };

    if has_subs {
        final_fc.push(format!(
            "[{}]ass=subtitles.ass,format=yuv420p[vfinal]",
            video_label
        ));
    } else {
        final_fc.push(format!("[{}]format=yuv420p[vfinal]", video_label));
    }

    let voice_bus = if total == 1 {
        final_fc.push("[0:a]loudnorm=I=-15:TP=-1.5:LRA=9[vbus]".to_string());
        "vbus".to_string()
    } else {
        let mut prev_a = "0:a".to_string();
        for k in 1..total {
            let out_a = format!("ax{}", k);
            let norm_suffix = if k + 1 == total {
                ",loudnorm=I=-15:TP=-1.5:LRA=9"
            } else {
                ""
            };
            final_fc.push(format!(
                "[{}][{}:a]acrossfade=d={:.3}:c1=tri:c2=tri{}[{}]",
                prev_a, k, trans_dur, norm_suffix, out_a
            ));
            prev_a = out_a;
        }
        prev_a
    };

    if let Some((_, ref bgm_chain)) = bgm_opt {
        let bgm_idx = total;
        final_fc.push(format!("[{}:a]{}[bgm]", bgm_idx, bgm_chain));
        final_fc.push(format!(
            "[{}][bgm]amix=inputs=2:duration=first:normalize=0[afinal]",
            voice_bus
        ));
    } else {
        final_fc.push(format!("[{}]anull[afinal]", voice_bus));
    }

    let final_filter_complex = final_fc.join(";");
    final_cmd.args([
        "-filter_complex",
        &final_filter_complex,
        "-map",
        "[vfinal]",
        "-map",
        "[afinal]",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "19",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
        "output.mp4",
    ]);

    let final_out = final_cmd
        .output()
        .await
        .map_err(|e| format!("Error en render final MP4: {}", e))?;

    if !final_out.status.success() {
        let err = String::from_utf8_lossy(&final_out.stderr);
        return Err(format!(
            "FFmpeg render final falló: {}",
            err.lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }

    // Cleanup intermediate segments cleanly
    for item in &prepared {
        let _ = tokio::fs::remove_file(project_dir.join(&item.seg_name)).await;
    }

    project.output_mp4 = Some("output.mp4".to_string());
    project.output_srt = if has_subs {
        Some("subtitles.srt".to_string())
    } else {
        None
    };
    project.status = "rendered".to_string();
    project.last_error = None;

    emit_render_progress(
        &progress_cb,
        &project.id,
        1.0,
        "Video MP4 exportado y listo para descargar.",
        true,
        None,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtitle_cues_and_srt_formatting() {
        let words = build_proportional_words(
            "Descubrí cómo crear videos con inteligencia artificial local.",
            6.0,
        );
        assert!(words.len() >= 8);
        let (srt_cues, ass_cues) =
            group_words_into_cues_and_karaoke(&words, 0.0, 6.0, "tiktok_bold");
        assert!(srt_cues.len() >= 2);
        assert_eq!(srt_cues.first().map(|c| c.start), Some(0.0));
        assert_eq!(ass_cues.len(), words.len());
        assert!(ass_cues[0].ass_text.contains("\\fscx108"));

        let srt = render_srt_content(&srt_cues);
        assert!(srt.contains("00:00:00,000 -->"));
        let ass = render_ass_content(&ass_cues, 1080, 1920, "tiktok_bold");
        assert!(ass.contains("PlayResX: 1080"));
        assert!(ass.contains("Dialogue: 0,"));
        assert!(bgm_filter_spec("ambient_pulse", 12.0).is_some());
        assert!(bgm_filter_spec("none", 12.0).is_none());
    }
}
