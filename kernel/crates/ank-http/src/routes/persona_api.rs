use crate::citadel::CitadelAuthenticated;
use crate::state::AppState;
use ank_core::enclave::TenantDB;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_persona))
        .route("/", post(set_persona))
        .route("/", delete(delete_persona))
        .route("/name", get(get_agent_name))
}

/// Extrae el nombre del agente de la cadena de persona.
/// Formato esperado: "Tu nombre es {name}. ..."
fn parse_agent_name(persona: &Option<String>) -> String {
    persona
        .as_deref()
        .and_then(|p| {
            let prefix = "Tu nombre es ";
            let start = p.find(prefix)? + prefix.len();
            let end = start + p[start..].find('.')?;
            let name = p[start..end].trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .unwrap_or_else(|| "Aegis".to_string())
}

async fn get_persona(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> impl IntoResponse {
    match TenantDB::with_async(auth.tenant_id, auth.session_key_hash, |db| {
        Ok(db.get_persona().unwrap_or(None))
    })
    .await
    {
        Ok(persona) => {
            let is_configured = persona.is_some();
            (
                StatusCode::OK,
                Json(
                    json!({ "persona": persona.unwrap_or_default(), "is_configured": is_configured }),
                ),
            )
        }
        Err(_) => (
            StatusCode::OK,
            Json(json!({ "persona": "", "is_configured": false })),
        ),
    }
}

#[derive(Deserialize)]
struct PersonaRequest {
    persona: String,
}

async fn set_persona(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<PersonaRequest>,
) -> impl IntoResponse {
    let persona_text = body.persona;
    match tokio::task::spawn_blocking(move || {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to open enclave".to_string(),
            )
        })?;
        db.set_persona(&persona_text)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
    })
    .await
    {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({ "success": true }))),
        Ok(Err((code, msg))) => (code, Json(json!({ "error": msg }))),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to open enclave" })),
        ),
    }
}

async fn get_agent_name(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> impl IntoResponse {
    let persona = TenantDB::with_async(auth.tenant_id, auth.session_key_hash, |db| {
        Ok(db.get_persona().ok().flatten())
    })
    .await
    .ok()
    .flatten();
    let name = parse_agent_name(&persona);
    (StatusCode::OK, Json(json!({ "name": name })))
}

async fn delete_persona(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> impl IntoResponse {
    let tid = auth.tenant_id.clone();
    match tokio::task::spawn_blocking(move || {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|_| "Failed to open enclave".to_string())?;
        db.delete_persona()
            .map_err(|_| "Failed to delete persona".to_string())
    })
    .await
    {
        Ok(Ok(())) => {
            crate::ws::chat::invalidate_onboarding_cache(&tid);
            (StatusCode::OK, Json(json!({ "success": true })))
        }
        Ok(Err(msg)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": msg })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to delete persona" })),
        ),
    }
}
