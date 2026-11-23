use super::types::{
    ApiPlanRecommendation, HardwareSpecs, LocalModelRecommendation, ProgressCallback,
    ProviderReadiness, RuntimeDependencyStatus, VideoCapabilityAdvisor, VideoProgressEvent,
};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

const MAX_FFMPEG_ARCHIVE_BYTES: u64 = 320 * 1024 * 1024;
/// Versiones explícitamente pineadas para evitar drift de cadena de suministro en `pip install`.
const EDGE_TTS_PIP_SPEC: &str = "edge-tts==7.3.1";
const PILLOW_PIP_SPEC: &str = "pillow==12.2.0";

#[cfg(target_os = "windows")]
const FFMPEG_STATIC_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
#[cfg(target_os = "windows")]
const FFMPEG_SHA256_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip.sha256";
#[cfg(target_os = "windows")]
const FFMPEG_CHECKSUM_FILENAME: &str = "ffmpeg-release-essentials.zip";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const FFMPEG_STATIC_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const FFMPEG_SHA256_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/checksums.sha256";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const FFMPEG_CHECKSUM_FILENAME: &str = "ffmpeg-master-latest-linux64-gpl.tar.xz";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const FFMPEG_STATIC_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linuxarm64-gpl.tar.xz";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const FFMPEG_SHA256_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/checksums.sha256";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const FFMPEG_CHECKSUM_FILENAME: &str = "ffmpeg-master-latest-linuxarm64-gpl.tar.xz";

#[cfg(not(any(
    target_os = "windows",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64")
)))]
const FFMPEG_STATIC_URL: &str = "";
#[cfg(not(any(
    target_os = "windows",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64")
)))]
const FFMPEG_SHA256_URL: &str = "";
#[cfg(not(any(
    target_os = "windows",
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64")
)))]
const FFMPEG_CHECKSUM_FILENAME: &str = "";

fn is_valid_sha256_hex(candidate: &str) -> bool {
    candidate.len() == 64 && candidate.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Parsea un archivo de checksum SHA-256 (hash directo de 64 hex chars o salida estilo `sha256sum`)
/// y devuelve el digest hexadecimal normalizado en minúsculas.
pub(crate) fn parse_sha256_checksum(
    raw: &str,
    expected_filename: Option<&str>,
) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Archivo de checksum SHA-256 vacío".to_string());
    }

    if is_valid_sha256_hex(trimmed) {
        return Ok(trimmed.to_ascii_lowercase());
    }

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        if !is_valid_sha256_hex(hash) {
            continue;
        }
        if let Some(target_name) = expected_filename.filter(|n| !n.is_empty()) {
            let Some(rest_name) = parts.next() else {
                continue;
            };
            let cleaned_name = rest_name.trim_start_matches('*');
            let base = cleaned_name
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(cleaned_name);
            if base == target_name {
                return Ok(hash.to_ascii_lowercase());
            }
        } else {
            return Ok(hash.to_ascii_lowercase());
        }
    }

    Err(match expected_filename.filter(|n| !n.is_empty()) {
        Some(name) => format!(
            "No se encontró un SHA-256 válido para '{}' en el manifiesto",
            name
        ),
        None => "No se encontró un digest SHA-256 hexadecimal válido".to_string(),
    })
}

/// Calcula el SHA-256 en streaming de `path` y verifica que coincida con `expected_hex`.
pub(crate) fn verify_file_sha256(path: &Path, expected_hex: &str) -> Result<String, String> {
    use std::io::Read;

    let normalized_expected = parse_sha256_checksum(expected_hex, None)?;
    let mut file = std::fs::File::open(path).map_err(|e| {
        format!(
            "Error abriendo '{}' para verificar SHA-256: {}",
            path.display(),
            e
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let read = file.read(&mut buf).map_err(|e| {
            format!(
                "Error leyendo '{}' para verificar SHA-256: {}",
                path.display(),
                e
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != normalized_expected {
        return Err(format!(
            "Fallo de integridad SHA-256 en '{}': esperado {}, obtenido {}",
            path.display(),
            normalized_expected,
            actual
        ));
    }
    Ok(actual)
}

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

const SCRIPT_TTS_EDGE: &str = include_str!("../../resources/video_studio/tts_edge.py");

const SCRIPT_RENDER_CARD: &str = include_str!("../../resources/video_studio/render_card.py");

const SCRIPT_EXTRACT_FFMPEG: &str = include_str!("../../resources/video_studio/extract_ffmpeg.py");

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
                EDGE_TTS_PIP_SPEC,
                PILLOW_PIP_SPEC,
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
            0.85,
            "Verificando integridad SHA-256 de FFmpeg...",
            false,
            None,
        )
        .await;

        let expected_sha256_res: Result<String, String> = match std::env::var("AEGIS_FFMPEG_SHA256")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            Some(override_hash) => parse_sha256_checksum(&override_hash, None),
            None => {
                if FFMPEG_SHA256_URL.is_empty() {
                    Err("No hay URL de checksum SHA-256 configurada para FFmpeg".to_string())
                } else {
                    match client.get(FFMPEG_SHA256_URL).send().await {
                        Ok(sum_resp) if sum_resp.status().is_success() => {
                            match sum_resp.text().await {
                                Ok(sum_text) => {
                                    let expected_name = if FFMPEG_CHECKSUM_FILENAME.is_empty() {
                                        None
                                    } else {
                                        Some(FFMPEG_CHECKSUM_FILENAME)
                                    };
                                    parse_sha256_checksum(&sum_text, expected_name)
                                }
                                Err(e) => {
                                    Err(format!("Error leyendo checksum SHA-256 de FFmpeg: {}", e))
                                }
                            }
                        }
                        Ok(sum_resp) => Err(format!(
                            "Error HTTP obteniendo checksum SHA-256 de FFmpeg ({})",
                            sum_resp.status()
                        )),
                        Err(e) => Err(format!(
                            "Error obteniendo checksum SHA-256 de FFmpeg: {}",
                            e
                        )),
                    }
                }
            }
        };

        let expected_sha256 = match expected_sha256_res {
            Ok(h) => h,
            Err(err_msg) => {
                let _ = tokio::fs::remove_file(&archive_path).await;
                emit_install_progress(&progress_cb, 0.0, &err_msg, true, Some(err_msg.clone()))
                    .await;
                return Err(err_msg);
            }
        };

        let verify_path = archive_path.clone();
        let verify_res =
            tokio::task::spawn_blocking(move || verify_file_sha256(&verify_path, &expected_sha256))
                .await
                .map_err(|e| format!("Error interno verificando SHA-256 de FFmpeg: {}", e))?;

        if let Err(err_msg) = verify_res {
            let _ = tokio::fs::remove_file(&archive_path).await;
            emit_install_progress(&progress_cb, 0.0, &err_msg, true, Some(err_msg.clone())).await;
            return Err(err_msg);
        }

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

#[cfg(test)]
mod tests {
    use super::{
        parse_sha256_checksum, verify_file_sha256, EDGE_TTS_PIP_SPEC, FFMPEG_CHECKSUM_FILENAME,
        FFMPEG_SHA256_URL, FFMPEG_STATIC_URL, PILLOW_PIP_SPEC,
    };
    use sha2::{Digest, Sha256};

    #[test]
    fn test_pinned_python_dependencies_and_checksum_config() {
        assert!(EDGE_TTS_PIP_SPEC.contains("=="));
        assert!(PILLOW_PIP_SPEC.contains("=="));
        if !FFMPEG_STATIC_URL.is_empty() {
            assert!(!FFMPEG_SHA256_URL.is_empty());
            assert!(!FFMPEG_CHECKSUM_FILENAME.is_empty());
        }
    }

    #[test]
    fn test_parse_sha256_checksum_formats() {
        let bare = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n";
        assert_eq!(
            parse_sha256_checksum(bare, Some("ffmpeg-release-essentials.zip")),
            Ok("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string())
        );

        let manifest = "\
1111111111111111111111111111111111111111111111111111111111111111  other-build.tar.xz
2222222222222222222222222222222222222222222222222222222222222222  ./ffmpeg-master-latest-linux64-gpl.tar.xz
";
        assert_eq!(
            parse_sha256_checksum(manifest, Some("ffmpeg-master-latest-linux64-gpl.tar.xz")),
            Ok("2222222222222222222222222222222222222222222222222222222222222222".to_string())
        );

        assert!(parse_sha256_checksum(manifest, Some("missing.tar.xz")).is_err());
        assert!(parse_sha256_checksum("not-a-valid-hash", None).is_err());
    }

    #[test]
    fn test_verify_file_sha256_accepts_match_and_rejects_tampering() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let file_path = dir.path().join("ffmpeg.bin");
        let payload = b"aegis-ffmpeg-verified-binary";
        std::fs::write(&file_path, payload).map_err(|e| e.to_string())?;

        let expected = hex::encode(Sha256::digest(payload));
        let verified = verify_file_sha256(&file_path, &expected.to_ascii_uppercase())?;
        assert_eq!(verified, expected);

        let wrong = "0".repeat(64);
        assert!(verify_file_sha256(&file_path, &wrong).is_err());
        assert!(verify_file_sha256(&file_path, "short").is_err());
        Ok(())
    }
}
