use super::types::{
    preset_engines, CreatorScriptSummary, VideoModuleConfig, VideoProject, VideoScene,
};
use crate::enclave::TenantDB;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const VIDEO_STATE_KEY: &str = "module_data:aegis.domain.video";
const VIDEO_CONFIG_KEY: &str = "module_config:aegis.domain.video";
const CREATOR_STATE_KEY: &str = "module_data:aegis.domain.creator";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoModuleState {
    #[serde(default)]
    pub projects: Vec<VideoProject>,
}

pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn new_id(prefix: &str) -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("{}-{}", prefix, &raw[..8])
}

/// Validates a path segment (tenant, project, scene or filename) preventing traversal.
pub fn validate_safe_segment(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('.')
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        return Err(format!("Identificador o nombre inválido: '{}'", raw));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return Err(format!(
            "El identificador '{}' contiene caracteres no permitidos",
            raw
        ));
    }
    Ok(trimmed.to_string())
}

pub fn tenant_video_root(data_dir: &Path, tenant_id: &str) -> Result<PathBuf, String> {
    let safe_tenant = validate_safe_segment(tenant_id)?;
    Ok(data_dir
        .join("users")
        .join(safe_tenant)
        .join("video_studio"))
}

pub fn project_media_dir(
    data_dir: &Path,
    tenant_id: &str,
    project_id: &str,
) -> Result<PathBuf, String> {
    let root = tenant_video_root(data_dir, tenant_id)?;
    let safe_proj = validate_safe_segment(project_id)?;
    Ok(root.join(safe_proj))
}

pub fn load_state(db: &TenantDB) -> Result<VideoModuleState, String> {
    match db
        .get_kv(VIDEO_STATE_KEY)
        .map_err(|e| format!("DB read error: {}", e))?
    {
        Some(raw) if !raw.trim().is_empty() => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        _ => Ok(VideoModuleState::default()),
    }
}

pub fn save_state(db: &TenantDB, state: &VideoModuleState) -> Result<(), String> {
    let serialized = serde_json::to_string(state).map_err(|e| format!("Serialize error: {}", e))?;
    db.set_kv(VIDEO_STATE_KEY, &serialized)
        .map_err(|e| format!("DB write error: {}", e))
}

pub fn load_config(db: &TenantDB) -> Result<VideoModuleConfig, String> {
    match db
        .get_kv(VIDEO_CONFIG_KEY)
        .map_err(|e| format!("DB read error: {}", e))?
    {
        Some(raw) if !raw.trim().is_empty() => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        _ => Ok(VideoModuleConfig::default()),
    }
}

pub fn save_config(db: &TenantDB, cfg: &VideoModuleConfig) -> Result<(), String> {
    let serialized = serde_json::to_string(cfg).map_err(|e| format!("Serialize error: {}", e))?;
    db.set_kv(VIDEO_CONFIG_KEY, &serialized)
        .map_err(|e| format!("DB write error: {}", e))
}

pub fn list_creator_scripts(db: &TenantDB) -> Vec<CreatorScriptSummary> {
    let Ok(Some(raw)) = db.get_kv(CREATOR_STATE_KEY) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(arr) = val.get("scripts").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let title = item.get("title")?.as_str()?.to_string();
            Some(CreatorScriptSummary {
                id,
                title,
                platform: item
                    .get("platform")
                    .and_then(Value::as_str)
                    .unwrap_or("tiktok_reels")
                    .to_string(),
                hook_0_3s: item
                    .get("hook_0_3s")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                body_script: item
                    .get("body_script")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                b_roll_notes: item
                    .get("b_roll_notes")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                cta: item
                    .get("cta")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

pub fn find_creator_script(db: &TenantDB, script_id: &str) -> Option<CreatorScriptSummary> {
    list_creator_scripts(db)
        .into_iter()
        .find(|s| s.id == script_id.trim())
}

/// Strips LLM wrapper instructions ("Video promocional vertical...", "Tono moderno...", "Generá recursos, exportá el MP4...")
/// so TTS never reads meta-instructions out loud when falling back to deterministic chunking.
pub fn strip_video_command_boilerplate(raw: &str) -> String {
    let mut cleaned_sentences = Vec::new();
    for piece in raw.split('\n') {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Cut trailing production instructions if they appear inline.
        let lower = trimmed.to_lowercase();
        let cut_markers = [
            "generá recursos",
            "genera recursos",
            "generá todo",
            "genera todo",
            "después generá",
            "despues genera",
            "exportá el mp4",
            "exporta el mp4",
            "exportalo a mp4",
            "mostrame la tarjeta",
            "pasame el reproductor",
            "tono moderno",
            "tono ",
        ];
        let mut end_idx = trimmed.len();
        for marker in cut_markers {
            if let Some(pos) = lower.find(marker) {
                // Only cut if preceded by sentence boundary, comma, or start of line
                let prefix = trimmed[..pos].trim_end();
                if prefix.is_empty()
                    || prefix.ends_with('.')
                    || prefix.ends_with(',')
                    || prefix.ends_with(';')
                {
                    end_idx = end_idx.min(pos);
                }
            }
        }
        let head = trimmed[..end_idx]
            .trim()
            .trim_end_matches(&['.', ',', ';', ' '][..]);
        if !head.is_empty() {
            cleaned_sentences.push(head.to_string());
        }
    }
    let joined = cleaned_sentences.join(". ");
    // Strip leading "Video promocional vertical de Aegis OS: ..." prefix while preserving the description after ":"
    let j_lower = joined.to_lowercase();
    for lead in [
        "video promocional vertical de ",
        "video promocional de ",
        "creame un proyecto nuevo ",
        "creame un video ",
        "crea un proyecto nuevo ",
    ] {
        if j_lower.starts_with(lead) {
            if let Some(colon) = joined.find(':') {
                let after = joined[colon + 1..].trim();
                if after.len() >= 12 {
                    return after.to_string();
                }
            }
        }
    }
    joined
}

fn find_numbered_scene_markers(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut markers = Vec::new();
    for i in 0..chars.len() {
        let (byte_pos, ch) = chars[i];
        if !('1'..='9').contains(&ch) {
            continue;
        }
        let prev_ok = if i == 0 {
            true
        } else {
            let prev = chars[i - 1].1;
            prev.is_whitespace() || matches!(prev, ':' | ';' | ',' | '(' | '-' | '\n')
        };
        if !prev_ok || i + 2 >= chars.len() {
            continue;
        }
        let sep = chars[i + 1].1;
        let after = chars[i + 2].1;
        if (sep == ')' || sep == '.') && after.is_whitespace() {
            markers.push((byte_pos, chars[i + 2].0 + after.len_utf8()));
        }
    }
    markers
}

fn split_scene_list_items(inner: &str) -> Vec<String> {
    let mut parts: Vec<String> = inner
        .split(&[',', ';', '\n'][..])
        .map(|s| s.trim())
        .map(|s| {
            let lower = s.to_lowercase();
            if lower.starts_with("y ") || lower.starts_with("e ") {
                s[2..].trim()
            } else {
                s
            }
        })
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    if (2..=3).contains(&parts.len()) {
        if let Some(last) = parts.pop() {
            let lower_last = last.to_lowercase();
            if let Some(y_pos) = lower_last.rfind(" y ") {
                let left = last[..y_pos].trim().to_string();
                let right = last[y_pos + 3..].trim().to_string();
                let r_lower = right.to_lowercase();
                let looks_like_closing_scene = parts.len() == 2
                    || r_lower.contains("cta")
                    || r_lower.contains("cierre")
                    || r_lower.contains("final")
                    || r_lower.contains("prob");
                if !left.is_empty() && !right.is_empty() && looks_like_closing_scene {
                    parts.push(left);
                    parts.push(right);
                } else {
                    parts.push(last);
                }
            } else {
                parts.push(last);
            }
        }
    }
    parts
}

fn extract_brand_hint(raw: &str) -> String {
    if raw.to_lowercase().contains("aegis") {
        "Aegis".to_string()
    } else {
        "tu plataforma".to_string()
    }
}

fn strip_scene_prefix_label(raw: &str) -> (&'static str, String) {
    let mut s = raw
        .trim()
        .trim_matches(&['(', ')', '.', ',', ';'][..])
        .trim()
        .to_string();
    let lower = s.to_lowercase();
    let mut kind = "body";
    if lower.starts_with("hook") || lower.starts_with("gancho") {
        kind = "hook";
    } else if lower.starts_with("cta")
        || lower.starts_with("cierre")
        || lower.starts_with("llamado")
    {
        kind = "cta";
    }

    for prefix in [
        "hook (0-3s) sobre ",
        "hook 0-3s sobre ",
        "hook (0-3s):",
        "hook 0-3s:",
        "hook 0-3s ",
        "hook:",
        "hook -",
        "hook sobre ",
        "hook ",
        "gancho:",
        "cta final:",
        "cta final",
        "cta:",
        "cta ",
        "cierre / cta:",
        "cierre:",
    ] {
        let cur_low = s.to_lowercase();
        if cur_low.starts_with(prefix) {
            s = s[prefix.len()..].trim().to_string();
            break;
        }
    }
    (kind, s)
}

fn build_scene_from_raw_segment(
    idx: usize,
    total: usize,
    raw_seg: &str,
    title_hint: &str,
    brand_hint: &str,
) -> VideoScene {
    let (kind_hint, body_clean) = strip_scene_prefix_label(raw_seg);
    let lower = body_clean.to_lowercase();
    let raw_lower = raw_seg.to_lowercase();
    let words = body_clean.split_whitespace().count();

    let (scene_title, narration, on_screen) = if kind_hint == "cta"
        || raw_lower == "cta"
        || (words <= 2 && (lower == "final" || lower == "cta"))
    {
        (
            "Cierre / CTA".to_string(),
            format!(
                "Centralizá todo tu trabajo en un solo lugar. Probá {} hoy.",
                brand_hint
            ),
            format!("PROBÁ {} HOY", brand_hint.to_uppercase()),
        )
    } else if raw_lower.contains("10 apps")
        || lower.contains("10 apps")
        || lower.contains("diez apps")
    {
        let nar = if words <= 4 {
            "¿Seguís perdiendo tiempo saltando entre diez apps distintas todos los días?"
                .to_string()
        } else if lower.starts_with("dejar de ") {
            format!("Dejá de {}", &body_clean["dejar de ".len()..])
        } else {
            body_clean.clone()
        };
        (
            "Hook (0-3s)".to_string(),
            nar,
            "¿10 APPS DISPERSAS?".to_string(),
        )
    } else if lower.contains("módulo") || lower.contains("modulo") {
        let nar = if words <= 4 {
            format!(
                "En {}, activás solo los módulos a medida para programar, crear contenido o llevar tu negocio.",
                brand_hint
            )
        } else if lower.starts_with("módulos") || lower.starts_with("modulos") {
            format!("Activá {}", body_clean)
        } else {
            body_clean.clone()
        };
        (
            "Módulos a Medida".to_string(),
            nar,
            "MÓDULOS A MEDIDA".to_string(),
        )
    } else if lower.contains("privacidad") || lower.contains("enclave") || lower.contains("cifrad")
    {
        let nar = if words <= 5 {
            "Toda tu memoria y tus datos viven protegidos bajo un enclave local cifrado."
                .to_string()
        } else {
            body_clean.clone()
        };
        (
            "Privacidad Cifrada".to_string(),
            nar,
            "ENCLAVE CIFRADO".to_string(),
        )
    } else {
        let stitle = if idx == 0 {
            "Hook (0-3s)".to_string()
        } else if idx + 1 == total {
            "Cierre / CTA".to_string()
        } else {
            format!("Escena {}", idx + 1)
        };
        (stitle, body_clean.clone(), shorten_overlay(&body_clean, 40))
    };

    let motion = match idx % 3 {
        0 => "zoom_in",
        1 => "zoom_out",
        _ => "pan_left",
    };

    VideoScene {
        id: new_id("scn"),
        order: idx,
        title: scene_title,
        narration_text: narration.clone(),
        on_screen_text: on_screen,
        visual_prompt: format!(
            "High-contrast sleek dark-mode commercial visual representing '{}', futuristic cyber lighting, no embedded text",
            narration
        ),
        stock_keywords: extract_stock_keywords(&narration, title_hint),
        duration_hint_secs: estimate_speech_secs(&narration),
        motion_effect: motion.to_string(),
        voice_override: None,
        visual_override: None,
        audio_file: None,
        visual_file: None,
        visual_kind: None,
        actual_audio_secs: None,
        status: "pending".to_string(),
        error: None,
    }
}

fn extract_structured_scenes_from_prompt(title: &str, body: &str) -> Option<Vec<VideoScene>> {
    let brand_hint = extract_brand_hint(&format!("{} {}", title, body));
    // 1. Numbered scenes: "1) ..., 2) ..., 3) ..., 4) ..." or "1. ... 2. ..."
    let markers = find_numbered_scene_markers(body);
    if markers.len() >= 2 {
        let mut raw_items = Vec::new();
        for i in 0..markers.len() {
            let content_start = markers[i].1;
            let content_end = if i + 1 < markers.len() {
                markers[i + 1].0
            } else {
                body.len()
            };
            let slice = strip_video_command_boilerplate(&body[content_start..content_end]);
            let clean = slice
                .trim()
                .trim_end_matches(&[',', ';', '.', ')'][..])
                .trim();
            let clean_lower = clean.to_lowercase();
            let final_item = if clean_lower.ends_with(" y") && clean.len() > 2 {
                clean[..clean.len() - 2].trim().trim_end_matches(',')
            } else {
                clean
            };
            if !final_item.is_empty() {
                raw_items.push(final_item.to_string());
            }
        }
        if raw_items.len() >= 2 {
            let total = raw_items.len();
            return Some(
                raw_items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        build_scene_from_raw_segment(i, total, item, title, &brand_hint)
                    })
                    .collect(),
            );
        }
    }

    // 2. Parenthesized scene list near "escena"/"escenas" or containing scene items
    let mut best_group: Option<(i32, Vec<String>)> = None;
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            if let Some(rel_end) = body[i + 1..].find(')') {
                let inner = body[i + 1..i + 1 + rel_end].trim();
                let inner_lower = inner.to_lowercase();
                let looks_like_config = (inner_lower.contains("preset ")
                    || inner_lower.contains("voz ")
                    || inner_lower.contains("free_builtin")
                    || inner_lower.contains("gemini_studio"))
                    && !inner_lower.contains("hook")
                    && !inner_lower.contains("cta");
                if !looks_like_config && inner.contains(',') {
                    let items = split_scene_list_items(inner);
                    if items.len() >= 2 {
                        let context_before = &body[i.saturating_sub(64)..i].to_lowercase();
                        let mut score = items.len() as i32;
                        if context_before.contains("escena") {
                            score += 10;
                        }
                        if inner_lower.contains("hook") || inner_lower.contains("cta") {
                            score += 5;
                        }
                        if best_group.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                            best_group = Some((score, items));
                        }
                    }
                }
                i += rel_end + 2;
                continue;
            }
        }
        i += 1;
    }

    if let Some((_, items)) = best_group {
        let total = items.len();
        return Some(
            items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    build_scene_from_raw_segment(idx, total, item, title, &brand_hint)
                })
                .collect(),
        );
    }

    // 3. Colon list after "escenas:" without parens
    let lower_body = body.to_lowercase();
    if let Some(es_pos) = lower_body.find("escena") {
        if let Some(rel_colon) = body[es_pos..].find(':') {
            let after_colon = strip_video_command_boilerplate(&body[es_pos + rel_colon + 1..]);
            let items = split_scene_list_items(&after_colon);
            if items.len() >= 2 {
                let total = items.len();
                return Some(
                    items
                        .iter()
                        .enumerate()
                        .map(|(idx, item)| {
                            build_scene_from_raw_segment(idx, total, item, title, &brand_hint)
                        })
                        .collect(),
                );
            }
        }
    }

    None
}

/// Parses heterogeneous LLM scene arrays (with omitted `id`/`order` or alternate field names like
/// `narration`/`voiceover`/`script`/`text`) into validated `VideoScene` structs.
pub fn parse_flexible_scenes(raw_val: &Value, fallback_title: &str) -> Option<Vec<VideoScene>> {
    let owned_val;
    let array_ref = match raw_val {
        Value::Array(arr) => arr,
        Value::Object(map) => map.get("scenes").and_then(Value::as_array)?,
        Value::String(s) => {
            owned_val = serde_json::from_str::<Value>(s).ok()?;
            match &owned_val {
                Value::Array(arr) => arr,
                Value::Object(map) => map.get("scenes").and_then(Value::as_array)?,
                _ => return None,
            }
        }
        _ => return None,
    };

    let get_first_str = |obj: &serde_json::Map<String, Value>, keys: &[&str]| -> Option<String> {
        for k in keys {
            if let Some(s) = obj.get(*k).and_then(Value::as_str) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    };

    let mut out = Vec::new();
    for (idx, item) in array_ref.iter().enumerate() {
        if let Some(raw_str) = item.as_str() {
            let cleaned = raw_str.trim();
            if cleaned.is_empty() {
                continue;
            }
            let total = array_ref.len().max(1);
            let mut built = build_scene_from_raw_segment(
                idx,
                total,
                cleaned,
                fallback_title,
                &extract_brand_hint(fallback_title),
            );
            built.order = out.len();
            out.push(built);
            continue;
        }
        let Some(obj) = item.as_object() else {
            continue;
        };

        let title = get_first_str(obj, &["title", "name", "heading", "scene"])
            .unwrap_or_else(|| format!("Escena {}", idx + 1));
        let mut narration = get_first_str(
            obj,
            &[
                "narration_text",
                "narration",
                "voiceover",
                "voz",
                "script",
                "text",
                "guion",
                "description",
            ],
        )
        .unwrap_or_default();

        let overlay_raw = get_first_str(
            obj,
            &[
                "on_screen_text",
                "overlay_text",
                "headline",
                "caption",
                "overlay",
            ],
        );
        if narration.is_empty() {
            if let Some(ref ov) = overlay_raw {
                narration = ov.clone();
            } else if !title.starts_with("Escena ") {
                narration = title.clone();
            } else {
                continue;
            }
        }

        let on_screen_text = overlay_raw.unwrap_or_else(|| shorten_overlay(&narration, 44));
        let visual_prompt =
            get_first_str(obj, &["visual_prompt", "visual", "image_prompt", "b_roll"])
                .unwrap_or_else(|| {
                    format!(
                        "Clean dark-mode futuristic editorial visual representing: '{}'",
                        narration
                    )
                });
        let stock_keywords = get_first_str(obj, &["stock_keywords", "keywords"])
            .unwrap_or_else(|| extract_stock_keywords(&narration, fallback_title));
        let duration_hint_secs = obj
            .get("duration_hint_secs")
            .or_else(|| obj.get("duration"))
            .and_then(Value::as_f64)
            .unwrap_or_else(|| estimate_speech_secs(&narration))
            .clamp(2.5, 25.0);
        let motion_effect = obj
            .get("motion_effect")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|m| matches!(*m, "zoom_in" | "zoom_out" | "pan_left" | "static"))
            .unwrap_or(match idx % 3 {
                0 => "zoom_in",
                1 => "zoom_out",
                _ => "pan_left",
            })
            .to_string();

        let id = get_first_str(obj, &["id"])
            .filter(|s| validate_safe_segment(s).is_ok())
            .unwrap_or_else(|| new_id("scn"));

        out.push(VideoScene {
            id,
            order: out.len(),
            title,
            narration_text: narration,
            on_screen_text,
            visual_prompt,
            stock_keywords,
            duration_hint_secs,
            motion_effect,
            voice_override: get_first_str(obj, &["voice_override"]).filter(|v| v != "inherit"),
            visual_override: get_first_str(obj, &["visual_override"]).filter(|v| v != "inherit"),
            audio_file: None,
            visual_file: None,
            visual_kind: None,
            actual_audio_secs: None,
            status: "pending".to_string(),
            error: None,
        });
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Deterministic script-to-scenes splitter used both as instant default and offline fallback.
pub fn split_text_to_scenes(
    title: &str,
    hook: Option<&str>,
    body: &str,
    cta: Option<&str>,
    b_roll_notes: Option<&str>,
) -> Vec<VideoScene> {
    if hook.map(str::trim).filter(|s| !s.is_empty()).is_none()
        && cta.map(str::trim).filter(|s| !s.is_empty()).is_none()
    {
        if let Some(structured) = extract_structured_scenes_from_prompt(title, body) {
            return structured;
        }
    }

    let cleaned_body = strip_video_command_boilerplate(body);
    let effective_body = if cleaned_body.is_empty() {
        body
    } else {
        cleaned_body.as_str()
    };

    let mut scenes = Vec::new();
    let b_roll = b_roll_notes.unwrap_or("").trim();

    if let Some(h) = hook.map(str::trim).filter(|s| !s.is_empty()) {
        scenes.push(VideoScene {
            id: new_id("scn"),
            order: scenes.len(),
            title: "Hook (0-3s)".to_string(),
            narration_text: h.to_string(),
            on_screen_text: shorten_overlay(h, 52),
            visual_prompt: format!(
                "High-contrast cinematic vertical shot illustrating hook: '{}'. {}",
                h, b_roll
            )
            .trim()
            .to_string(),
            stock_keywords: extract_stock_keywords(h, title),
            duration_hint_secs: estimate_speech_secs(h),
            motion_effect: "zoom_in".to_string(),
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

    let chunks = chunk_narration_paragraphs(effective_body);
    let total_body = chunks.len();
    for (idx, chunk) in chunks.into_iter().enumerate() {
        let scene_num = idx + 1;
        let motion = match idx % 3 {
            0 => "zoom_in",
            1 => "zoom_out",
            _ => "pan_left",
        };
        scenes.push(VideoScene {
            id: new_id("scn"),
            order: scenes.len(),
            title: if total_body == 1 {
                "Idea Clave".to_string()
            } else {
                format!("Punto {}", scene_num)
            },
            narration_text: chunk.clone(),
            on_screen_text: shorten_overlay(&chunk, 42),
            visual_prompt: format!(
                "Clean dark-mode futuristic editorial visual representing: '{}'. {}",
                chunk, b_roll
            )
            .trim()
            .to_string(),
            stock_keywords: extract_stock_keywords(&chunk, title),
            duration_hint_secs: estimate_speech_secs(&chunk),
            motion_effect: motion.to_string(),
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

    if let Some(c) = cta.map(str::trim).filter(|s| !s.is_empty()) {
        scenes.push(VideoScene {
            id: new_id("scn"),
            order: scenes.len(),
            title: "Cierre / CTA".to_string(),
            narration_text: c.to_string(),
            on_screen_text: shorten_overlay(c, 40),
            visual_prompt: format!("Clean dynamic closing frame for CTA: '{}'", c),
            stock_keywords: "technology futuristic minimal".to_string(),
            duration_hint_secs: estimate_speech_secs(c),
            motion_effect: "zoom_out".to_string(),
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

    if scenes.is_empty() {
        let fallback_text = if title.trim().is_empty() {
            "Video creado con Aegis Video Studio"
        } else {
            title.trim()
        };
        scenes.push(VideoScene {
            id: new_id("scn"),
            order: 0,
            title: "Escena 1".to_string(),
            narration_text: fallback_text.to_string(),
            on_screen_text: shorten_overlay(fallback_text, 40),
            visual_prompt: format!("Visual minimalista y tecnológico sobre: {}", fallback_text),
            stock_keywords: extract_stock_keywords(fallback_text, title),
            duration_hint_secs: 4.5,
            motion_effect: "zoom_in".to_string(),
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

    scenes
}

pub(crate) fn estimate_speech_secs(text: &str) -> f64 {
    let words = text.split_whitespace().count().max(1);
    let est = (words as f64 / 2.4) + 0.8;
    est.clamp(3.0, 14.0)
}

pub(crate) fn shorten_overlay(text: &str, max_chars: usize) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= max_chars {
        return clean
            .trim_end_matches(&['.', ',', ';', ':'][..])
            .to_string();
    }
    let mut acc = String::new();
    for (word_count, w) in clean.split_whitespace().enumerate() {
        let next_len = if acc.is_empty() {
            w.chars().count()
        } else {
            acc.chars().count() + 1 + w.chars().count()
        };
        if next_len > max_chars && !acc.is_empty() {
            break;
        }
        if word_count >= 7 {
            break;
        }
        if !acc.is_empty() {
            acc.push(' ');
        }
        acc.push_str(w);
    }
    acc.trim_end_matches(&['.', ',', ';', ':', '-', '—'][..])
        .to_string()
}

fn chunk_narration_paragraphs(body: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let sentences: Vec<&str> = trimmed
            .split_inclusive(&['.', '!', '?', ';'][..])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if sentences.is_empty() {
            chunks.push(trimmed.to_string());
            continue;
        }
        // When a single paragraph has 2+ sentences, keep each sentence as a punchy scene unless very short.
        if sentences.len() >= 2
            && chunks.is_empty()
            && body.lines().filter(|l| !l.trim().is_empty()).count() == 1
        {
            for sent in sentences {
                chunks.push(sent.to_string());
            }
            continue;
        }
        let mut current = String::new();
        for sent in sentences {
            let next_words = current.split_whitespace().count() + sent.split_whitespace().count();
            if !current.is_empty() && next_words > 26 {
                chunks.push(current.trim().to_string());
                current = sent.to_string();
            } else {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(sent);
            }
        }
        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }
    }
    chunks
}

pub(crate) fn extract_stock_keywords(text: &str, fallback_title: &str) -> String {
    let stop = [
        "para", "como", "cómo", "pero", "este", "esta", "esto", "sobre", "entre", "todos", "todas",
        "desde", "porque", "cuando", "donde", "tiene", "hacer", "puede", "puedes", "solo", "más",
        "with", "that", "this", "from", "your", "about", "what",
    ];
    let combined = format!("{} {}", text, fallback_title);
    let mut words: Vec<String> = Vec::new();
    for tok in combined.split(|c: char| !c.is_alphanumeric()) {
        let lower = tok.to_lowercase();
        if lower.len() >= 4 && !stop.contains(&lower.as_str()) && !words.contains(&lower) {
            words.push(lower);
            if words.len() >= 3 {
                break;
            }
        }
    }
    if words.is_empty() {
        "technology workspace abstract".to_string()
    } else {
        words.join(" ")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_new_project(
    db: &TenantDB,
    title: String,
    prompt: String,
    source_script_id: Option<String>,
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
    explicit_scenes: Option<Vec<VideoScene>>,
) -> Result<VideoProject, String> {
    let cfg = load_config(db).unwrap_or_default();
    let chosen_preset = preset
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| cfg.default_preset.clone());

    let (p_voice, p_vis, p_hook, p_sub) = if chosen_preset == "custom" {
        (
            cfg.default_voice_engine.as_str(),
            cfg.default_visual_engine.as_str(),
            cfg.default_hook_video_engine.as_str(),
            cfg.default_subtitle_engine.as_str(),
        )
    } else {
        preset_engines(&chosen_preset)
    };

    let final_voice = voice_engine
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| p_voice.to_string());
    let final_visual = visual_engine
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| p_vis.to_string());
    let final_hook = hook_video_engine
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| p_hook.to_string());
    let final_sub = subtitle_engine
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| p_sub.to_string());
    let final_voice_id = voice_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            if final_voice == "openai_tts" {
                "onyx".to_string()
            } else if final_voice == "gemini_tts" {
                "Charon".to_string()
            } else if final_voice == "elevenlabs" {
                "EXAVITQu4vr4xnSDxMaL".to_string()
            } else {
                cfg.default_voice_id.clone()
            }
        });

    let mut resolved_title = title.trim().to_string();
    let mut resolved_prompt = prompt.trim().to_string();
    let mut resolved_platform = platform
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "tiktok_reels".to_string());

    let scenes = if let Some(mut provided) = explicit_scenes {
        for (i, s) in provided.iter_mut().enumerate() {
            if s.id.trim().is_empty() {
                s.id = new_id("scn");
            }
            s.order = i;
            if s.title.trim().is_empty() {
                s.title = format!("Escena {}", i + 1);
            }
            if s.narration_text.trim().is_empty() {
                if !s.on_screen_text.trim().is_empty() {
                    s.narration_text = s.on_screen_text.clone();
                } else {
                    s.narration_text = s.title.clone();
                }
            }
            if s.on_screen_text.trim().is_empty() {
                s.on_screen_text = shorten_overlay(&s.narration_text, 44);
            }
            if s.visual_prompt.trim().is_empty() {
                s.visual_prompt = format!(
                    "Clean dark-mode futuristic editorial visual representing: '{}'",
                    s.narration_text
                );
            }
            if s.stock_keywords.trim().is_empty() {
                s.stock_keywords = extract_stock_keywords(&s.narration_text, &resolved_title);
            }
            if s.status.trim().is_empty() {
                s.status = "pending".to_string();
            }
        }
        provided
    } else if let Some(ref sid) = source_script_id {
        if let Some(script) = find_creator_script(db, sid) {
            if resolved_title.is_empty() {
                resolved_title = script.title.clone();
            }
            if resolved_prompt.is_empty() {
                resolved_prompt = script.body_script.clone();
            }
            resolved_platform = script.platform.clone();
            split_text_to_scenes(
                &resolved_title,
                Some(&script.hook_0_3s),
                &script.body_script,
                Some(&script.cta),
                Some(&script.b_roll_notes),
            )
        } else {
            split_text_to_scenes(&resolved_title, None, &resolved_prompt, None, None)
        }
    } else {
        split_text_to_scenes(&resolved_title, None, &resolved_prompt, None, None)
    };

    if resolved_title.is_empty() {
        resolved_title = "Video IA sin título".to_string();
    }

    let now = now_iso();
    Ok(VideoProject {
        id: new_id("vid"),
        title: resolved_title,
        prompt: resolved_prompt,
        source_script_id,
        platform: resolved_platform,
        aspect_ratio: aspect_ratio
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "9:16".to_string()),
        preset: chosen_preset,
        voice_engine: final_voice,
        voice_id: final_voice_id,
        visual_engine: final_visual,
        hook_video_engine: final_hook,
        subtitle_engine: final_sub,
        subtitle_style: subtitle_style
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| cfg.default_subtitle_style.clone()),
        bgm_style: bgm_style
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| cfg.default_bgm_style.clone()),
        scenes,
        output_mp4: None,
        output_srt: None,
        status: "draft".to_string(),
        last_error: None,
        fallbacks_applied: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_flexible_scenes_without_id_or_order() {
        let raw = json!([
            {
                "title": "Hook",
                "narration_text": "¿Seguís usando diez apps sueltas para trabajar?",
                "on_screen_text": "¿DIEZ APPS SUELTAS?"
            },
            {
                "name": "Módulos a medida",
                "voiceover": "Activá solo los módulos de código, video o negocio que necesitás.",
                "visual": "Modular dark neon architecture blocks"
            },
            {
                "scene": "Privacidad",
                "text": "Tu memoria y archivos viven en un enclave local cifrado."
            },
            {
                "title": "CTA",
                "script": "Probá Aegis OS hoy mismo."
            }
        ]);

        let parsed = parse_flexible_scenes(&raw, "Promo Aegis").expect("must parse 4 scenes");
        assert_eq!(parsed.len(), 4);
        assert!(parsed.iter().all(|s| s.id.starts_with("scn-")));
        assert_eq!(parsed[0].order, 0);
        assert_eq!(parsed[1].order, 1);
        assert_eq!(parsed[2].order, 2);
        assert_eq!(parsed[3].order, 3);
        assert_eq!(parsed[1].title, "Módulos a medida");
        assert!(parsed[1].narration_text.contains("Activá solo"));
        assert!(!parsed[2].visual_prompt.is_empty());
        assert!(!parsed[3].on_screen_text.is_empty());
    }

    #[test]
    fn test_split_text_to_scenes_parenthesized_4_scenes_and_no_meta_boilerplate() {
        let prompt = "Creame un proyecto nuevo vertical 9:16 con preset free_builtin, voz en-US-AndrewMultilingualNeural y 4 escenas promocionando Aegis (Hook 10 apps, Módulos a medida, Privacidad cifrada y CTA). Generá recursos, exportá el MP4 y mostrame la tarjeta para descargarlo.";
        let scenes = split_text_to_scenes("Promo Aegis", None, prompt, None, None);
        assert_eq!(scenes.len(), 4, "Expected 4 scenes, got: {:?}", scenes);
        let joined: String = scenes
            .iter()
            .map(|s| s.narration_text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains("creame un proyecto"));
        assert!(!joined.contains("exportá el mp4"));
        assert!(!joined.contains("video promocional vertical"));
        assert!(scenes[0]
            .narration_text
            .to_lowercase()
            .contains("diez apps"));
        assert!(scenes[1].narration_text.to_lowercase().contains("módulos"));
        assert!(scenes[2].narration_text.to_lowercase().contains("cifrado"));
        assert!(scenes[3].narration_text.to_lowercase().contains("aegis"));
    }

    #[test]
    fn test_split_text_to_scenes_numbered_4_scenes() {
        let prompt = "Creame un proyecto nuevo vertical 9:16 con preset gemini_studio, voz Charon y 4 escenas promocionando Aegis: 1) Dejá de usar 10 apps dispersas, 2) Activá solo tus módulos de código, video o negocio, 3) Enclave cifrado local para tu privacidad, y 4) Probá Aegis OS hoy. Generá todo, exportalo a MP4 y pasame el reproductor.";
        let scenes = split_text_to_scenes("Aegis OS", None, prompt, None, None);
        assert_eq!(scenes.len(), 4, "Expected 4 scenes, got: {:?}", scenes);
        assert!(scenes[0].narration_text.contains("10 apps dispersas"));
        assert!(scenes[1].narration_text.contains("módulos de código"));
        assert!(scenes[2]
            .narration_text
            .to_lowercase()
            .contains("enclave cifrado"));
        assert!(scenes[3].narration_text.contains("Aegis OS"));
    }
}
