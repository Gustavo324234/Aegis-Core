use crate::{citadel::CitadelAuthenticated, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tracing::{error, info};

// ---------------------------------------------------------------------------
// Static download state (one global download at a time)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize)]
pub struct SttDownloadState {
    pub downloading: bool,
    pub progress: f32, // 0.0 – 1.0
    pub current_model: Option<String>,
    pub error: Option<String>,
}

static STT_STATE: OnceLock<Arc<Mutex<SttDownloadState>>> = OnceLock::new();

fn stt_state() -> Arc<Mutex<SttDownloadState>> {
    STT_STATE
        .get_or_init(|| Arc::new(Mutex::new(SttDownloadState::default())))
        .clone()
}

// ---------------------------------------------------------------------------
// Whisper model catalogue
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct WhisperModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub size_mb: u32,
    pub ram_mb: u32,
    pub recommended: bool,
    pub description: &'static str,
    pub url: &'static str,
}

const WHISPER_MODELS: &[WhisperModelInfo] = &[
    WhisperModelInfo {
        id: "tiny",
        label: "Tiny",
        size_mb: 75,
        ram_mb: 150,
        recommended: false,
        description: "Más rápido, menor precisión. Ideal para hardware muy limitado.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
    },
    WhisperModelInfo {
        id: "base",
        label: "Base",
        size_mb: 142,
        ram_mb: 290,
        recommended: true,
        description: "Mejor balance calidad/velocidad. Recomendado para la mayoría de sistemas.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
    },
    WhisperModelInfo {
        id: "small",
        label: "Small",
        size_mb: 466,
        ram_mb: 900,
        recommended: false,
        description: "Mayor precisión, aún eficiente. Buena opción con 2+ GB de RAM libre.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
    },
    WhisperModelInfo {
        id: "medium",
        label: "Medium",
        size_mb: 1500,
        ram_mb: 3100,
        recommended: false,
        description: "Alta precisión, requiere 4+ GB RAM. Para servidores dedicados.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
    },
    WhisperModelInfo {
        id: "large-v3",
        label: "Large v3",
        size_mb: 3100,
        ram_mb: 6200,
        recommended: false,
        description: "Máxima precisión. Requiere 8+ GB RAM. Solo para hardware profesional.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
    },
];

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/models", get(list_models))
        .route("/download", post(start_download))
        .route("/status", get(get_download_status))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_models(
    _auth: CitadelAuthenticated,
) -> Json<Value> {
    Json(json!({ "models": WHISPER_MODELS }))
}

#[derive(Deserialize)]
struct DownloadRequest {
    model: String,
}

async fn start_download(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(req): Json<DownloadRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    let model_info = WHISPER_MODELS
        .iter()
        .find(|m| m.id == req.model)
        .ok_or_else(|| {
            AegisHttpError::Internal(anyhow::anyhow!("Unknown model: {}", req.model))
        })?;

    {
        let stt_arc = stt_state();
        let mut st = stt_arc.lock().await;
        if st.downloading {
            return Ok(Json(json!({
                "ok": false,
                "message": "Ya hay una descarga en progreso."
            })));
        }
        st.downloading = true;
        st.progress = 0.0;
        st.current_model = Some(req.model.clone());
        st.error = None;
    }

    let model_id = model_info.id.to_string();
    let url = model_info.url.to_string();
    let models_dir = state.config.data_dir.join("models");
    let stt_st = stt_state();

    tokio::spawn(async move {
        if let Err(e) = download_model(url, model_id.clone(), models_dir, stt_st.clone()).await {
            error!("STT model download failed: {}", e);
            let mut st = stt_st.lock().await;
            st.downloading = false;
            st.error = Some(e.to_string());
            st.current_model = None;
        }
    });

    Ok(Json(json!({ "ok": true, "message": "Descarga iniciada." })))
}

async fn get_download_status(
    _auth: CitadelAuthenticated,
) -> Json<Value> {
    let stt_arc = stt_state();
    let st = stt_arc.lock().await;
    Json(json!({
        "downloading": st.downloading,
        "progress": st.progress,
        "current_model": st.current_model,
        "error": st.error,
    }))
}

// ---------------------------------------------------------------------------
// Background download task
// ---------------------------------------------------------------------------

async fn download_model(
    url: String,
    model_id: String,
    models_dir: std::path::PathBuf,
    state: Arc<Mutex<SttDownloadState>>,
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(&models_dir).await?;

    let tmp_path = models_dir.join(format!("ggml-{}.bin.downloading", model_id));
    let final_path = models_dir.join(format!("ggml-{}.bin", model_id));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("HTTP {} al descargar modelo", response.status());
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let progress = if total > 0 {
            downloaded as f32 / total as f32
        } else {
            0.0
        };

        let mut st = state.lock().await;
        st.progress = progress;
        drop(st);
    }

    file.flush().await?;
    drop(file);

    tokio::fs::rename(&tmp_path, &final_path).await?;

    // Persist which model is active
    let active_path = models_dir.join("active_model.txt");
    tokio::fs::write(&active_path, &model_id).await?;

    info!("STT model '{}' downloaded and activated.", model_id);

    let mut st = state.lock().await;
    st.downloading = false;
    st.progress = 1.0;
    st.error = None;

    Ok(())
}
