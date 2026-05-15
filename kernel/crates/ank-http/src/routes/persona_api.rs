use crate::citadel::hash_passphrase;
use crate::state::AppState;
use ank_core::enclave::TenantDB;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
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
            if name.is_empty() { None } else { Some(name.to_string()) }
        })
        .unwrap_or_else(|| "Aegis".to_string())
}

fn extract_auth(headers: &HeaderMap) -> Option<(String, String)> {
    let tenant_id = headers.get("x-citadel-tenant")?.to_str().ok()?.to_string();
    let session_key = headers.get("x-citadel-key")?.to_str().ok()?.to_string();
    Some((tenant_id, session_key))
}

async fn get_persona(State(_state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some((tenant_id, session_key)) = extract_auth(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Missing auth headers" })),
        );
    };

    let hash = hash_passphrase(&session_key);
    match TenantDB::open(&tenant_id, &hash) {
        Ok(db) => {
            let persona = db.get_persona().unwrap_or(None);
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
    headers: HeaderMap,
    Json(body): Json<PersonaRequest>,
) -> impl IntoResponse {
    let Some((tenant_id, session_key)) = extract_auth(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Missing auth headers" })),
        );
    };

    let hash = hash_passphrase(&session_key);
    match TenantDB::open(&tenant_id, &hash) {
        Ok(db) => match db.set_persona(&body.persona) {
            Ok(()) => (StatusCode::OK, Json(json!({ "success": true }))),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            ),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to open enclave" })),
        ),
    }
}

async fn get_agent_name(State(_state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some((tenant_id, session_key)) = extract_auth(&headers) else {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "name": "Aegis" })));
    };
    let hash = hash_passphrase(&session_key);
    let persona = TenantDB::open(&tenant_id, &hash)
        .ok()
        .and_then(|db| db.get_persona().ok().flatten());
    let name = parse_agent_name(&persona);
    (StatusCode::OK, Json(json!({ "name": name })))
}

async fn delete_persona(State(_state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some((tenant_id, session_key)) = extract_auth(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Missing auth headers" })),
        );
    };

    let hash = hash_passphrase(&session_key);
    match TenantDB::open(&tenant_id, &hash) {
        Ok(db) => match db.delete_persona() {
            Ok(()) => (StatusCode::OK, Json(json!({ "success": true }))),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to delete persona" })),
            ),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to open enclave" })),
        ),
    }
}
