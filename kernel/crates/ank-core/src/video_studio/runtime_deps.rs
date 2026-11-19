use super::types::{
    ApiPlanRecommendation, HardwareSpecs, LocalModelRecommendation, ProgressCallback,
    ProviderReadiness, RuntimeDependencyStatus, VideoCapabilityAdvisor, VideoProgressEvent,
};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

const MAX_FFMPEG_ARCHIVE_BYTES: u64 = 320 * 1024 * 1024;

#[cfg(target_os = "windows")]
const FFMPEG_STATIC_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const FFMPEG_STATIC_URL: &str =
    "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const FFMPEG_STATIC_URL: &str =
    "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz";
#[cfg(not(any(
    target_os = "windows",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64")
)))]
const FFMPEG_STATIC_URL: &str = "";

static INSTALL_LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
static ACTIVE_INSTALL_STATUS: OnceLock<Arc<tokio::sync::RwLock<Option<VideoProgressEvent>>>> =
    OnceLock::new();

fn install_lock() -> Arc<Mutex<()>> {
    INSTALL_LOCK
        .get_or_init(|| Arc::new(Mutex::new(())))
        .clone()
}

fn active_install_cell() -> Arc<tokio::sync::RwLock<Option<VideoProgressEvent>>> {
    ACTIVE_INSTALL_STATUS
        .get_or_init(|| Arc::new(tokio::sync::RwLock::new(None)))
        .clone()
}

pub async fn current_install_progress() -> Option<VideoProgressEvent> {
    active_install_cell().read().await.clone()
}

pub fn runtime_root(data_dir: &Path) -> PathBuf {
    data_dir.join("video_studio").join("runtime")
}

pub fn scripts_dir(data_dir: &Path) -> PathBuf {
    runtime_root(data_dir).join("scripts")
}

fn managed_ffmpeg_bin_dir(data_dir: &Path) -> PathBuf {
    runtime_root(data_dir).join("ffmpeg").join("bin")
}

fn managed_venv_dir(data_dir: &Path) -> PathBuf {
    runtime_root(data_dir).join("venv")
}

pub fn managed_ffmpeg_exe(data_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        managed_ffmpeg_bin_dir(data_dir).join("ffmpeg.exe")
    } else {
        managed_ffmpeg_bin_dir(data_dir).join("ffmpeg")
    }
}

pub fn managed_ffprobe_exe(data_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        managed_ffmpeg_bin_dir(data_dir).join("ffprobe.exe")
    } else {
        managed_ffmpeg_bin_dir(data_dir).join("ffprobe")
    }
}

pub fn managed_venv_python(data_dir: &Path) -> PathBuf {
    let venv = managed_venv_dir(data_dir);
    if cfg!(target_os = "windows") {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

async fn check_exe_runs(exe: &str, flag: &str) -> bool {
    let mut cmd = Command::new(exe);
    cmd.arg(flag);
    match tokio::time::timeout(std::time::Duration::from_secs(6), cmd.output()).await {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}

pub async fn resolve_ffmpeg_path(data_dir: &Path) -> Option<(String, String)> {
    let local = managed_ffmpeg_exe(data_dir);
    if local.exists() {
        let s = local.to_string_lossy().to_string();
        if check_exe_runs(&s, "-version").await {
            return Some((s, "Aegis Runtime".to_string()));
        }
    }
    if check_exe_runs("ffmpeg", "-version").await {
        return Some(("ffmpeg".to_string(), "Sistema (PATH)".to_string()));
    }
    None
}

pub async fn resolve_ffprobe_path(data_dir: &Path) -> Option<String> {
    let local = managed_ffprobe_exe(data_dir);
    if local.exists() {
        let s = local.to_string_lossy().to_string();
        if check_exe_runs(&s, "-version").await {
            return Some(s);
        }
    }
    if check_exe_runs("ffprobe", "-version").await {
        return Some("ffprobe".to_string());
    }
    None
}

async fn find_system_python() -> Option<String> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "windows") {
        &[("python", &["--version"]), ("py", &["-3", "--version"])]
    } else {
        &[("python3", &["--version"]), ("python", &["--version"])]
    };

    for (cmd, args) in candidates {
        let mut c = Command::new(cmd);
        c.args(*args);
        if let Ok(Ok(out)) =
            tokio::time::timeout(std::time::Duration::from_secs(6), c.output()).await
        {
            if out.status.success() {
                if *cmd == "py" {
                    return Some("py -3".to_string());
                }
                return Some((*cmd).to_string());
            }
        }
    }
    None
}

async fn python_has_module(python_exe: &str, module_name: &str) -> bool {
    let mut cmd = if python_exe == "py -3" {
        let mut c = Command::new("py");
        c.arg("-3");
        c
    } else {
        Command::new(python_exe)
    };
    cmd.arg("-c").arg(format!("import {}", module_name));
    match tokio::time::timeout(std::time::Duration::from_secs(8), cmd.output()).await {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}

pub async fn resolve_python_for_video(data_dir: &Path) -> Option<(String, String)> {
    let venv_py = managed_venv_python(data_dir);
    if venv_py.exists() {
        let s = venv_py.to_string_lossy().to_string();
        if check_exe_runs(&s, "--version").await {
            return Some((s, "Aegis Venv".to_string()));
        }
    }
    find_system_python()
        .await
        .map(|s| (s, "Sistema".to_string()))
}

pub async fn inspect_runtime_status(data_dir: &Path) -> RuntimeDependencyStatus {
    let ffmpeg = resolve_ffmpeg_path(data_dir).await;
    let ffprobe = resolve_ffprobe_path(data_dir).await;
    let python = resolve_python_for_video(data_dir).await;

    let (edge_tts_ready, pillow_ready) = if let Some((ref py, _)) = python {
        let e = python_has_module(py, "edge_tts").await;
        let p = python_has_module(py, "PIL").await;
        (e, p)
    } else {
        (false, false)
    };

    let ffmpeg_ready = ffmpeg.is_some();
    let ffprobe_ready = ffprobe.is_some();
    let python_ready = python.is_some();
    let ready_for_free_video =
        ffmpeg_ready && ffprobe_ready && python_ready && edge_tts_ready && pillow_ready;

    RuntimeDependencyStatus {
        ffmpeg_ready,
        ffprobe_ready,
        python_ready,
        edge_tts_ready,
        pillow_ready,
        ready_for_free_video,
        ffmpeg_source: ffmpeg.map(|(_, src)| src),
        python_source: python.map(|(_, src)| src),
        runtime_dir: runtime_root(data_dir).to_string_lossy().to_string(),
    }
}

static HW_SPECS_CACHE: OnceLock<RwLock<Option<(Instant, HardwareSpecs)>>> = OnceLock::new();

fn hw_cache() -> &'static RwLock<Option<(Instant, HardwareSpecs)>> {
    HW_SPECS_CACHE.get_or_init(|| RwLock::new(None))
}

async fn probe_gpu_name_and_vram() -> (String, u64, bool) {
    let mut nvidia_cmd = Command::new("nvidia-smi");
    nvidia_cmd.args([
        "--query-gpu=name,memory.total",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(target_os = "windows")]
    nvidia_cmd.creation_flags(0x0800_0000);

    if let Ok(Ok(out)) =
        tokio::time::timeout(Duration::from_millis(1500), nvidia_cmd.output()).await
    {
        if out.status.success() {
            let txt = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = txt.lines().find(|l| !l.trim().is_empty()) {
                let mut parts = first_line.splitn(2, ',');
                let name = parts.next().unwrap_or("NVIDIA GPU").trim().to_string();
                let vram_mb = parts
                    .next()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                if !name.is_empty() {
                    return (name, vram_mb, true);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let mut ps_cmd = Command::new("powershell");
        ps_cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_VideoController | Select-Object -First 1 Name).Name",
        ]);
        ps_cmd.creation_flags(0x0800_0000);
        if let Ok(Ok(out)) =
            tokio::time::timeout(Duration::from_millis(1500), ps_cmd.output()).await
        {
            if out.status.success() {
                let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !name.is_empty() {
                    return (name, 0, false);
                }
            }
        }
    }

    (
        "Gráficos integrados / Sin GPU NVIDIA dedicada".to_string(),
        0,
        false,
    )
}

/// Inspects host CPU, RAM, disk space, and NVIDIA GPU/VRAM (cached 60s) to advise on local vs cloud AI.
pub async fn probe_hardware_specs(data_dir: &Path) -> HardwareSpecs {
    {
        let guard = hw_cache().read().await;
        if let Some((ts, ref cached)) = *guard {
            if ts.elapsed() < Duration::from_secs(60) {
                return cached.clone();
            }
        }
    }

    let data_dir_buf = data_dir.to_path_buf();
    let (cpu_brand, cpu_cores, total_ram_gb, available_ram_gb, free_disk_gb) =
        tokio::task::spawn_blocking(move || {
            use sysinfo::{Disks, System};
            let mut sys = System::new();
            sys.refresh_cpu_list(sysinfo::CpuRefreshKind::everything());
            sys.refresh_memory();

            let brand = sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "CPU genérica".to_string());
            let cores = sys.cpus().len().max(1);
            let total_gb = (sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0)) as f32;
            let avail_gb = (sys.available_memory() as f64 / (1024.0 * 1024.0 * 1024.0)) as f32;

            let disks = Disks::new_with_refreshed_list();
            let target_str = data_dir_buf.to_string_lossy().to_ascii_lowercase();
            let mut best_free_bytes = 0u64;
            let mut best_mount_len = 0usize;
            for d in disks.list() {
                let mp = d.mount_point().to_string_lossy().to_ascii_lowercase();
                if !mp.is_empty() && target_str.starts_with(&mp) && mp.len() >= best_mount_len {
                    best_mount_len = mp.len();
                    best_free_bytes = d.available_space();
                } else if best_mount_len == 0 && d.available_space() > best_free_bytes {
                    best_free_bytes = d.available_space();
                }
            }
            let disk_gb = (best_free_bytes as f64 / (1024.0 * 1024.0 * 1024.0)) as f32;
            (brand, cores, total_gb, avail_gb, disk_gb)
        })
        .await
        .unwrap_or_else(|_| ("CPU".to_string(), 2, 8.0, 2.0, 20.0));

    let (gpu_name, dedicated_vram_mb, has_nvidia_cuda) = probe_gpu_name_and_vram().await;

    let specs = HardwareSpecs {
        cpu_brand,
        cpu_cores,
        total_ram_gb: (total_ram_gb * 10.0).round() / 10.0,
        available_ram_gb: (available_ram_gb * 10.0).round() / 10.0,
        free_disk_gb: (free_disk_gb * 10.0).round() / 10.0,
        gpu_name,
        dedicated_vram_mb,
        has_nvidia_cuda,
    };

    let mut guard = hw_cache().write().await;
    *guard = Some((Instant::now(), specs.clone()));
    specs
}

/// Builds actionable local-model and cloud-API recommendations tailored to this PC and configured keys.
pub async fn build_capability_advisor(
    data_dir: &Path,
    readiness: &ProviderReadiness,
) -> VideoCapabilityAdvisor {
    let pc = probe_hardware_specs(data_dir).await;
    let vram_gb = pc.dedicated_vram_mb as f32 / 1024.0;

    let pc_verdict = if pc.has_nvidia_cuda && pc.dedicated_vram_mb >= 8000 {
        format!(
            "Tu PC ({} núcleos, {:.1} GB RAM, {} con {:.1} GB VRAM CUDA) puede correr generación local de voz e imágenes IA (SDXL/Flux) y renderizado FFmpeg fluido.",
            pc.cpu_cores, pc.total_ram_gb, pc.gpu_name, vram_gb
        )
    } else if pc.has_nvidia_cuda && pc.dedicated_vram_mb >= 4000 {
        format!(
            "Tu PC ({} núcleos, {:.1} GB RAM, {} con {:.1} GB VRAM) soporta voz neural local (XTTS-v2) y Motion 2.0 FFmpeg. Para imágenes/video IA pesados conviene usar API en la nube.",
            pc.cpu_cores, pc.total_ram_gb, pc.gpu_name, vram_gb
        )
    } else {
        format!(
            "Tu PC actual ({} núcleos, {:.1} GB RAM total / {:.1} GB libre, {}) no tiene GPU NVIDIA dedicada con 8+ GB VRAM: corre sin problemas Edge TTS HD y Motion Compositor 2.0 por CPU, pero difusión local (SDXL/Flux/Wan 2.1) requiere GPU CUDA o API cloud.",
            pc.cpu_cores, pc.total_ram_gb, pc.available_ram_gb, pc.gpu_name
        )
    };

    let kokoro_ok = pc.total_ram_gb >= 4.0;
    let xtts_ok = pc.has_nvidia_cuda && pc.dedicated_vram_mb >= 4000 && pc.total_ram_gb >= 12.0;
    let flux_ok = pc.has_nvidia_cuda && pc.dedicated_vram_mb >= 8000 && pc.total_ram_gb >= 16.0;
    let wan_ok = pc.has_nvidia_cuda && pc.dedicated_vram_mb >= 8000 && pc.total_ram_gb >= 16.0;

    let local_models = vec![
        LocalModelRecommendation {
            category: "Voz (TTS Local)".to_string(),
            model_name: "Kokoro-82M / Piper TTS (CPU)".to_string(),
            min_ram_gb: 2,
            min_vram_gb: 0,
            compatible_with_this_pc: kokoro_ok,
            status_note: if kokoro_ok {
                "Compatible: corre rápido en CPU sin GPU dedicada.".to_string()
            } else {
                "RAM muy justa (<4 GB).".to_string()
            },
        },
        LocalModelRecommendation {
            category: "Voz (TTS Local)".to_string(),
            model_name: "XTTS-v2 / F5-TTS Clonación Vocal".to_string(),
            min_ram_gb: 12,
            min_vram_gb: 4,
            compatible_with_this_pc: xtts_ok,
            status_note: if xtts_ok {
                "Compatible con tu GPU CUDA.".to_string()
            } else {
                format!(
                    "Requiere GPU NVIDIA 4+ GB VRAM y 12+ GB RAM (tenés {:.1} GB RAM y {}).",
                    pc.total_ram_gb, pc.gpu_name
                )
            },
        },
        LocalModelRecommendation {
            category: "Imágenes (Fondos IA)".to_string(),
            model_name: "SDXL-Lightning / Flux.1 Schnell (ComfyUI)".to_string(),
            min_ram_gb: 16,
            min_vram_gb: 8,
            compatible_with_this_pc: flux_ok,
            status_note: if flux_ok {
                "Compatible con tu GPU CUDA de 8+ GB VRAM.".to_string()
            } else {
                format!(
                    "No viable offline en esta PC (pide >=8 GB VRAM NVIDIA y 16 GB RAM; detectado {:.1} GB VRAM).",
                    vram_gb
                )
            },
        },
        LocalModelRecommendation {
            category: "Video Generativo (B-Roll)".to_string(),
            model_name: "Wan 2.1 1.3B / 14B (ComfyUI)".to_string(),
            min_ram_gb: 16,
            min_vram_gb: 8,
            compatible_with_this_pc: wan_ok,
            status_note: if wan_ok {
                "Compatible en modo 1.3B con tu GPU NVIDIA.".to_string()
            } else {
                "Requiere GPU NVIDIA 8-16+ GB VRAM; usá Pexels gratis o APIs cloud.".to_string()
            },
        },
    ];

    let api_plans = vec![
        ApiPlanRecommendation {
            provider: "Pollinations AI (Flux · Sin API Key)".to_string(),
            current_status: "Activo por defecto (100% gratis, sin registro)".to_string(),
            recommended_plan: "Incluido en Aegis (0 USD · Sin API Key)".to_string(),
            what_it_unlocks: "Fondos verticales 9:16 generados por IA al vuelo en cada escena + HUD animado sin usar GPU local."
                .to_string(),
            estimated_cost: "Gratis (0 USD)".to_string(),
        },
        ApiPlanRecommendation {
            provider: "Pixabay Stock API (Videos MP4 y Fotos HD)".to_string(),
            current_status: if readiness.has_pexels_key {
                "Configurada y lista (100% gratis)".to_string()
            } else {
                "Opcional sin configurar (pixabay.com/api/docs)".to_string()
            },
            recommended_plan: "Cuenta Gratuita Pixabay API (reemplaza a Pexels que pausó claves nuevas)".to_string(),
            what_it_unlocks: "Clips de video MP4 reales HD y fotos libres de derechos para B-roll sin usar GPU."
                .to_string(),
            estimated_cost: "Gratis (0 USD)".to_string(),
        },
        ApiPlanRecommendation {
            provider: "Google Gemini API (Google AI Studio)".to_string(),
            current_status: if readiness.has_gemini_key {
                "API Key cargada (en Free Tier puede limitar Imagen/Veo o TPM)".to_string()
            } else {
                "Sin API Key configurada".to_string()
            },
            recommended_plan: "Activar Pay-As-You-Go (Tier 1) en Google AI Studio".to_string(),
            what_it_unlocks: "Voz WAV Studio (gemini-2.5-flash-preview-tts Charon/Aoede), fondos IA 9:16 (gemini-2.5-flash-image), Google Veo y 1M+ TPM sin bloqueos."
                .to_string(),
            estimated_cost: "~USD 0.02 a 0.15 por reel (~$0.04 con Imagen sin Veo)".to_string(),
        },
        ApiPlanRecommendation {
            provider: "OpenAI API / ElevenLabs".to_string(),
            current_status: if readiness.has_openai_key || readiness.has_elevenlabs_key {
                "Parcialmente configurada".to_string()
            } else {
                "Opcional (sin configurar)".to_string()
            },
            recommended_plan: "OpenAI Tier 1 (recarga mínima USD 5) o ElevenLabs Starter"
                .to_string(),
            what_it_unlocks: "Voces tts-1-hd / eleven_multilingual_v2 hiperrealistas + gpt-image-1 + Whisper."
                .to_string(),
            estimated_cost: "USD 5 recarga prepaga / ~USD 5 mes".to_string(),
        },
    ];

    VideoCapabilityAdvisor {
        pc_specs: pc,
        pc_verdict,
        local_models,
        api_plans,
    }
}

const SCRIPT_TTS_EDGE: &str = r#"import asyncio
import json
import os
import sys
import edge_tts

async def main():
    if len(sys.argv) < 3 or sys.argv[1] != "--spec":
        raise SystemExit("Usage: tts_edge.py --spec <spec.json>")
    with open(sys.argv[2], "r", encoding="utf-8") as f:
        spec = json.load(f)
    text = (spec.get("text") or "").strip()
    voice = (spec.get("voice") or "es-AR-TomasNeural").strip()
    rate = (spec.get("rate") or "+2%").strip()
    pitch = (spec.get("pitch") or "+0Hz").strip()
    out_path = spec["out"]
    words_out = spec.get("words_out")
    if not text:
        raise SystemExit("Empty narration text")

    words = []
    audio_bytes = bytearray()

    try:
        communicate = edge_tts.Communicate(
            text, voice, rate=rate, pitch=pitch, boundary="WordBoundary"
        )
        async for chunk in communicate.stream():
            ctype = chunk.get("type")
            if ctype == "audio":
                data = chunk.get("data") or b""
                audio_bytes.extend(data)
            elif ctype == "WordBoundary":
                w_text = (chunk.get("text") or "").strip()
                off = float(chunk.get("offset") or 0) / 10_000_000.0
                dur = float(chunk.get("duration") or 0) / 10_000_000.0
                if w_text:
                    words.append({
                        "start": round(off, 3),
                        "end": round(max(off + 0.12, off + dur), 3),
                        "text": w_text,
                    })
    except Exception:
        audio_bytes.clear()
        words.clear()

    if not audio_bytes:
        communicate = edge_tts.Communicate(text, voice, rate=rate, pitch=pitch)
        await communicate.save(out_path)
    else:
        with open(out_path, "wb") as out_f:
            out_f.write(bytes(audio_bytes))

    if words_out:
        with open(words_out, "w", encoding="utf-8") as wf:
            json.dump(words, wf, ensure_ascii=False)

if __name__ == "__main__":
    asyncio.run(main())
"#;

const SCRIPT_RENDER_CARD: &str = r###"import json
import os
import sys
import textwrap
from PIL import Image, ImageDraw, ImageFilter, ImageFont

def load_font(size: int, bold: bool = False, mono: bool = False):
    candidates = []
    if os.name == "nt":
        windir = os.environ.get("WINDIR", "C:\\Windows")
        fonts = os.path.join(windir, "Fonts")
        if mono:
            candidates += [
                os.path.join(fonts, "consolab.ttf" if bold else "consola.ttf"),
                os.path.join(fonts, "lucon.ttf"),
            ]
        elif bold:
            candidates += [
                os.path.join(fonts, "segoeuib.ttf"),
                os.path.join(fonts, "arialbd.ttf"),
                os.path.join(fonts, "calibrib.ttf"),
            ]
        else:
            candidates += [
                os.path.join(fonts, "segoeui.ttf"),
                os.path.join(fonts, "arial.ttf"),
                os.path.join(fonts, "calibri.ttf"),
            ]
    else:
        if mono:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf" if bold else "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ]
        elif bold:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            ]
        else:
            candidates += [
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ]
    for path in candidates:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size)
            except Exception:
                pass
    return ImageFont.load_default()

def detect_theme(spec):
    blob = " ".join([
        str(spec.get("title") or ""),
        str(spec.get("text") or ""),
        str(spec.get("visual_prompt") or ""),
    ]).lower()
    idx = int(spec.get("scene_idx") or 0)
    total = max(1, int(spec.get("total_scenes") or 1))

    if any(k in blob for k in ("enclave", "cifrad", "privac", "segur", "aes", "sqlcipher", "shield", "candado")):
        return "privacy", (16, 185, 129), (6, 182, 212)
    if any(k in blob for k in ("módulo", "modulo", "program", "creador", "negocio", "stock", "bento", "herramienta")):
        return "modules", (6, 182, 212), (99, 102, 241)
    if any(k in blob for k in ("app", "dispers", "caos", "basta", "problema", "hook", "dejá", "deja")):
        return "chaos", (244, 63, 94), (245, 158, 11)
    if any(k in blob for k in ("cta", "probá", "proba", "descarg", "cierre", "empez", "unite")) or (total >= 2 and idx == total - 1):
        return "cta", (168, 85, 247), (34, 211, 238)
    palettes = [
        ("chaos", (236, 72, 153), (99, 102, 241)),
        ("modules", (6, 182, 212), (99, 102, 241)),
        ("privacy", (16, 185, 129), (34, 211, 238)),
        ("cta", (168, 85, 247), (236, 72, 153)),
    ]
    return palettes[idx % len(palettes)]

def wrap_fit(draw, text: str, max_width: int, start_size: int, min_size: int, max_lines: int):
    clean = " ".join((text or "").split()).rstrip(".")
    for sz in range(start_size, min_size - 1, -4):
        fnt = load_font(sz, bold=True)
        # Estimate chars per line using average uppercase/lowercase char width
        bbox = draw.textbbox((0, 0), "AgostoMódulo", font=fnt)
        avg_w = max(8.0, (bbox[2] - bbox[0]) / 12.0)
        est_chars = max(10, int(max_width / avg_w))
        lines = textwrap.wrap(clean, width=est_chars, break_long_words=False)
        if len(lines) <= max_lines:
            widest = max((draw.textbbox((0, 0), ln, font=fnt)[2] for ln in lines), default=0)
            if widest <= max_width:
                return fnt, lines, sz
    fnt = load_font(min_size, bold=True)
    est_chars = max(12, int(max_width / max(8.0, min_size * 0.56)))
    lines = textwrap.wrap(clean, width=est_chars, break_long_words=True)[:max_lines]
    return fnt, lines, min_size

def draw_glow_orb(base_img: Image.Image, cx: int, cy: int, radius: int, rgb: tuple, alpha: int = 95):
    layer = Image.new("RGBA", base_img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    d.ellipse([cx - radius, cy - radius, cx + radius, cy + radius], fill=(rgb[0], rgb[1], rgb[2], alpha))
    layer = layer.filter(ImageFilter.GaussianBlur(radius=max(16, int(radius * 0.45))))
    return Image.alpha_composite(base_img, layer)

def draw_hero_graphic(canvas: Image.Image, theme: str, w: int, h: int, c1: tuple, c2: tuple):
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    sc = w / 1080.0
    cy_mid = int(h * 0.57)
    cx_mid = w // 2
    box_w = int(w * 0.84)
    left = (w - box_w) // 2
    right = left + box_w
    top = int(h * 0.45)
    bot = int(h * 0.71)

    if theme == "chaos":
        chips = [
            ("Chat Suelto", left + int(24 * sc), top + int(20 * sc), (244, 63, 94)),
            ("Planilla Excel", right - int(320 * sc), top + int(32 * sc), (245, 158, 11)),
            ("App de Notas", left + int(40 * sc), bot - int(155 * sc), (148, 163, 184)),
            ("Editor Aparte", right - int(310 * sc), bot - int(145 * sc), (236, 72, 153)),
        ]
        f_chip = load_font(max(18, int(24 * sc)), bold=True)
        for lbl, x, y, col in chips:
            cw, ch = int(285 * sc), int(84 * sc)
            d.rounded_rectangle([x, y, x + cw, y + ch], radius=18, fill=(22, 26, 40, 225), outline=(col[0], col[1], col[2], 150), width=2)
            d.text((x + int(24 * sc), y + int(24 * sc)), lbl, font=f_chip, fill=(203, 213, 225, 220))
            d.line([(x + 18, y + ch // 2 + 2), (x + cw - 18, y + ch // 2 + 2)], fill=(244, 63, 94, 195), width=3)
        # Central converging Aegis ring
        r = int(96 * sc)
        d.ellipse([cx_mid - r, cy_mid - r, cx_mid + r, cy_mid + r], fill=(12, 18, 34, 245), outline=(34, 211, 238, 255), width=5)
        f_core = load_font(max(24, int(34 * sc)), bold=True)
        tw = d.textbbox((0, 0), "AEGIS", font=f_core)[2]
        d.text((cx_mid - tw // 2, cy_mid - int(20 * sc)), "AEGIS", font=f_core, fill=(255, 255, 255, 255))

    elif theme == "modules":
        cards = [
            ("DEV STUDIO", "Agentes & Código", (6, 182, 212), 0.78),
            ("CREATOR + VIDEO", "Guiones & MP4 IA", (168, 85, 247), 0.92),
            ("NEGOCIO POS", "Stock & Ventas", (16, 185, 129), 0.68),
        ]
        f_h = load_font(max(20, int(28 * sc)), bold=True)
        f_s = load_font(max(16, int(22 * sc)), bold=False)
        card_h = int(112 * sc)
        gap = int(24 * sc)
        start_y = top + int(24 * sc)
        for i, (t1, t2, col, pct) in enumerate(cards):
            y0 = start_y + i * (card_h + gap)
            d.rounded_rectangle([left, y0, right, y0 + card_h], radius=24, fill=(17, 24, 39, 230), outline=(col[0], col[1], col[2], 190), width=3)
            d.ellipse([left + int(28 * sc), y0 + int(38 * sc), left + int(56 * sc), y0 + int(66 * sc)], fill=(col[0], col[1], col[2], 255))
            d.text((left + int(78 * sc), y0 + int(20 * sc)), t1, font=f_h, fill=(248, 250, 252, 255))
            d.text((left + int(78 * sc), y0 + int(60 * sc)), t2, font=f_s, fill=(148, 163, 184, 245))
            bar_w = int(170 * sc)
            bx = right - bar_w - int(28 * sc)
            by = y0 + int(48 * sc)
            d.rounded_rectangle([bx, by, bx + bar_w, by + int(14 * sc)], radius=7, fill=(30, 41, 59, 255))
            d.rounded_rectangle([bx, by, bx + int(bar_w * pct), by + int(14 * sc)], radius=7, fill=(col[0], col[1], col[2], 255))

    elif theme == "privacy":
        for mul, al in [(1.45, 75), (1.12, 130), (0.82, 210)]:
            rr = int(115 * sc * mul)
            d.ellipse([cx_mid - rr, cy_mid - rr - int(18 * sc), cx_mid + rr, cy_mid + rr - int(18 * sc)], outline=(c1[0], c1[1], c1[2], al), width=3)
        sw = int(118 * sc)
        sh = int(138 * sc)
        yc = cy_mid - int(22 * sc)
        pts = [
            (cx_mid, yc - sh // 2),
            (cx_mid + sw // 2, yc - int(sh * 0.28)),
            (cx_mid + int(sw * 0.42), yc + int(sh * 0.25)),
            (cx_mid, yc + sh // 2),
            (cx_mid - int(sw * 0.42), yc + int(sh * 0.25)),
            (cx_mid - sw // 2, yc - int(sh * 0.28)),
        ]
        d.polygon(pts, fill=(12, 42, 42, 235), outline=(16, 185, 129, 255), width=5)
        f_tag = load_font(max(18, int(24 * sc)), bold=True, mono=True)
        tag = "AES-256  ·  SQLCIPHER  ·  ENCLAVE LOCAL"
        tw = d.textbbox((0, 0), tag, font=f_tag)[2]
        py = bot - int(72 * sc)
        d.rounded_rectangle([cx_mid - tw // 2 - 28, py, cx_mid + tw // 2 + 28, py + int(50 * sc)], radius=25, fill=(10, 30, 32, 230), outline=(16, 185, 129, 190), width=2)
        d.text((cx_mid - tw // 2, py + int(11 * sc)), tag, font=f_tag, fill=(110, 231, 183, 255))

    elif theme == "cta":
        card_top = top + int(20 * sc)
        d.rounded_rectangle([left, card_top, right, bot - int(20 * sc)], radius=32, fill=(18, 20, 38, 235), outline=(c1[0], c1[1], c1[2], 210), width=3)
        f_sub = load_font(max(22, int(30 * sc)), bold=True)
        f_btn = load_font(max(24, int(34 * sc)), bold=True)
        s1 = "Tu sistema IA modular y privado"
        w1 = d.textbbox((0, 0), s1, font=f_sub)[2]
        d.text((cx_mid - w1 // 2, card_top + int(56 * sc)), s1, font=f_sub, fill=(226, 232, 240, 255))
        btn_txt = "DESCUBRÍ AEGIS OS"
        bw = max(int(520 * sc), d.textbbox((0, 0), btn_txt, font=f_btn)[2] + 90)
        bx0 = cx_mid - bw // 2
        by0 = card_top + int(135 * sc)
        bh = int(88 * sc)
        d.rounded_rectangle([bx0, by0, bx0 + bw, by0 + bh], radius=44, fill=(34, 211, 238, 255))
        btw = d.textbbox((0, 0), btn_txt, font=f_btn)[2]
        d.text((cx_mid - btw // 2, by0 + int(22 * sc)), btn_txt, font=f_btn, fill=(8, 12, 22, 255))

    else:
        d.rounded_rectangle([left, top + 20, right, bot - 20], radius=28, fill=(15, 23, 42, 225), outline=(c1[0], c1[1], c1[2], 185), width=3)
        bars = [0.35, 0.72, 0.55, 0.92, 0.64, 0.84, 0.48, 0.76, 0.90, 0.58]
        bw = int((box_w - 140) / (len(bars) * 1.45))
        gap = int(bw * 0.45)
        x = left + 70
        base_y = bot - int(65 * sc)
        max_bh = int(175 * sc)
        for i, ratio in enumerate(bars):
            bh = int(max_bh * ratio)
            col = c1 if i % 2 == 0 else c2
            d.rounded_rectangle([x, base_y - bh, x + bw, base_y], radius=8, fill=(col[0], col[1], col[2], 225))
            x += bw + gap

    return Image.alpha_composite(canvas, layer)

def main():
    if len(sys.argv) < 3 or sys.argv[1] != "--spec":
        raise SystemExit("Usage: render_card.py --spec <spec.json>")
    with open(sys.argv[2], "r", encoding="utf-8") as f:
        spec = json.load(f)

    w = int(spec.get("width", 1080))
    h = int(spec.get("height", 1920))
    mode = (spec.get("mode") or "full").strip().lower()
    title = (spec.get("title") or "").strip()
    headline = (spec.get("text") or "").strip() or title or "AEGIS STUDIO"
    badge = (spec.get("badge") or "AEGIS STUDIO").strip()
    idx = int(spec.get("scene_idx") or 0)
    total = max(1, int(spec.get("total_scenes") or 1))
    out_path = spec["out"]
    sc = w / 1080.0

    theme, c1, c2 = detect_theme(spec)

    if mode == "overlay":
        img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)
        # Subtle top and bottom scrims for crisp readability over AI/stock visuals
        top_h = int(h * 0.29)
        for y in range(top_h):
            al = int(185 * (1.0 - y / max(1, top_h)))
            d.line([(0, y), (w, y)], fill=(6, 9, 18, al))
        bot_start = int(h * 0.65)
        for y in range(bot_start, h):
            t = (y - bot_start) / max(1, h - bot_start)
            al = int(215 * (t ** 1.1))
            d.line([(0, y), (w, y)], fill=(6, 9, 18, al))

        pad = int(w * 0.075)
        f_badge = load_font(max(18, int(24 * sc)), bold=True, mono=True)
        pill_txt = f"{badge.upper()}"
        pb = d.textbbox((0, 0), pill_txt, font=f_badge)
        pw = (pb[2] - pb[0]) + int(42 * sc)
        ph = int(46 * sc)
        py = int(h * 0.055)
        d.rounded_rectangle([pad, py, pad + pw, py + ph], radius=ph // 2, fill=(10, 15, 28, 215), outline=(c1[0], c1[1], c1[2], 235), width=2)
        d.text((pad + int(21 * sc), py + int(10 * sc)), pill_txt, font=f_badge, fill=(c1[0], c1[1], c1[2], 255))

        f_head, lines, sz = wrap_fit(d, headline, w - pad * 2, max(36, int(58 * sc)), max(26, int(36 * sc)), 3)
        y_cur = py + ph + int(26 * sc)
        for ln in lines:
            d.text((pad + 3, y_cur + 3), ln, font=f_head, fill=(0, 0, 0, 210))
            d.text((pad, y_cur), ln, font=f_head, fill=(255, 255, 255, 255))
            y_cur += int(sz * 1.24)
        img.save(out_path, "PNG")
        return

    # Full Motion Graphics 2.0 Canvas
    img = Image.new("RGBA", (w, h), (7, 10, 19, 255))
    bg_draw = ImageDraw.Draw(img)
    for y in range(h):
        t = y / max(1, h - 1)
        r = int(8 + 10 * (1 - t) + c2[0] * 0.07 * t)
        g = int(11 + 8 * (1 - t) + c1[1] * 0.06 * (1 - t))
        b = int(24 + 20 * t)
        bg_draw.line([(0, y), (w, y)], fill=(min(255, r), min(255, g), min(255, b), 255))

    # Soft blurred neon light orbs
    img = draw_glow_orb(img, int(w * 0.18), int(h * 0.18), int(w * 0.38), c1, 88)
    img = draw_glow_orb(img, int(w * 0.85), int(h * 0.56), int(w * 0.42), c2, 76)

    # Subtle blueprint dot matrix background
    grid = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gd = ImageDraw.Draw(grid)
    step = max(36, int(56 * sc))
    for gx in range(step // 2, w, step):
        for gy in range(step // 2, h, step):
            gd.rectangle([gx, gy, gx + 2, gy + 2], fill=(148, 163, 184, 34))
    img = Image.alpha_composite(img, grid)

    # Hero scene-specific illustration
    img = draw_hero_graphic(img, theme, w, h, c1, c2)

    # Bottom gradient scrim for live wave & subtitles
    scrim = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    sd = ImageDraw.Draw(scrim)
    bot_start = int(h * 0.71)
    for y in range(bot_start, h):
        t = (y - bot_start) / max(1, h - bot_start)
        al = int(215 * (t ** 1.15))
        sd.line([(0, y), (w, y)], fill=(5, 8, 16, al))
    img = Image.alpha_composite(img, scrim)

    d = ImageDraw.Draw(img)
    pad = int(w * 0.08)

    # Top segmented progress indicator
    seg_w = int((w - pad * 2 - max(0, total - 1) * 12) / max(1, total))
    seg_y = int(h * 0.052)
    for si in range(total):
        sx = pad + si * (seg_w + 12)
        col = (c1[0], c1[1], c1[2], 245) if si <= idx else (255, 255, 255, 45)
        d.rounded_rectangle([sx, seg_y, sx + seg_w, seg_y + int(7 * sc)], radius=4, fill=col)

    # Neon pill badge
    f_badge = load_font(max(18, int(24 * sc)), bold=True, mono=True)
    pill_txt = badge.upper()
    pb = d.textbbox((0, 0), pill_txt, font=f_badge)
    pw = (pb[2] - pb[0]) + int(42 * sc)
    ph = int(48 * sc)
    py = seg_y + int(28 * sc)
    d.rounded_rectangle([pad, py, pad + pw, py + ph], radius=ph // 2, fill=(14, 21, 38, 235), outline=(c1[0], c1[1], c1[2], 235), width=2)
    d.text((pad + int(21 * sc), py + int(11 * sc)), pill_txt, font=f_badge, fill=(c1[0], c1[1], c1[2], 255))

    # Optional crisp kicker title above hero headline when distinct
    y_cur = py + ph + int(34 * sc)
    if title and title.lower() not in headline.lower() and len(title) <= 32:
        f_kicker = load_font(max(20, int(28 * sc)), bold=True)
        d.text((pad, y_cur), title.upper(), font=f_kicker, fill=(148, 163, 184, 255))
        y_cur += int(42 * sc)

    # Big non-truncated editorial headline
    f_head, lines, sz = wrap_fit(d, headline, w - pad * 2, max(40, int(68 * sc)), max(28, int(38 * sc)), 4)
    for ln in lines:
        d.text((pad + 3, y_cur + 4), ln, font=f_head, fill=(0, 0, 0, 195))
        d.text((pad, y_cur), ln, font=f_head, fill=(255, 255, 255, 255))
        y_cur += int(sz * 1.24)

    # Accent underline bar beneath headline
    d.rounded_rectangle([pad, y_cur + int(24 * sc), pad + int(130 * sc), y_cur + int(32 * sc)], radius=4, fill=(c1[0], c1[1], c1[2], 255))

    img.convert("RGB").save(out_path, "PNG")

if __name__ == "__main__":
    main()
"###;

const SCRIPT_EXTRACT_FFMPEG: &str = r#"import os
import shutil
import sys
import tarfile
import zipfile

def safe_extract(archive_path: str, bin_dir: str):
    os.makedirs(bin_dir, exist_ok=True)
    targets = {"ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe"}
    found = []

    if archive_path.lower().endswith(".zip"):
        with zipfile.ZipFile(archive_path, "r") as zf:
            for member in zf.infolist():
                norm = member.filename.replace("\\", "/")
                if ".." in norm.split("/") or norm.startswith("/"):
                    continue
                base = os.path.basename(norm)
                if base in targets:
                    dest = os.path.join(bin_dir, base)
                    with zf.open(member, "r") as src, open(dest, "wb") as dst:
                        shutil.copyfileobj(src, dst)
                    if os.name != "nt":
                        os.chmod(dest, 0o755)
                    found.append(base)
    else:
        with tarfile.open(archive_path, "r:*") as tf:
            for member in tf.getmembers():
                if not member.isfile():
                    continue
                norm = member.name.replace("\\", "/")
                if ".." in norm.split("/") or norm.startswith("/"):
                    continue
                base = os.path.basename(norm)
                if base in targets:
                    src = tf.extractfile(member)
                    if src is not None:
                        dest = os.path.join(bin_dir, base)
                        with src, open(dest, "wb") as dst:
                            shutil.copyfileobj(src, dst)
                        if os.name != "nt":
                            os.chmod(dest, 0o755)
                        found.append(base)

    if not any(f.startswith("ffmpeg") for f in found):
        raise SystemExit(f"Archive did not contain ffmpeg binary (found: {found})")
    print("Extracted:", ",".join(sorted(set(found))))

if __name__ == "__main__":
    safe_extract(sys.argv[1], sys.argv[2])
"#;

pub async fn ensure_helper_scripts(data_dir: &Path) -> Result<PathBuf, String> {
    let dir = scripts_dir(data_dir);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create video runtime scripts dir: {}", e))?;

    for (name, body) in [
        ("tts_edge.py", SCRIPT_TTS_EDGE),
        ("render_card.py", SCRIPT_RENDER_CARD),
        ("extract_ffmpeg.py", SCRIPT_EXTRACT_FFMPEG),
    ] {
        let path = dir.join(name);
        let needs_write = match tokio::fs::read_to_string(&path).await {
            Ok(existing) => existing != body,
            Err(_) => true,
        };
        if needs_write {
            tokio::fs::write(&path, body)
                .await
                .map_err(|e| format!("Failed writing script '{}': {}", name, e))?;
        }
    }
    Ok(dir)
}

async fn emit_install_progress(
    cb: &Option<ProgressCallback>,
    progress: f32,
    message: &str,
    done: bool,
    error: Option<String>,
) {
    let evt = VideoProgressEvent {
        project_id: "_runtime".to_string(),
        stage: "install_runtime".to_string(),
        progress,
        message: message.to_string(),
        scene_id: None,
        done,
        error: error.clone(),
    };
    {
        let cell_arc = active_install_cell();
        let mut cell = cell_arc.write().await;
        if done {
            *cell = None;
        } else {
            *cell = Some(evt.clone());
        }
    }
    if let Some(callback) = cb {
        callback(evt);
    }
}

pub async fn ensure_runtime_installed(
    data_dir: &Path,
    force: bool,
    progress_cb: Option<ProgressCallback>,
) -> Result<RuntimeDependencyStatus, String> {
    let lock = install_lock();
    let _guard = lock.lock().await;

    ensure_helper_scripts(data_dir).await?;
    let current = inspect_runtime_status(data_dir).await;
    if current.ready_for_free_video && !force {
        return Ok(current);
    }

    info!("Aegis Video Studio: Preparing on-demand runtime dependencies...");
    emit_install_progress(
        &progress_cb,
        0.05,
        "Verificando Python y entorno aislado de Video Studio...",
        false,
        None,
    )
    .await;

    // 1. Ensure system Python + isolated venv with edge-tts & pillow
    let sys_py = find_system_python().await.ok_or_else(|| {
        "Python 3 no está instalado o no se encuentra en PATH. Instalá Python 3.10+ para habilitar Edge TTS y gráficos automáticos.".to_string()
    })?;

    let venv_dir = managed_venv_dir(data_dir);
    let venv_py = managed_venv_python(data_dir);

    if force || !venv_py.exists() {
        emit_install_progress(
            &progress_cb,
            0.12,
            "Creando entorno Python aislado para Video Studio...",
            false,
            None,
        )
        .await;
        tokio::fs::create_dir_all(runtime_root(data_dir))
            .await
            .map_err(|e| format!("No se pudo crear carpeta runtime: {}", e))?;

        let mut cmd = if sys_py == "py -3" {
            let mut c = Command::new("py");
            c.arg("-3");
            c
        } else {
            Command::new(&sys_py)
        };
        let out = cmd
            .arg("-m")
            .arg("venv")
            .arg(&venv_dir)
            .output()
            .await
            .map_err(|e| format!("Error creando venv: {}", e))?;
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr).to_string();
            emit_install_progress(
                &progress_cb,
                0.0,
                "Error creando entorno Python",
                true,
                Some(msg.clone()),
            )
            .await;
            return Err(format!("Fallo al crear venv Python: {}", msg));
        }
    }

    let venv_py_str = venv_py.to_string_lossy().to_string();
    let needs_pip_pkgs = force
        || !python_has_module(&venv_py_str, "edge_tts").await
        || !python_has_module(&venv_py_str, "PIL").await;

    if needs_pip_pkgs {
        emit_install_progress(
            &progress_cb,
            0.25,
            "Instalando Edge-TTS (voces gratuitas) y Pillow en entorno aislado...",
            false,
            None,
        )
        .await;

        let out = Command::new(&venv_py)
            .args([
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-input",
                "edge-tts",
                "pillow",
            ])
            .output()
            .await
            .map_err(|e| format!("Error ejecutando pip install: {}", e))?;

        if !out.status.success() {
            let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_string();
            emit_install_progress(
                &progress_cb,
                0.0,
                "Error instalando edge-tts / pillow",
                true,
                Some(err_txt.clone()),
            )
            .await;
            return Err(format!("Fallo al instalar edge-tts y pillow: {}", err_txt));
        }
    }

    // 2. Ensure FFmpeg + FFprobe
    let has_ffmpeg = resolve_ffmpeg_path(data_dir).await.is_some()
        && resolve_ffprobe_path(data_dir).await.is_some();

    if force || !has_ffmpeg {
        if FFMPEG_STATIC_URL.is_empty() {
            return Err(
                "Descarga automática de FFmpeg no soportada en esta arquitectura; instalá FFmpeg en tu PATH."
                    .to_string(),
            );
        }

        emit_install_progress(
            &progress_cb,
            0.42,
            "Descargando motor FFmpeg portable...",
            false,
            None,
        )
        .await;

        let tmp_dir = runtime_root(data_dir).join("tmp");
        tokio::fs::create_dir_all(&tmp_dir)
            .await
            .map_err(|e| format!("Failed to create runtime tmp dir: {}", e))?;

        let archive_name = if FFMPEG_STATIC_URL.ends_with(".zip") {
            "ffmpeg-static.zip"
        } else {
            "ffmpeg-static.tar.xz"
        };
        let archive_path = tmp_dir.join(archive_name);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let resp = client
            .get(FFMPEG_STATIC_URL)
            .send()
            .await
            .map_err(|e| format!("Error descargando FFmpeg: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_msg = format!("Error HTTP descargando FFmpeg ({})", status);
            emit_install_progress(&progress_cb, 0.0, &err_msg, true, Some(err_msg.clone())).await;
            return Err(err_msg);
        }

        let total_bytes = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = tokio::fs::File::create(&archive_path)
            .await
            .map_err(|e| format!("Error creando archivo temporal FFmpeg: {}", e))?;

        let mut stream = resp.bytes_stream();
        let mut last_pct_reported = 0u32;

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| format!("Error de red descargando FFmpeg: {}", e))?;
            downloaded += chunk.len() as u64;
            if downloaded > MAX_FFMPEG_ARCHIVE_BYTES {
                let _ = tokio::fs::remove_file(&archive_path).await;
                return Err("Archivo FFmpeg excede el límite de seguridad de 320 MB".to_string());
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Error escribiendo FFmpeg en disco: {}", e))?;

            if total_bytes > 0 {
                let pct = ((downloaded as f64 / total_bytes as f64) * 100.0) as u32;
                if pct >= last_pct_reported + 10 {
                    last_pct_reported = pct;
                    let mapped = 0.42 + (pct as f32 / 100.0) * 0.43;
                    emit_install_progress(
                        &progress_cb,
                        mapped,
                        &format!("Descargando FFmpeg portable... {}%", pct.min(100)),
                        false,
                        None,
                    )
                    .await;
                }
            }
        }
        let _ = file.flush().await;
        drop(file);

        emit_install_progress(
            &progress_cb,
            0.88,
            "Extrayendo binarios ffmpeg y ffprobe...",
            false,
            None,
        )
        .await;

        let sdir = ensure_helper_scripts(data_dir).await?;
        let extractor = sdir.join("extract_ffmpeg.py");
        let bin_dir = managed_ffmpeg_bin_dir(data_dir);

        let extract_out = Command::new(&venv_py)
            .arg(&extractor)
            .arg(&archive_path)
            .arg(&bin_dir)
            .output()
            .await
            .map_err(|e| format!("Error ejecutando extractor FFmpeg: {}", e))?;

        let _ = tokio::fs::remove_file(&archive_path).await;

        if !extract_out.status.success() {
            let stderr = String::from_utf8_lossy(&extract_out.stderr).to_string();
            emit_install_progress(
                &progress_cb,
                0.0,
                "Error extrayendo FFmpeg",
                true,
                Some(stderr.clone()),
            )
            .await;
            return Err(format!("Fallo al extraer FFmpeg: {}", stderr));
        }
    }

    let final_status = inspect_runtime_status(data_dir).await;
    if !final_status.ready_for_free_video {
        warn!(
            "Video Studio runtime finished install check with missing pieces: {:?}",
            final_status
        );
        emit_install_progress(
            &progress_cb,
            1.0,
            "Instalación finalizada con advertencias; revisá el estado del entorno.",
            true,
            None,
        )
        .await;
    } else {
        emit_install_progress(
            &progress_cb,
            1.0,
            "Motor de Video Studio listo (FFmpeg + Edge-TTS + Gráficos).",
            true,
            None,
        )
        .await;
    }

    Ok(final_status)
}
