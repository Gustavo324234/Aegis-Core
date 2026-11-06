use crate::{citadel::CitadelCredentials, error::AegisHttpError, state::AppState};
use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", post(search_memory))
        .route("/index", post(index_memory))
}

#[derive(Deserialize)]
pub struct SearchMemoryRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchMemoryResponse {
    pub results: Vec<MemoryFragmentDto>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct MemoryFragmentDto {
    pub id: String,
    pub text: String,
    pub timestamp: i64,
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct IndexMemoryRequest {
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct IndexMemoryResponse {
    pub status: String,
    pub id: String,
}

/// POST /api/memory/search — Búsqueda semántica en la memoria neuronal L3 del tenant
async fn search_memory(
    State(state): State<AppState>,
    creds: Option<CitadelCredentials>,
    Json(payload): Json<SearchMemoryRequest>,
) -> Result<Json<SearchMemoryResponse>, AegisHttpError> {
    let tenant_id = creds.map(|c| c.tenant_id).unwrap_or_else(|| "default".to_string());
    let limit = payload.limit.unwrap_or(5).min(20);

    let fragments = state
        .hal
        .search_memory(&tenant_id, &payload.query, limit)
        .await
        .map_err(|e| AegisHttpError::Kernel(format!("Semantic search failed: {}", e)))?;

    let dtos: Vec<MemoryFragmentDto> = fragments
        .into_iter()
        .map(|f| MemoryFragmentDto {
            id: f.id,
            text: f.text,
            timestamp: f.timestamp,
            tags: f.tags,
        })
        .collect();

    let count = dtos.len();
    Ok(Json(SearchMemoryResponse {
        results: dtos,
        count,
    }))
}

/// POST /api/memory/index — Indexa un nuevo fragmento en la memoria neuronal L3 del tenant
async fn index_memory(
    State(state): State<AppState>,
    creds: Option<CitadelCredentials>,
    Json(payload): Json<IndexMemoryRequest>,
) -> Result<Json<IndexMemoryResponse>, AegisHttpError> {
    let tenant_id = creds.map(|c| c.tenant_id).unwrap_or_else(|| "default".to_string());

    let id = state
        .hal
        .store_memory(&tenant_id, &payload.text)
        .await
        .map_err(|e| AegisHttpError::Kernel(format!("Memory indexing failed: {}", e)))?;

    Ok(Json(IndexMemoryResponse {
        status: "indexed".to_string(),
        id,
    }))
}
