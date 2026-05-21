use crate::{
    citadel::{CitadelAuthenticated, CitadelCredentials},
    state::AppState,
};
use ank_core::agents::{node::AgentRole, persistence::AgentPersistence};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects))
        .route("/projects/autonomous", get(list_autonomous_projects))
        .route(
            "/projects/:project_id/autonomous",
            post(set_project_autonomous),
        )
        .route("/tree", get(get_agent_tree))
        .route("/:agent_id", get(get_agent))
        .route("/spawn", post(spawn_agent))
        .route("/:agent_id/reply", post(reply_to_agent))
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProjectSummaryDto {
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub root_agent_id: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectListDto {
    pub projects: Vec<ProjectSummaryDto>,
}

#[derive(Serialize)]
pub struct AgentNodeDto {
    pub agent_id: String,
    pub role: String,
    pub project_id: String,
    pub parent_id: Option<String>,
    pub state: String,
    pub model: String,
    pub task_type: String,
    pub is_restored: bool,
    pub last_report: Option<String>,
}

#[derive(Serialize)]
pub struct AgentTreeDto {
    pub nodes: Vec<AgentNodeDto>,
    pub total_agents: usize,
}

#[derive(Deserialize)]
pub struct SpawnAgentRequest {
    pub project_id: String,
    pub role: String,
    /// Optional: name/scope for supervisor or specialist roles.
    pub name: Option<String>,
    pub parent_id: String,
    pub system_prompt: Option<String>,
    pub task_type: String,
}

#[derive(Serialize)]
pub struct SpawnAgentResponse {
    pub agent_id: String,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/agents/projects — retorna los proyectos activos derivados del árbol de agentes.
async fn list_projects(
    State(state): State<AppState>,
    _creds: CitadelCredentials,
) -> Json<ProjectListDto> {
    let snapshot = state.agent_orchestrator.tree_snapshot().await;

    let mut projects_map: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();

    for node in &snapshot {
        let root_id = if node.parent_id.is_none() {
            Some(node.agent_id.to_string())
        } else {
            None
        };
        projects_map
            .entry(node.project_id.clone())
            .and_modify(|r| {
                if root_id.is_some() {
                    *r = root_id.clone();
                }
            })
            .or_insert(root_id);
    }

    let projects = projects_map
        .into_iter()
        .map(|(project_id, root_agent_id)| ProjectSummaryDto {
            name: project_id.clone(),
            project_id,
            description: None,
            status: "active".to_string(),
            root_agent_id,
        })
        .collect();

    Json(ProjectListDto { projects })
}

/// GET /api/agents/tree — retorna el árbol completo de agentes activos.
async fn get_agent_tree(
    State(state): State<AppState>,
    _creds: CitadelCredentials,
) -> Json<AgentTreeDto> {
    let snapshot = state.agent_orchestrator.tree_snapshot().await;
    let total = snapshot.len();
    let nodes = snapshot.into_iter().map(summary_to_dto).collect();
    Json(AgentTreeDto {
        nodes,
        total_agents: total,
    })
}

/// GET /api/agents/{agent_id} — retorna el estado de un agente específico.
async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id_str): Path<String>,
    _creds: CitadelCredentials,
) -> Result<Json<AgentNodeDto>, axum::http::StatusCode> {
    let id = agent_id_str
        .parse::<uuid::Uuid>()
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    let snapshot = state.agent_orchestrator.tree_snapshot().await;
    let summary = snapshot
        .into_iter()
        .find(|n| n.agent_id == id)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    Ok(Json(summary_to_dto(summary)))
}

/// POST /api/agents/spawn — permite spawnear manualmente un agente para debugging.
async fn spawn_agent(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(body): Json<SpawnAgentRequest>,
) -> Result<Json<SpawnAgentResponse>, axum::http::StatusCode> {
    let parent_id = body
        .parent_id
        .parse::<uuid::Uuid>()
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    let name = body.name.unwrap_or_default();

    let role = match body.role.to_uppercase().as_str() {
        "PROJECT_SUPERVISOR" | "PROJECTSUPERVISOR" => AgentRole::ProjectSupervisor {
            name: name.clone(),
            description: String::new(),
        },
        "SUPERVISOR" => AgentRole::Supervisor {
            name: name.clone(),
            scope: name.clone(),
        },
        _ => AgentRole::Specialist { scope: name },
    };

    let task_type = match body.task_type.to_uppercase().as_str() {
        "CODE" | "CODING" => ank_core::pcb::TaskType::Code,
        "PLANNING" => ank_core::pcb::TaskType::Planning,
        "ANALYSIS" => ank_core::pcb::TaskType::Analysis,
        "CREATIVE" => ank_core::pcb::TaskType::Creative,
        _ => ank_core::pcb::TaskType::Chat,
    };

    let new_id = state
        .agent_orchestrator
        .spawn_agent(
            role,
            body.project_id,
            parent_id,
            body.system_prompt,
            task_type,
        )
        .await
        .map_err(|_| axum::http::StatusCode::UNPROCESSABLE_ENTITY)?;

    Ok(Json(SpawnAgentResponse {
        agent_id: new_id.to_string(),
    }))
}

// ── CORE-271: Respuesta directa al supervisor ────────────────────────────────

#[derive(Deserialize)]
pub struct AgentReplyBody {
    pub answer: String,
}

/// POST /api/agents/:agent_id/reply — entrega la respuesta del usuario al supervisor pausado.
async fn reply_to_agent(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Path(agent_id_str): Path<String>,
    Json(body): Json<AgentReplyBody>,
) -> impl IntoResponse {
    let agent_id = match agent_id_str.parse::<uuid::Uuid>() {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid_agent_id" })),
            )
                .into_response()
        }
    };

    if state
        .agent_orchestrator
        .answer_user_question(agent_id, body.answer.clone())
        .await
    {
        update_user_exchange_in_ledger(&state, agent_id, &body.answer, &auth.tenant_id).await;
        Json(serde_json::json!({ "status": "delivered" })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "no_supervisor_waiting" })),
        )
            .into_response()
    }
}

async fn update_user_exchange_in_ledger(
    state: &AppState,
    agent_id: uuid::Uuid,
    answer: &str,
    tenant_id: &str,
) {
    let project_id = {
        let tree = state.agent_orchestrator.tree.read().await;
        match tree.get(&agent_id) {
            Some(node) => node.project_id.clone(),
            None => return,
        }
    };

    let persistence = AgentPersistence::from_env();
    let mut ledger = match persistence.load_ledger(tenant_id, &project_id) {
        Ok(Some(l)) => l,
        _ => return,
    };

    if let Some(exchange) = ledger
        .user_exchanges
        .iter_mut()
        .rev()
        .find(|e| e.answer.is_none())
    {
        exchange.answer = Some(answer.to_string());
        exchange.answered_at = Some(chrono::Utc::now());
    }

    let _ = persistence.save_ledger(tenant_id, &project_id, &ledger);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn summary_to_dto(s: ank_core::agents::orchestrator::AgentNodeSummary) -> AgentNodeDto {
    AgentNodeDto {
        agent_id: s.agent_id.to_string(),
        role: s.role_label,
        project_id: s.project_id,
        parent_id: s.parent_id.map(|p| p.to_string()),
        state: s.state,
        model: s.model,
        task_type: s.task_type,
        is_restored: s.is_restored,
        last_report: s.last_report,
    }
}

#[derive(Deserialize)]
struct SetAutonomousRequest {
    enabled: bool,
}

/// GET /api/agents/projects/autonomous — list the project IDs the tenant has put
/// in autonomous mode (specialists skip the external-path approval gate).
async fn list_autonomous_projects(
    State(_state): State<AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let projects = db.get_autonomous_projects().unwrap_or_default();
    Ok(Json(serde_json::json!({ "autonomous_projects": projects })))
}

/// POST /api/agents/projects/{project_id}/autonomous — enable/disable autonomous
/// mode for a project. In autonomous mode the project's specialists get full
/// filesystem access without per-path approval prompts.
async fn set_project_autonomous(
    State(_state): State<AppState>,
    Path(project_id): Path<String>,
    auth: CitadelAuthenticated,
    Json(body): Json<SetAutonomousRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = ank_core::enclave::TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.set_project_autonomous(&project_id, body.enabled)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "project_id": project_id,
        "autonomous": body.enabled,
    })))
}
