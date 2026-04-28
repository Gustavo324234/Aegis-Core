# BRIEFING — Kernel Engineer — CORE-209 + CORE-210
**Fecha:** 2026-04-28  
**Rama:** `fix/core-209-210-agents-routing`  
**PR title:** `fix(ank-http): CORE-209 CORE-210 montar ws/agents, agregar /api/agents/projects y fix chat_agent fallback`

---

## Contexto

Dos bugs detectados en sesión de QA el 2026-04-28. Todo el código subyacente ya existe — son fixes quirúrgicos de routing e instrucciones.

---

## TICKET 1 — CORE-209

### Problema
`routes/mod.rs::build_router` no monta `/ws/agents`. El WebSocket existe completo en `ws/agents.rs` pero nunca se registra en el router principal. El `ws::build_router` en `ws/mod.rs` es un router secundario que nadie llama.

Además, `GET /api/agents/projects` no existe en `routes/agents.rs` aunque `ProjectRegistry::list_active()` ya está implementado en `ank-core`.

### Fix 1 — `kernel/crates/ank-http/src/routes/mod.rs`

Leer el archivo. Buscar el bloque `// WebSocket Routes` y agregar la línea faltante:

```rust
// WebSocket Routes
.nest("/ws/chat", ws::chat::router())
.nest("/ws/siren", ws::siren::router())
.nest("/ws/agents", ws::agents::router())   // ← AGREGAR
```

### Fix 2 — `kernel/crates/ank-http/src/routes/agents.rs`

Leer el archivo completo. Agregar:

**En `router()`:**
```rust
.route("/projects", get(list_projects))
```
Agregar **antes** de `/tree` para que no colisione con `/:agent_id`.

**DTOs nuevos:**
```rust
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
```

**Handler:**

El `AppState` (leer `kernel/crates/ank-http/src/state.rs`) tiene `agent_orchestrator: Arc<AgentOrchestrator>` pero **no tiene `project_registry`** como campo directo.

El `ProjectRegistry` vive dentro del `Citadel` (`state.citadel`). Leer `ank-core/src/citadel/` para ver cómo acceder a él, o bien alternativamente usar solo `agent_orchestrator.tree_snapshot()` para construir la lista de proyectos a partir del árbol activo.

**Implementación recomendada — solo usando `agent_orchestrator`** (no requiere tocar Citadel):

```rust
async fn list_projects(
    State(state): State<AppState>,
    _creds: CitadelCredentials,
) -> Json<ProjectListDto> {
    let snapshot = state.agent_orchestrator.tree_snapshot().await;

    // Agrupar nodos por project_id, buscar root (parent_id == None) de cada proyecto
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
```

Si preferís acceder al `ProjectRegistry` real (para obtener `name` y `description` correctos), buscar cómo está expuesto en `AppState` o en `Citadel` y ajustar. Cualquiera de las dos implementaciones es válida para este ticket.

---

## TICKET 2 — CORE-210

### Problema
El Chat Agent inventa datos del proyecto cuando no hay ProjectSupervisor activo. El modelo `openrouter/free` responde con descripción falsa del repositorio.

### Fix 1 — `kernel/config/agents/chat_agent.md`

Leer el archivo. Agregar esta sección **antes** de `## Restricciones absolutas`:

```markdown
## Cuando no tenés información del proyecto

Si el usuario pregunta por el estado de un proyecto y no tenés un QueryReply real
de un ProjectSupervisor activo, **no inventés datos del proyecto**.

Respuestas correctas en ese caso:
✓ "Todavía no tengo un equipo activo para ese proyecto. ¿Querés que arranquemos?"
✓ "No tengo información actualizada sobre ese proyecto en este momento."

Nunca describas estructura de archivos, conteo de archivos, tecnologías usadas
ni ningún detalle técnico del proyecto a menos que lo hayas recibido en un QueryReply.
```

### Fix 2 — inyección de contexto en el prompt del Chat Agent

Leer `kernel/crates/ank-core/src/chal/mod.rs`, específicamente `build_prompt`.

En la función `build_prompt` de `CognitiveHAL`, antes de armar el `final_prompt`,
agregar una sección de contexto de agentes si el VCM lo permite.

**El acceso al orchestrator desde `build_prompt` es problemático** porque `CognitiveHAL` no tiene referencia directa a `AgentOrchestrator`. Dos opciones:

**Opción A (preferida — no modifica `CognitiveHAL`):** Inyectar el contexto en `l1_instruction` del PCB antes de llamar a `route_and_execute`. Buscar dónde se construye el PCB para el Chat Agent (probablemente en `ank-http/src/ws/chat.rs` o en el executor del scheduler) y agregar al inicio de la instrucción:

```
[SISTEMA: agents_available=false, active_projects=ninguno]
```
(o `true` y la lista de proyectos si el snapshot no está vacío).

**Opción B (más simple para el fix inmediato):** Solo aplicar el Fix 1 de `chat_agent.md`. El fix del prompt de fallback en las instrucciones ya mejora significativamente el comportamiento sin requerir cambios en Rust.

Implementar al menos la Opción B. Si el tiempo lo permite, sumar la Opción A.

---

## Verificación

```
cargo build
```

Sin `cargo test` — los tests corren en CI tras el push.

---

## Branch y commit

```
# Crear rama
git checkout -b fix/core-209-210-agents-routing

# Commits (uno por fix es suficiente)
git commit -m "fix(ank-http): CORE-209 montar ws/agents y agregar GET /api/agents/projects"
git commit -m "fix(chat_agent): CORE-210 fallback cuando no hay proyecto activo"

# Push — Tavo hace el PR y merge manualmente
git push origin fix/core-209-210-agents-routing
```

---

*Arquitecto IA — 2026-04-28*
