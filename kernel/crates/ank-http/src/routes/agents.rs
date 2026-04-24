use crate::{
    citadel::{CitadelAuthenticated, CitadelCredentials},
    state::AppState,
};
use ank_core::agents::node::{AgentRole, AgentState};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tree", get(get_agent_tree))
        .route("/:agent_id", get(get_agent))
        .route("/spawn", post(spawn_agent))
}

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentNodeDto {
    pub agent_id: String,
    pub role: String,
    pub project_id: String,
    pub domain: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub state: String,
    pub task_type: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct AgentTreeDto {
    pub nodes: Vec<AgentNodeDto>,
    pub roots: Vec<String>,
    pub total_agents: usize,
}

#[derive(Deserialize)]
pub struct SpawnAgentRequest {
    pub project_id: String,
    pub role: String,
    pub domain: String,
    pub parent_id: String,
    pub system_prompt: String,
    pub task_type: String,
}

#[derive(Serialize)]
pub struct SpawnAgentResponse {
    pub agent_id: String,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /api/agents/tree — retorna el árbol completo de agentes activos.
/// Requiere: x-citadel-tenant + x-citadel-key válidos.
async fn get_agent_tree(
    State(state): State<AppState>,
    _creds: CitadelCredentials,
) -> Json<AgentTreeDto> {
    let snapshot = state.agent_orchestrator.tree_snapshot().await;

    let mut nodes: Vec<AgentNodeDto> = Vec::new();
    let mut roots: Vec<String> = Vec::new();

    for root in snapshot.all_roots() {
        roots.push(root.agent_id.to_string());
        collect_nodes(&snapshot, root.agent_id, &mut nodes);
    }

    let total = nodes.len();
    Json(AgentTreeDto {
        nodes,
        roots,
        total_agents: total,
    })
}

fn collect_nodes(
    tree: &ank_core::agents::tree::AgentTree,
    id: uuid::Uuid,
    out: &mut Vec<AgentNodeDto>,
) {
    let Some(node) = tree.get(&id) else { return };

    out.push(AgentNodeDto {
        agent_id: node.agent_id.to_string(),
        role: format!("{:?}", node.role),
        project_id: node.project_id.clone(),
        domain: node.domain.clone(),
        parent_id: node.parent_id.map(|p| p.to_string()),
        children: node.children.iter().map(|c| c.to_string()).collect(),
        state: agent_state_label(&node.state),
        task_type: format!("{:?}", node.task_type),
        created_at: node.created_at.to_rfc3339(),
    });

    for child_id in node.children.clone() {
        collect_nodes(tree, child_id, out);
    }
}

fn agent_state_label(state: &AgentState) -> String {
    match state {
        AgentState::Idle => "Idle".to_string(),
        AgentState::Running => "Running".to_string(),
        AgentState::WaitingReport => "WaitingReport".to_string(),
        AgentState::Complete => "Complete".to_string(),
        AgentState::Failed { reason } => format!("Failed({})", reason),
    }
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
    let node = snapshot.get(&id).ok_or(axum::http::StatusCode::NOT_FOUND)?;

    Ok(Json(AgentNodeDto {
        agent_id: node.agent_id.to_string(),
        role: format!("{:?}", node.role),
        project_id: node.project_id.clone(),
        domain: node.domain.clone(),
        parent_id: node.parent_id.map(|p| p.to_string()),
        children: node.children.iter().map(|c| c.to_string()).collect(),
        state: agent_state_label(&node.state),
        task_type: format!("{:?}", node.task_type),
        created_at: node.created_at.to_rfc3339(),
    }))
}

/// POST /api/agents/spawn — requiere autenticación válida (admin en producción).
/// Permite spawnear manualmente un agente para debugging.
async fn spawn_agent(
    State(state): State<AppState>,
    _auth: CitadelAuthenticated,
    Json(body): Json<SpawnAgentRequest>,
) -> Result<Json<SpawnAgentResponse>, axum::http::StatusCode> {
    let parent_id = body
        .parent_id
        .parse::<uuid::Uuid>()
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    let role = match body.role.to_uppercase().as_str() {
        "PROJECT_SUPERVISOR" | "PROJECTSUPERVISOR" => AgentRole::ProjectSupervisor,
        "DOMAIN_SUPERVISOR" | "DOMAINSUPERVISOR" => AgentRole::DomainSupervisor,
        _ => AgentRole::Specialist,
    };

    let task_type = match body.task_type.to_uppercase().as_str() {
        "CODE" | "CODING" => ank_core::pcb::TaskType::Coding,
        "PLANNING" => ank_core::pcb::TaskType::Planning,
        "ANALYSIS" => ank_core::pcb::TaskType::Analysis,
        _ => ank_core::pcb::TaskType::Chat,
    };

    let new_id = state
        .agent_orchestrator
        .spawn_agent(
            role,
            body.project_id,
            body.domain,
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
