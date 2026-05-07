# BRIEFING — Sesión 4: Cierre — Reply directo + get_project_ledger + CORE-273 rediseño

**Fecha:** 2026-05-06  
**Para:** Kernel Engineer (Claude Code)  
**Épica:** EPIC 50 — Agent Inbox  
**Tickets:** CORE-271, CORE-272

---

## Prerequisito

**Las Sesiones 2 y 3 deben estar mergeadas a `main` antes de empezar.**

Verificar que existan:
- `AgentOrchestrator::answer_user_question` (CORE-263 ✅)
- `AgentOrchestrator::set_event_channel` + `emit_event` (Sesión 2)
- `ProjectLedger` + `AgentPersistence::save_ledger/load_ledger` (Sesión 1)

Leer antes de implementar:
- `governance/Tickets/CORE-271.md`
- `governance/Tickets/CORE-272.md`

---

## Branch

```
feat/core-271-direct-reply
```

---

## Objetivo

### 1. CORE-271 — Endpoint respuesta directa al supervisor

Expone `answer_user_question` del orchestrator como endpoint HTTP.
El usuario responde desde el hilo dedicado (CORE-270) sin pasar por el chat_agent.

**Archivo:** `kernel/crates/ank-http/src/routes/` (crear `agents.rs` o agregar a routes existentes)

```
POST /api/agents/:agent_id/reply
Headers: x-citadel-tenant, x-citadel-key
Body:    { "answer": "string" }
```

Handler:

```rust
pub async fn reply_to_agent(
    State(state): State<AppState>,
    CitadelAuth(tenant_id): CitadelAuth,
    Path(agent_id): Path<uuid::Uuid>,
    Json(body): Json<AgentReplyBody>,
) -> impl IntoResponse {
    let orchestrator = match state.agent_orchestrator.read().await.clone() {
        Some(o) => o,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    if orchestrator.answer_user_question(agent_id, body.answer.clone()).await {
        // También actualizar UserExchange en el ProjectLedger si existe
        // Buscar el project_id del agente en el árbol y actualizar el exchange pendiente
        update_user_exchange_in_ledger(&state, &orchestrator, agent_id, &body.answer, &tenant_id).await;

        Json(serde_json::json!({ "status": "delivered" })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "no_supervisor_waiting" })),
        ).into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct AgentReplyBody {
    pub answer: String,
}
```

Helper `update_user_exchange_in_ledger`:
- Obtener `project_id` del agente via `orchestrator.tree`
- Cargar el ledger del tenant desde `AgentPersistence`
- Buscar el `UserExchange` con `answer: None` más reciente para ese agente
- Completar con `answer` y `answered_at: Utc::now()`
- Guardar el ledger en disco

Registrar la ruta en el router de Axum con el mismo middleware de autenticación
Citadel que usan los otros endpoints protegidos.

---

### 2. CORE-272 — Herramienta `get_project_ledger` para el chat_agent

**Archivo:** `kernel/crates/ank-core/src/agents/tool_registry.rs`

Agregar al arm `AgentRole::ChatAgent`:

```rust
AgentRole::ChatAgent => vec![
    Self::spawn_agent(),
    Self::answer_supervisor(),
    Self::get_project_ledger(),   // nuevo
],
```

Definición:

```rust
fn get_project_ledger() -> ToolDefinition {
    ToolDefinition {
        name: "get_project_ledger",
        description: "Get the history of a project: entries recorded by supervisors and exchanges with the user. Use ONLY when the user explicitly asks about a project's progress, decisions, or what was discussed with supervisors. Do not use proactively.",
        parameters: json!({
            "type": "object",
            "properties": {
                "project_name": {
                    "type": "string",
                    "description": "Name of the project to query."
                }
            },
            "required": ["project_name"]
        }),
    }
}
```

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

Arm `get_project_ledger`:

```rust
"get_project_ledger" => {
    let project_name = args["project_name"].as_str().unwrap_or("").to_string();

    let orchestrator_opt = self.agent_orchestrator.read().await.clone();
    let orchestrator = match orchestrator_opt {
        Some(o) => o,
        None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
    };

    // Buscar project_id por nombre (case-insensitive, partial match)
    let project_id = {
        let tree = orchestrator.tree.read().await;
        tree.all_nodes()
            .iter()
            .filter_map(|n| {
                if let AgentRole::ProjectSupervisor { name, .. } = &n.role {
                    if name.to_lowercase().contains(&project_name.to_lowercase()) {
                        return Some(n.project_id.clone());
                    }
                }
                None
            })
            .next()
    };

    let project_id = match project_id {
        Some(id) => id,
        None => return format!(
            "{{\"error\":\"project_not_found\",\"project\":\"{}\"}}",
            project_name
        ),
    };

    let tenant_id = match &pcb.tenant_id {
        Some(t) => t.clone(),
        None => return "{\"error\":\"no_tenant_id\"}".to_string(),
    };

    let persistence = ank_core::agents::persistence::AgentPersistence::from_env();
    match persistence.load_ledger(&tenant_id, &project_id) {
        Ok(Some(ledger)) => serde_json::to_string(&ledger)
            .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string()),
        Ok(None) => format!(
            "{{\"error\":\"no_ledger\",\"project_id\":\"{}\"}}",
            project_id
        ),
        Err(e) => format!("{{\"error\":\"load_failed\",\"detail\":\"{}\"}}", e),
    }
}
```

**Archivo:** `kernel/config/agents/chat_agent.md`

Agregar al final:

```markdown
## Consulta de estado de proyectos

Usá `get_project_ledger` SOLO cuando el usuario pregunte explícitamente
sobre el estado, historial, o decisiones de un proyecto.

Ejemplos de cuándo usarlo:
- "¿qué avanzamos en el proyecto X?"
- "¿qué le respondí al supervisor?"
- "¿qué se decidió sobre Y?"

No lo uses proactivamente. Al presentar el resultado, resumí en lenguaje
natural — no vuelques el JSON crudo al usuario.
```

---

## Verificación

```bash
cargo build --workspace
```

Sin errores ni warnings nuevos.

---

## Commit y PR

**Commit message:**
```
feat(ank-core,ank-http): CORE-271/272 direct reply endpoint, get_project_ledger tool
```

**PR title:**
```
feat(ank-core,ank-http): CORE-271/272 — respuesta directa a supervisor + get_project_ledger
```

**PR description:**
```
## Cambios

### CORE-271 — Endpoint respuesta directa
- `POST /api/agents/:agent_id/reply` protegido con autenticación Citadel
- Entrega la respuesta al supervisor via `answer_user_question`
- Actualiza el `UserExchange` pendiente en el ProjectLedger con answer + answered_at
- 200 si había supervisor esperando, 404 si no

### CORE-272 — get_project_ledger para chat_agent
- Herramienta `get_project_ledger` en ToolRegistry para ChatAgent
- Arm en `execute_tool_call_internal` lee del ProjectLedger en disco
- Busca proyecto por nombre (case-insensitive, partial match)
- Instrucción en `chat_agent.md`: usar solo cuando el usuario lo pide explícitamente

## Verificación
`cargo build --workspace` ✅

## Cierre del EPIC 50
Con este PR mergeado, el EPIC 50 está completo en la parte Kernel.
La parte Shell (Sesión 3) cubre la UI.
```

**Target branch:** `main`

---

## Estado del EPIC 50 al completar las 4 sesiones

```
Sesión 1 ✅  CORE-275 + CORE-273 + CORE-274 (Rust)
Sesión 2 ✅  CORE-276 + CORE-277 + CORE-268
Sesión 3 ✅  CORE-269 + CORE-274 (Shell) + CORE-270
Sesión 4 ✅  CORE-271 + CORE-272
```

EPIC 50 cerrado. Los specialists pueden trabajar. Los supervisores tienen
presencia en la UI. El ProjectLedger persiste el avance del proyecto.

---

*Briefing creado por Arquitecto IA — 2026-05-06*
