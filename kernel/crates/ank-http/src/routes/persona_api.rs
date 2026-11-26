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
        .route("/name", get(get_agent_name).post(set_agent_name))
}

/// Extrae el nombre del agente de la cadena de persona.
/// Formato esperado: "Tu nombre es {name}. ..."
pub(crate) fn parse_agent_name(persona: &Option<String>) -> String {
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
    let (explicit_name, persona) =
        TenantDB::with_async(auth.tenant_id, auth.session_key_hash, |db| {
            let explicit = db.get_kv("assistant_name").ok().flatten();
            let persona = db.get_persona().ok().flatten();
            Ok((explicit, persona))
        })
        .await
        .unwrap_or((None, None));

    let name = explicit_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| parse_agent_name(&persona));
    (StatusCode::OK, Json(json!({ "name": name })))
}

#[derive(Deserialize)]
struct AgentNameRequest {
    name: String,
}

async fn set_agent_name(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<AgentNameRequest>,
) -> impl IntoResponse {
    let trimmed = body.name.trim().to_string();
    if trimmed.is_empty() || trimmed.chars().count() > 40 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "El nombre debe tener entre 1 y 40 caracteres." })),
        );
    }

    let tid = auth.tenant_id.clone();
    let name_clone = trimmed.clone();
    match tokio::task::spawn_blocking(move || -> Result<(), String> {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(|_| "Failed to open enclave".to_string())?;
        db.set_kv("assistant_name", &name_clone)
            .map_err(|e| e.to_string())?;

        // Keep persona prefix aligned if already customized, or leave persona coherent.
        if let Ok(Some(existing)) = db.get_persona() {
            let updated = if let Some(rest_idx) = existing.find("Tu nombre es ") {
                let after = &existing[rest_idx + "Tu nombre es ".len()..];
                if let Some(dot_idx) = after.find('.') {
                    format!(
                        "{}Tu nombre es {}.{}",
                        &existing[..rest_idx],
                        name_clone,
                        &after[dot_idx + 1..]
                    )
                } else {
                    format!("Tu nombre es {}. {}", name_clone, existing)
                }
            } else {
                format!("Tu nombre es {}. {}", name_clone, existing)
            };
            let _ = db.set_persona(&updated);
        }
        Ok(())
    })
    .await
    {
        Ok(Ok(())) => {
            crate::ws::chat::invalidate_onboarding_cache(&tid);
            (
                StatusCode::OK,
                Json(json!({ "success": true, "name": trimmed })),
            )
        }
        Ok(Err(msg)) => (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to update assistant name" })),
        ),
    }
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
