use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use ank_core::chal::drivers::siren::whisper_local::is_safe_whisper_model_id;
use ank_core::scheduler::persistence::VoiceProfile;
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use base64::Engine as _;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, OnceLock, Weak,
};
use std::time::Duration;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

const SECRET_MASK_PREFIX: &str = "••••••••";
const MAX_ENROLL_B64_BYTES: usize = 8 * 1024 * 1024;
const MAX_SDP_OFFER_BYTES: usize = 64 * 1024;
const MAX_WEBRTC_PCM_SAMPLES: usize = 16_000 * 120; // ~2 min @ 16kHz mono
const MAX_ACTIVE_WEBRTC_PER_TENANT: usize = 4;

type ActivePeerRegistry = DashMap<String, Vec<(u64, Arc<RTCPeerConnection>)>>;

static NEXT_WEBRTC_CONN_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_WEBRTC_PEERS: OnceLock<ActivePeerRegistry> = OnceLock::new();

fn active_webrtc_peers() -> &'static ActivePeerRegistry {
    ACTIVE_WEBRTC_PEERS.get_or_init(DashMap::new)
}

fn unregister_webrtc_peer(tenant_id: &str, conn_id: u64) -> Option<Arc<RTCPeerConnection>> {
    let registry = active_webrtc_peers();
    let mut removed = None;
    let mut remove_entry = false;
    if let Some(mut list) = registry.get_mut(tenant_id) {
        if let Some(pos) = list.iter().position(|(id, _)| *id == conn_id) {
            removed = Some(list.remove(pos).1);
        }
        remove_entry = list.is_empty();
    }
    if remove_entry {
        registry.remove_if(tenant_id, |_, list| list.is_empty());
    }
    removed
}

fn mask_secret(secret: &str) -> String {
    if secret.is_empty() {
        return String::new();
    }
    let char_count = secret.chars().count();
    if char_count <= 4 {
        return SECRET_MASK_PREFIX.to_string();
    }
    let tail: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}{}", SECRET_MASK_PREFIX, tail)
}

fn resolve_secret_input(incoming: &str, existing: &str) -> String {
    if incoming.starts_with("••••") {
        existing.to_string()
    } else {
        incoming.to_string()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_siren_config))
        .route("/config", post(set_siren_config))
        .route("/voices", get(list_siren_voices))
        .route("/enroll", post(enroll_speaker))
        .route("/enroll", delete(delete_enrollment))
        .route("/enroll/status", get(enrollment_status))
        .route("/webrtc/offer/:tenant_id", post(webrtc_offer_handler))
}

#[derive(Deserialize)]
pub struct SirenConfigBody {
    pub provider: String,
    pub api_key: String,
    pub voice_id: String,
    #[serde(default)]
    pub stt_provider: String,
    #[serde(default)]
    pub stt_api_key: String,
}

async fn get_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let models_dir = state.config.data_dir.join("models");
    let active_model_path = models_dir.join("active_model.txt");
    let (stt_available, active_model) =
        if let Ok(name) = std::fs::read_to_string(&active_model_path) {
            let name = name.trim().to_string();
            if is_safe_whisper_model_id(&name) {
                let model_file = models_dir.join(format!("ggml-{}.bin", name));
                (model_file.exists(), Some(name))
            } else {
                tracing::warn!(
                    model = %name,
                    "Siren API: unsafe model id in active_model.txt; ignoring."
                );
                (false, None)
            }
        } else {
            // backwards-compat: accept ggml-base.bin without active_model.txt
            let legacy = models_dir.join("ggml-base.bin");
            if legacy.exists() {
                (true, Some("base".to_string()))
            } else {
                (false, None)
            }
        };

    let profile = state
        .persistence
        .get_voice_profile(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    match profile {
        Some(p) => {
            let settings = serde_json::from_str::<serde_json::Value>(&p.settings_json).ok();
            let raw_api_key = settings
                .as_ref()
                .and_then(|v| v["api_key"].as_str())
                .unwrap_or_default();
            let stt_provider = settings
                .as_ref()
                .and_then(|v| v["stt_provider"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "browser".to_string());
            let raw_stt_api_key = settings
                .as_ref()
                .and_then(|v| v["stt_api_key"].as_str())
                .unwrap_or_default();
            Ok(Json(json!({
                "provider": p.engine_id,
                "voice_id": p.voice_id,
                "api_key": mask_secret(raw_api_key),
                "stt_provider": stt_provider,
                "stt_api_key": mask_secret(raw_stt_api_key),
                "configured": true,
                "stt_available": stt_available,
                "active_model": active_model
            })))
        }
        None => Ok(Json(json!({
            "provider": "mock",
            "voice_id": "",
            "api_key": "",
            "stt_provider": "browser",
            "stt_api_key": "",
            "configured": false,
            "stt_available": stt_available,
            "active_model": active_model
        }))),
    }
}

async fn set_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<SirenConfigBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let existing = state
        .persistence
        .get_voice_profile(&auth.tenant_id)
        .await
        .unwrap_or(None);

    let existing_settings = existing
        .as_ref()
        .and_then(|p| serde_json::from_str::<serde_json::Value>(&p.settings_json).ok());
    let existing_api_key = existing_settings
        .as_ref()
        .and_then(|v| v["api_key"].as_str())
        .unwrap_or_default();
    let existing_stt_api_key = existing_settings
        .as_ref()
        .and_then(|v| v["stt_api_key"].as_str())
        .unwrap_or_default();

    let resolved_api_key = resolve_secret_input(&req.api_key, existing_api_key);
    let resolved_stt_api_key = resolve_secret_input(&req.stt_api_key, existing_stt_api_key);

    let profile = VoiceProfile {
        tenant_id: auth.tenant_id.clone(),
        engine_id: req.provider,
        voice_id: req.voice_id,
        model_pref: existing
            .map(|p| p.model_pref)
            .unwrap_or_else(|| "HybridSmart".to_string()),
        settings_json: json!({
            "api_key": resolved_api_key,
            "stt_provider": req.stt_provider,
            "stt_api_key": resolved_stt_api_key
        })
        .to_string(),
    };

    state
        .persistence
        .update_voice_profile(profile)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Siren config updated successfully."
    })))
}

#[derive(Deserialize)]
pub struct EnrollBody {
    pub pcm_b64: String,
    #[serde(default)]
    pub threshold: Option<f32>,
}

async fn enroll_speaker(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<EnrollBody>,
) -> Result<Json<Value>, AegisHttpError> {
    if req.pcm_b64.len() > MAX_ENROLL_B64_BYTES {
        return Err(AegisHttpError::BadRequest(
            "Enrollment audio payload exceeds 8 MiB limit".to_string(),
        ));
    }

    let threshold = req
        .threshold
        .unwrap_or(ank_core::speaker_id::DEFAULT_THRESHOLD);
    if !threshold.is_finite() || !(0.1..=0.99).contains(&threshold) {
        return Err(AegisHttpError::BadRequest(
            "Invalid verification threshold (expected 0.10..=0.99)".to_string(),
        ));
    }

    let pcm_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.pcm_b64)
        .map_err(|e| AegisHttpError::BadRequest(format!("Invalid base64 PCM: {}", e)))?;

    let fingerprint = ank_core::speaker_id::extract_fingerprint(&pcm_bytes).ok_or_else(|| {
        AegisHttpError::BadRequest(
            "Audio demasiado corto para enrollment (mínimo 25ms)".to_string(),
        )
    })?;

    state
        .persistence
        .save_voice_fingerprint(&auth.tenant_id, &fingerprint, threshold)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Voice fingerprint enrolled successfully.",
        "threshold": threshold
    })))
}

async fn delete_enrollment(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    state
        .persistence
        .delete_voice_fingerprint(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({
        "success": true,
        "message": "Voice enrollment deleted."
    })))
}

async fn enrollment_status(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<Value>, AegisHttpError> {
    let enrolled = state
        .persistence
        .get_voice_fingerprint(&auth.tenant_id)
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?
        .is_some();

    Ok(Json(json!({ "enrolled": enrolled })))
}

async fn list_siren_voices(_auth: CitadelAuthenticated) -> Json<Value> {
    Json(json!({
        "voices": [
            { "id": "aura-asteria-en", "name": "Asteria (EN)", "provider": "voxtral" },
            { "id": "aura-luna-en", "name": "Luna (EN)", "provider": "voxtral" },
            { "id": "mock-voice", "name": "Mock Voice", "provider": "mock" },
            { "id": "21m00Tcm4TlvDq8ikWAM", "name": "Rachel (EN)", "provider": "elevenlabs" },
            { "id": "AZnzlk1XvdvUeBnXmlld", "name": "Domi (EN)", "provider": "elevenlabs" },
            { "id": "EXAVITQu4vr4xnSDxMaL", "name": "Bella (EN)", "provider": "elevenlabs" },
            { "id": "ErXwobaYiN019PkySvjV", "name": "Antoni (EN)", "provider": "elevenlabs" },
            { "id": "MF3mGyEYCl7XYWbV9V6O", "name": "Elli (EN)", "provider": "elevenlabs" },
            { "id": "TxGEqnHWrfWFTfGW9XjX", "name": "Josh (EN)", "provider": "elevenlabs" },
            { "id": "pNInz6obpgDQGcFmaJgB", "name": "Adam (EN)", "provider": "elevenlabs" }
        ]
    }))
}

// ── WebRTC Signaling Structs ────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct WebRtcOffer {
    pub sdp: String,
}

#[derive(Serialize)]
pub struct WebRtcAnswer {
    pub sdp: String,
}

// ── Audio Resampling Helper ──────────────────────────────────────────────────
fn resample_linear(input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if from_rate == 0 || to_rate == 0 || input.is_empty() {
        return Vec::new();
    }
    if from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (input.len() as f64 * (to_rate as f64 / from_rate as f64)) as usize;
    let mut output = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = pos - idx as f64;
        if idx + 1 < input.len() {
            let sample = (input[idx] as f64 * (1.0 - frac) + input[idx + 1] as f64 * frac) as i16;
            output.push(sample);
        } else if idx < input.len() {
            output.push(input[idx]);
        }
    }
    output
}

// ── WebRTC Offer Handler ─────────────────────────────────────────────────────
async fn webrtc_offer_handler(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(tenant_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<WebRtcOffer>,
) -> Result<Json<WebRtcAnswer>, AegisHttpError> {
    if !auth.is_master && auth.tenant_id != tenant_id {
        return Err(AegisHttpError::Forbidden(
            "Cross-tenant WebRTC signaling denied".to_string(),
        ));
    }

    if req.sdp.is_empty() || req.sdp.len() > MAX_SDP_OFFER_BYTES {
        return Err(AegisHttpError::BadRequest(
            "Invalid or oversized SDP offer".to_string(),
        ));
    }

    let session_key = headers
        .get("x-citadel-key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();

    tracing::info!(
        "Siren WebRTC: Iniciando negociacion SDP para tenant={}",
        tenant_id
    );

    // 1. Configurar MediaEngine y APIBuilder
    let mut m = MediaEngine::default();
    m.register_default_codecs().map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("MediaEngine setup failed: {}", e))
    })?;

    let api = APIBuilder::new().with_media_engine(m).build();

    let config = RTCConfiguration {
        ice_servers: vec![webrtc::ice_transport::ice_server::RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(config).await.map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to create PeerConnection: {}", e))
    })?);

    // 1b. Acotar sesiones WebRTC activas por tenant y cerrar conexiones obsoletas si exceden el límite
    let conn_id = NEXT_WEBRTC_CONN_ID.fetch_add(1, Ordering::Relaxed);
    let mut evicted_peers: Vec<Arc<RTCPeerConnection>> = Vec::new();
    {
        let mut entry = active_webrtc_peers().entry(tenant_id.clone()).or_default();
        while entry.len() >= MAX_ACTIVE_WEBRTC_PER_TENANT {
            let (_, old_pc) = entry.remove(0);
            evicted_peers.push(old_pc);
        }
        entry.push((conn_id, Arc::clone(&peer_connection)));
    }
    for old_pc in evicted_peers {
        tracing::info!(
            tenant = %tenant_id,
            "Siren WebRTC: Closing oldest PeerConnection (per-tenant cap reached)."
        );
        let _ = old_pc.close().await;
    }

    // 2. Configurar Track de Salida (TTS Opus)
    let local_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: "audio/opus".to_owned(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    if let Err(e) = peer_connection
        .add_track(Arc::clone(&local_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        let _ = unregister_webrtc_peer(&tenant_id, conn_id);
        let _ = peer_connection.close().await;
        return Err(AegisHttpError::Internal(anyhow::anyhow!(
            "Failed to add WebRTC audio track: {}",
            e
        )));
    }

    // 3. Crear buffer de audio compartido (acotado a MAX_WEBRTC_PCM_SAMPLES)
    let shared_audio = Arc::new(tokio::sync::Mutex::new(Vec::<i16>::new()));

    // 3b. Cerrar PeerConnection y liberar memoria ante desconexión/fallo ICE (sin ciclo Arc)
    let pc_weak: Weak<RTCPeerConnection> = Arc::downgrade(&peer_connection);
    let shared_audio_cleanup = Arc::clone(&shared_audio);
    let tenant_cleanup = tenant_id.clone();
    peer_connection.on_peer_connection_state_change(Box::new(
        move |conn_state: RTCPeerConnectionState| {
            let pc_weak = pc_weak.clone();
            let audio_ref = Arc::clone(&shared_audio_cleanup);
            let tenant_ref = tenant_cleanup.clone();
            Box::pin(async move {
                if matches!(
                    conn_state,
                    RTCPeerConnectionState::Failed
                        | RTCPeerConnectionState::Disconnected
                        | RTCPeerConnectionState::Closed
                ) {
                    tracing::info!(
                        tenant = %tenant_ref,
                        state = ?conn_state,
                        "Siren WebRTC: PeerConnection state terminal; releasing resources."
                    );
                    {
                        let mut buf = audio_ref.lock().await;
                        buf.clear();
                        buf.shrink_to_fit();
                    }
                    let removed = unregister_webrtc_peer(&tenant_ref, conn_id);
                    if conn_state != RTCPeerConnectionState::Closed {
                        if let Some(pc) = removed.or_else(|| pc_weak.upgrade()) {
                            let _ = pc.close().await;
                        }
                    }
                }
            })
        },
    ));

    // 4. Configurar handler para el Track de entrada remoto (Microfono Opus RTP)
    let shared_audio_clone = Arc::clone(&shared_audio);
    peer_connection.on_track(Box::new(move |track, _receiver, _| {
        let shared_audio_track = Arc::clone(&shared_audio_clone);
        Box::pin(async move {
            tracing::info!("Siren WebRTC: Track de audio remoto detectado.");
            tokio::spawn(async move {
                let mut decoder = match opus::Decoder::new(16000, opus::Channels::Mono) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::error!("Siren WebRTC: Failed to create Opus Decoder: {}", e);
                        return;
                    }
                };
                let mut pcm_buf = vec![0i16; 1920];

                while let Ok((rtp_packet, _)) = track.read_rtp().await {
                    let payload = rtp_packet.payload;
                    match decoder.decode(&payload, &mut pcm_buf, false) {
                        Ok(len) => {
                            let mut buf = shared_audio_track.lock().await;
                            if buf.len().saturating_add(len) > MAX_WEBRTC_PCM_SAMPLES {
                                let excess = (buf.len() + len) - MAX_WEBRTC_PCM_SAMPLES;
                                let drain_cnt = excess.min(buf.len());
                                buf.drain(..drain_cnt);
                            }
                            buf.extend_from_slice(&pcm_buf[..len]);
                        }
                        Err(e) => {
                            tracing::warn!("Siren WebRTC: Opus decode error: {}", e);
                        }
                    }
                }
                tracing::info!("Siren WebRTC: Track de audio remoto cerrado.");
            });
        })
    }));

    // 5. Configurar handler de DataChannel para recepcion de control (VAD_END_SIGNAL)
    let shared_audio_dc = Arc::clone(&shared_audio);
    let local_track_dc = Arc::clone(&local_track);
    let state_dc = state.clone();
    let tenant_id_dc = tenant_id.clone();
    let session_key_dc = session_key.clone();

    peer_connection.on_data_channel(Box::new(move |d| {
        let shared_audio_dc = Arc::clone(&shared_audio_dc);
        let local_track_dc = Arc::clone(&local_track_dc);
        let state_dc = state_dc.clone();
        let tenant_id_dc = tenant_id_dc.clone();
        let session_key_dc = session_key_dc.clone();

        Box::pin(async move {
            tracing::info!("Siren WebRTC: DataChannel establecido.");

            let d_clone = Arc::clone(&d);
            d.on_message(Box::new(move |msg| {
                let shared_audio_dc = Arc::clone(&shared_audio_dc);
                let local_track_dc = Arc::clone(&local_track_dc);
                let state_dc = state_dc.clone();
                let tenant_id_dc = tenant_id_dc.clone();
                let session_key_dc = session_key_dc.clone();
                let d_inner = Arc::clone(&d_clone);

                Box::pin(async move {
                    let text = String::from_utf8_lossy(&msg.data);
                    if text.contains("VAD_END_SIGNAL") {
                        tracing::info!("Siren WebRTC: VAD_END_SIGNAL recibido.");

                        // Evento VAD_START
                        let _ = d_inner
                            .send_text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "VAD_START" }
                                })
                                .to_string(),
                            )
                            .await;

                        // Evento STT_START
                        let _ = d_inner
                            .send_text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "STT_START" }
                                })
                                .to_string(),
                            )
                            .await;

                        // Obtener muestras y vaciar buffer
                        let pcm_samples = {
                            let mut buf = shared_audio_dc.lock().await;
                            std::mem::take(&mut *buf)
                        };

                        if pcm_samples.is_empty() {
                            let _ = d_inner
                                .send_text(
                                    json!({
                                        "event": "siren_event",
                                        "data": { "event_type": "STT_EMPTY" }
                                    })
                                    .to_string(),
                                )
                                .await;
                            return;
                        }

                        // Convertir a bytes Little-Endian
                        let mut pcm_bytes = Vec::with_capacity(pcm_samples.len() * 2);
                        for sample in pcm_samples {
                            pcm_bytes.extend_from_slice(&sample.to_le_bytes());
                        }

                        // Procesar STT
                        let transcript = match state_dc
                            .siren_router
                            .process_audio(&tenant_id_dc, pcm_bytes)
                            .await
                        {
                            Ok(t) => t,
                            Err(e) => {
                                tracing::error!("Siren WebRTC: STT Processing failed: {}", e);
                                let _ = d_inner
                                    .send_text(
                                        json!({
                                            "event": "siren_event",
                                            "data": { "event_type": "STT_ERROR", "message": e.to_string() }
                                        })
                                        .to_string(),
                                    )
                                    .await;
                                return;
                            }
                        };

                        if transcript.trim().is_empty() {
                            tracing::info!(
                                "Siren WebRTC: Empty STT transcript; skipping LLM task."
                            );
                            let _ = d_inner
                                .send_text(
                                    json!({
                                        "event": "siren_event",
                                        "data": { "event_type": "STT_EMPTY" }
                                    })
                                    .to_string(),
                                )
                                .await;
                            return;
                        }

                        // Agendar tarea en el Scheduler
                        let mut pcb =
                            ank_core::PCB::new("Voice Task".to_string(), 5, transcript.clone());
                        pcb.tenant_id = Some(tenant_id_dc.clone());
                        pcb.session_key = Some(session_key_dc.clone());
                        pcb.task_type = ank_core::pcb::TaskType::Chat;
                        let pid = pcb.pid.clone();

                        let (output_tx, output_rx) = tokio::sync::oneshot::channel::<String>();

                        if let Err(e) = state_dc
                            .scheduler_tx
                            .send(ank_core::SchedulerEvent::ScheduleTaskConfirmed(
                                Box::new(pcb),
                                output_tx,
                            ))
                            .await
                        {
                            tracing::error!(
                                "Siren WebRTC: Failed to schedule STT transcript: {}",
                                e
                            );
                            return;
                        }

                        // Enviar STT_DONE
                        let payload = json!({ "transcript": transcript, "pid": pid }).to_string();
                        let _ = d_inner
                            .send_text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "STT_DONE", "message": payload }
                                })
                                .to_string(),
                            )
                            .await;

                        // Esperar respuesta LLM
                        let llm_output =
                            match tokio::time::timeout(Duration::from_secs(60), output_rx).await {
                                Ok(Ok(output)) => output,
                                _ => {
                                    tracing::warn!(
                                        "Siren WebRTC: LLM response channel error or timeout."
                                    );
                                    return;
                                }
                            };

                        // Sintetizar TTS
                        let engine = match state_dc.siren_router.resolve(&tenant_id_dc).await {
                            Ok(e) => e,
                            Err(e) => {
                                tracing::warn!("Siren WebRTC: No TTS engine available: {}", e);
                                return;
                            }
                        };

                        let tts_bytes = match engine.synthesize(llm_output).await {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                tracing::warn!("Siren WebRTC: TTS synthesis failed: {}", e);
                                return;
                            }
                        };

                        // Remuestrear TTS PCM a 48kHz según el sample_rate real del motor
                        let src_rate = engine.sample_rate();
                        let mut samples_src = Vec::with_capacity(tts_bytes.len() / 2);
                        for chunk in tts_bytes.chunks_exact(2) {
                            samples_src.push(i16::from_le_bytes([chunk[0], chunk[1]]));
                        }

                        let samples_48000 = resample_linear(&samples_src, src_rate, 48000);

                        // Codificar Opus a 48kHz mono con cadencia RTP real de 20ms por frame
                        let mut encoder = match opus::Encoder::new(
                            48000,
                            opus::Channels::Mono,
                            opus::Application::Voip,
                        ) {
                            Ok(enc) => enc,
                            Err(e) => {
                                tracing::error!(
                                    "Siren WebRTC: Failed to create Opus Encoder: {}",
                                    e
                                );
                                return;
                            }
                        };

                        const OPUS_FRAME_SIZE: usize = 960;
                        let mut opus_buf = vec![0u8; 2048];
                        let mut frame_ticker = tokio::time::interval(Duration::from_millis(20));
                        frame_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                        for chunk in samples_48000.chunks(OPUS_FRAME_SIZE) {
                            frame_ticker.tick().await;
                            let mut padded_chunk = vec![0i16; OPUS_FRAME_SIZE];
                            padded_chunk[..chunk.len()].copy_from_slice(chunk);

                            match encoder.encode(&padded_chunk, &mut opus_buf) {
                                Ok(len) => {
                                    let sample = webrtc::media::Sample {
                                        data: bytes::Bytes::copy_from_slice(&opus_buf[..len]),
                                        duration: Duration::from_millis(20),
                                        ..Default::default()
                                    };
                                    if let Err(e) = local_track_dc.write_sample(&sample).await {
                                        tracing::error!(
                                            "Siren WebRTC: Failed to write sample: {}",
                                            e
                                        );
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Siren WebRTC: Opus encode error: {}", e);
                                }
                            }
                        }

                        // Enviar TTS_DONE
                        let _ = d_inner
                            .send_text(
                                json!({
                                    "event": "siren_event",
                                    "data": { "event_type": "TTS_DONE" }
                                })
                                .to_string(),
                            )
                            .await;
                    }
                })
            }));
        })
    }));

    // 6. Aplicar la oferta y generar la respuesta SDP
    let offer = match RTCSessionDescription::offer(req.sdp) {
        Ok(o) => o,
        Err(e) => {
            unregister_webrtc_peer(&tenant_id, conn_id);
            let _ = peer_connection.close().await;
            return Err(AegisHttpError::BadRequest(format!(
                "Failed to parse SDP offer: {}",
                e
            )));
        }
    };

    if let Err(e) = peer_connection.set_remote_description(offer).await {
        unregister_webrtc_peer(&tenant_id, conn_id);
        let _ = peer_connection.close().await;
        return Err(AegisHttpError::BadRequest(format!(
            "Failed to set remote description: {}",
            e
        )));
    }

    let answer = match peer_connection.create_answer(None).await {
        Ok(a) => a,
        Err(e) => {
            unregister_webrtc_peer(&tenant_id, conn_id);
            let _ = peer_connection.close().await;
            return Err(AegisHttpError::Internal(anyhow::anyhow!(
                "Failed to create answer: {}",
                e
            )));
        }
    };

    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    if let Err(e) = peer_connection.set_local_description(answer).await {
        unregister_webrtc_peer(&tenant_id, conn_id);
        let _ = peer_connection.close().await;
        return Err(AegisHttpError::Internal(anyhow::anyhow!(
            "Failed to set local description: {}",
            e
        )));
    }

    // Esperar recoleccion de ICE locales con timeout acotado (evita colgar ante STUN caído)
    if tokio::time::timeout(Duration::from_secs(10), gather_complete.recv())
        .await
        .is_err()
    {
        tracing::warn!(
            tenant = %tenant_id,
            "Siren WebRTC: ICE gathering timed out after 10s; returning partial local SDP."
        );
    }

    let local_desc = match peer_connection.local_description().await {
        Some(desc) => desc,
        None => {
            unregister_webrtc_peer(&tenant_id, conn_id);
            let _ = peer_connection.close().await;
            return Err(AegisHttpError::Internal(anyhow::anyhow!(
                "Failed to extract local SDP answer"
            )));
        }
    };

    tracing::info!("Siren WebRTC: SDP Answer generada y enviada.");

    Ok(Json(WebRtcAnswer {
        sdp: local_desc.sdp,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_and_resolve_secret_roundtrip() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("abc"), "••••••••");
        assert_eq!(mask_secret("sk_live_1234567890abcd"), "••••••••abcd");

        let masked = mask_secret("sk_live_1234567890abcd");
        assert_eq!(
            resolve_secret_input(&masked, "sk_live_1234567890abcd"),
            "sk_live_1234567890abcd"
        );
        assert_eq!(
            resolve_secret_input("sk_new_secret_9999", "sk_live_1234567890abcd"),
            "sk_new_secret_9999"
        );
        assert_eq!(resolve_secret_input("", "sk_live_1234567890abcd"), "");
    }

    #[test]
    fn test_resample_linear_16k_and_22k_to_48k() {
        let input_16k = vec![1000i16; 160]; // 10ms @ 16kHz
        let out_48k = resample_linear(&input_16k, 16000, 48000);
        assert_eq!(out_48k.len(), 480); // 10ms @ 48kHz
        assert!(out_48k.iter().all(|&s| (s - 1000).abs() <= 1));

        let input_22k = vec![-500i16; 2205]; // 100ms @ 22.05kHz
        let out_22_to_48 = resample_linear(&input_22k, 22050, 48000);
        assert_eq!(out_22_to_48.len(), 4800);
        assert_eq!(resample_linear(&[], 16000, 48000).len(), 0);
        assert_eq!(resample_linear(&input_16k, 0, 48000).len(), 0);
    }
}
