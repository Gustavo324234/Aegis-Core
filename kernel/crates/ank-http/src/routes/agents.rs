use crate::{
    citadel::{CitadelAuthenticated, CitadelCredentials},
    state::AppState,
};
use ank_core::agents::node::AgentRole;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects))
        .route("/tree", get(get_agent_tree))
        .route("/:agent_id", get(get_agent))
        .route("/spawn", post(spawn_agent))
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
