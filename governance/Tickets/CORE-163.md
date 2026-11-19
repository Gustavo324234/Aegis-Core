# CORE-163 — feat: HTTP Routes /api/agents/* — Árbol de Agentes y Estado

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-http  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Media  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-158

---

## Contexto

La Shell necesita poder consultar el estado del árbol de agentes en tiempo real para mostrarlo al usuario (componente `AgentTreeView`, ticket CORE-164). Este ticket expone endpoints HTTP en `ank-http` para ese propósito.

Las rutas son **de solo lectura** para clientes normales. Solo el admin puede spawnear agentes manualmente (para debugging).

---

## Cambios Requeridos

### Nuevas rutas en `ank-http/src/routes/`

Crear `ank-http/src/routes/agents.rs`:

```rust
// GET /api/agents/tree
// Retorna el árbol completo de agentes activos como JSON.
// Requiere: Citadel auth (session_key)
async fn get_agent_tree(state: State<AppState>, ...) -> impl IntoResponse {
    let tree = state.scheduler.agent_orchestrator.tree_snapshot().await;
    Json(tree_to_dto(tree))
}

// GET /api/agents/{agent_id}
// Retorna el estado de un agente específico.
// Requiere: Citadel auth
async fn get_agent(
    state: State<AppState>,
    Path(agent_id): Path<String>,
    ...
) -> impl IntoResponse { ... }

// POST /api/agents/spawn  (solo Master Admin)
// Permite spawnear manualmente un agente para debugging.
// Body: { project_id, role, domain, parent_id, system_prompt, task_type }
async fn spawn_agent(
    state: State<AppState>,
    Json(body): Json<SpawnAgentRequest>,
    ...
) -> impl IntoResponse { ... }
```

### DTOs de respuesta

```rust
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
```

### Registrar las rutas

En `ank-http/src/lib.rs` o donde se construye el router Axum, agregar las nuevas rutas al árbol de routes existente bajo el prefijo `/api/agents`.

---

## Criterios de Aceptación

- [ ] `GET /api/agents/tree` retorna `200` con el árbol serializado como JSON
- [ ] `GET /api/agents/tree` retorna `401` si no hay `session_key` válida en el header
- [ ] `GET /api/agents/{id}` retorna `404` para IDs inexistentes
- [ ] `POST /api/agents/spawn` retorna `403` para tenants no-admin
- [ ] Los DTOs no exponen datos internos sensibles (no hay system_prompt en la respuesta pública)
- [ ] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Rutas /api/agents/* montadas en routes/mod.rs:57; tree/spawn/projects en routes/agents.rs.
