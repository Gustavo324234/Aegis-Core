use crate::citadel::CitadelAuthenticated;
use crate::error::AegisHttpError;
use crate::state::AppState;
use ank_core::enclave::TenantDB;
use ank_core::pcb::PCB;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/summary", get(get_personal_summary))
        .route("/facts/:id", delete(delete_fact).put(update_fact))
        .route("/reminders/:id", delete(delete_reminder))
        .route("/reminders/:id/complete", post(complete_reminder))
        .route("/chronos/consolidate", post(consolidate_chronos))
}

#[derive(Debug, Deserialize)]
pub struct ChronosEventsQuery {
    pub limit: Option<u32>,
}

pub async fn get_chronos_events(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Query(query): Query<ChronosEventsQuery>,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let reminders = db.get_reminders(limit).unwrap_or_default();
    let now_naive = chrono::Local::now().naive_local();

    let events: Vec<serde_json::Value> = reminders
        .into_iter()
        .filter(|r| r.get("status").and_then(|s| s.as_str()) == Some("pending"))
        .map(|r| {
            let id = r.get("id").cloned().unwrap_or(json!(0));
            let title = r
                .get("description")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let time = r
                .get("remind_at")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let recurrence = r
                .get("recurrence")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let urgency = ank_core::enclave::parse_reminder_datetime(&time)
                .map(|dt| {
                    let mins = (dt - now_naive).num_minutes();
                    if mins <= 120 {
                        "high"
                    } else {
                        "normal"
                    }
                })
                .unwrap_or("normal");
            json!({
                "id": id,
                "title": title,
                "time": time,
                "recurrence": recurrence,
                "urgency": urgency,
            })
        })
        .collect();

    Ok(Json(json!({ "events": events })))
}

async fn get_personal_summary(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let facts = db.get_personal_facts(50).unwrap_or_default();
    let ledger = db.query_expenses_summary(None).unwrap_or_else(|_| {
        json!({
            "total": 0.0,
            "totals_by_category": {},
            "recent_entries": []
        })
    });
    let reminders = db.get_reminders(20).unwrap_or_default();

    Ok(Json(json!({
        "facts": facts,
        "ledger": ledger,
        "reminders": reminders
    })))
}

async fn delete_fact(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let deleted = db
        .delete_personal_fact(id)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to delete fact: {}", e)))?;

    if deleted {
        Ok(Json(json!({ "success": true })))
    } else {
        Err(AegisHttpError::NotFound("Fact not found".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateFactRequest {
    pub category: Option<String>,
    pub fact: String,
}

async fn update_fact(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<i64>,
    Json(body): Json<UpdateFactRequest>,
) -> Result<impl IntoResponse, AegisHttpError> {
    let trimmed = body.fact.trim();
    if trimmed.is_empty() {
        return Err(AegisHttpError::BadRequest(
            "El contenido de la nota no puede estar vacío".to_string(),
        ));
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let updated = db
        .update_personal_fact(id, body.category.as_deref(), trimmed, "user_ui")
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to update fact: {}", e)))?;

    if updated {
        Ok(Json(json!({ "success": true, "id": id, "fact": trimmed })))
    } else {
        Err(AegisHttpError::NotFound("Fact not found".to_string()))
    }
}

async fn delete_reminder(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let count = db.delete_reminder(&id.to_string()).map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to delete reminder: {}", e))
    })?;

    if count > 0 {
        Ok(Json(json!({ "success": true, "deleted_count": count })))
    } else {
        Err(AegisHttpError::NotFound("Reminder not found".to_string()))
    }
}

async fn complete_reminder(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let count = db.complete_reminder(&id.to_string()).map_err(|e| {
        AegisHttpError::Internal(anyhow::anyhow!("Failed to complete reminder: {}", e))
    })?;

    if count > 0 {
        Ok(Json(json!({ "success": true, "matched_count": count })))
    } else {
        Err(AegisHttpError::NotFound(
            "Pending reminder not found".to_string(),
        ))
    }
}

async fn consolidate_chronos(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<impl IntoResponse, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("Failed to open TenantDB: {}", e)))?;

    let workspace_log_path = state
        .config
        .data_dir
        .join("users")
        .join(&auth.tenant_id)
        .join("workspace")
        .join("chat_history.log");
    let root_log_path = state
        .config
        .data_dir
        .join("users")
        .join(&auth.tenant_id)
        .join("chat_history.log");

    let content = match tokio::fs::read_to_string(&workspace_log_path).await {
        Ok(c) if !c.trim().is_empty() => c,
        _ => tokio::fs::read_to_string(&root_log_path)
            .await
            .unwrap_or_default(),
    };

    if content.trim().is_empty() {
        return Ok(Json(json!({
            "status": "ok",
            "message": "No recent chat history to consolidate",
            "new_facts": []
        })));
    }

    let lines: Vec<&str> = content.lines().rev().take(40).collect();
    let recent_text = lines.into_iter().rev().collect::<Vec<&str>>().join("\n");

    let existing_facts = db.get_personal_facts(50).unwrap_or_default();
    let existing_block = if existing_facts.is_empty() {
        "(Ninguno todavía)".to_string()
    } else {
        existing_facts
            .iter()
            .map(|f| {
                format!(
                    "- [#{}] [{}] {}",
                    f.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                    f.get("category")
                        .and_then(|v| v.as_str())
                        .unwrap_or("general"),
                    f.get("fact").and_then(|v| v.as_str()).unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let prompt = format!(
        "Analizá los siguientes fragmentos recientes de conversación con el usuario y extraé hechos personales implícitos o explícitos que valga la pena recordar a largo plazo (gustos, horarios, familia, salud, proyectos, preferencias).\n\
        Reglas anti-duplicación:\n\
        1. Revisá la lista de HECHOS YA REGISTRADOS. No repitas hechos que ya existan ni guardes horarios puntuales/recordatorios como hechos.\n\
        2. Si un dato nuevo actualiza o amplía un hecho existente (por ejemplo un cambio de cantidad, estado o trabajo), incluí su \"id\" numérico para reemplazar esa nota en lugar de duplicarla.\n\
        Respondé estrictamente en formato JSON: {{\"summary\": \"...\", \"tags\": [\"...\"], \"facts\": [{{\"id\": null, \"category\": \"preferencias|rutina|salud|trabajo|general\", \"fact\": \"...\"}}]}}\n\n\
        HECHOS YA REGISTRADOS:\n{}\n\n\
        CONVERSATION:\n{}",
        existing_block,
        recent_text
    );

    let mut pcb = PCB::new("ChronosConsolidator".to_string(), 1, prompt);
    pcb.tenant_id = Some(auth.tenant_id.clone());
    pcb.session_key = Some(auth.session_key_hash.clone());

    let mut new_facts = Vec::new();
    let shared_pcb = std::sync::Arc::new(tokio::sync::RwLock::new(pcb));

    if let Ok(mut stream) = state.hal.clone().route_and_execute(shared_pcb, None).await {
        let mut full_resp = String::new();
        while let Some(chunk_res) = stream.next().await {
            if let Ok(chunk) = chunk_res {
                full_resp.push_str(&chunk);
            }
        }

        if let (Some(start), Some(end)) = (full_resp.find('{'), full_resp.rfind('}')) {
            if start < end {
                if let Ok(parsed) =
                    serde_json::from_str::<serde_json::Value>(&full_resp[start..=end])
                {
                    if let Some(facts_arr) = parsed.get("facts").and_then(|v| v.as_array()) {
                        for item in facts_arr {
                            let target_id = item.get("id").and_then(|v| {
                                v.as_i64().or_else(|| {
                                    v.as_str()
                                        .map(|s| s.trim().trim_start_matches('#').trim())
                                        .and_then(|s| s.parse::<i64>().ok())
                                })
                            });
                            let cat = item
                                .get("category")
                                .and_then(|v| v.as_str())
                                .unwrap_or("general");
                            let fact = item.get("fact").and_then(|v| v.as_str()).unwrap_or("");
                            if !fact.trim().is_empty() {
                                match db.upsert_personal_fact(target_id, cat, fact, "chronos") {
                                    Ok(
                                        ank_core::enclave::PersonalFactUpsertOutcome::Inserted {
                                            id,
                                        },
                                    ) => {
                                        new_facts.push(json!({
                                            "id": id,
                                            "category": cat,
                                            "fact": fact,
                                            "action": "inserted",
                                            "source": "chronos"
                                        }));
                                    }
                                    Ok(ank_core::enclave::PersonalFactUpsertOutcome::Updated {
                                        id,
                                        ..
                                    }) => {
                                        new_facts.push(json!({
                                            "id": id,
                                            "category": cat,
                                            "fact": fact,
                                            "action": "updated",
                                            "source": "chronos"
                                        }));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({
        "status": "ok",
        "new_facts": new_facts
    })))
}
