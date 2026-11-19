# CORE-209 — fix(ank-http): montar /ws/agents y agregar GET /api/agents/projects en router

**Epic:** 45 — Cognitive Agent Architecture  
**Repo:** Aegis-Core / kernel/crates/ank-http  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna — todo el código subyacente ya existe

---

## Contexto

El Dashboard falla al abrirse porque dos rutas nunca fueron montadas en `build_router`:

**Bug 1 — WebSocket `/ws/agents/{tenant_id}` no está registrado**

`ws/agents.rs` existe y está completo (CORE-200). `ws/mod.rs` tiene su `pub mod agents` y lo monta
en un `build_router` secundario. Sin embargo `routes/mod.rs::build_router` — que es el único
router que realmente se usa — **no incluye** `.nest("/ws/agents", ws::agents::router())`.

El `ws::build_router` en `ws/mod.rs` nunca es llamado desde ningún punto del código.
Resultado: la UI llama a `connectAgentStream()` → WebSocket 404 → Dashboard sin datos de agentes.

**Bug 2 — GET `/api/agents/projects` no existe**

`useAegisStore.fetchActiveProjects()` llama a `GET /api/agents/projects`.
`agents.rs` solo tiene `/tree`, `/:agent_id` y `/spawn`. No hay `/projects`.
El `ProjectRegistry` ya está implementado en `ank-core` con `list_active()`.
Falta únicamente la ruta HTTP que lo exponga.

---

## Cambios requeridos

### 1. `kernel/crates/ank-http/src/routes/mod.rs`

Agregar la ruta WebSocket de agentes al `build_router` principal.

Buscar el bloque de WebSocket Routes y agregar la línea faltante:

```rust
// WebSocket Routes
.nest("/ws/chat", ws::chat::router())
.nest("/ws/siren", ws::siren::router())
.nest("/ws/agents", ws::agents::router())   // ← AGREGAR ESTA LÍNEA
```

### 2. `kernel/crates/ank-http/src/routes/agents.rs`

Agregar ruta y handler para `GET /projects`.

En la función `router()`, agregar:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects))   // ← AGREGAR
        .route("/tree", get(get_agent_tree))
        .route("/:agent_id", get(get_agent))
        .route("/spawn", post(spawn_agent))
}
```

Agregar el DTO de respuesta:

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

Agregar el handler. Lee `state.project_registry` (del `AppState`) para listar proyectos activos
y busca el root agent de cada proyecto en el `AgentOrchestrator`:

```rust
async fn list_projects(
    State(state): State<AppState>,
    creds: CitadelCredentials,
) -> Json<ProjectListDto> {
    let registry = state.project_registry
        .get(&creds.tenant_id)
        .read()
        .await;

    let active = registry.list_active();
    let tree_snapshot = state.agent_orchestrator.tree_snapshot().await;

    let projects = active.iter().map(|meta| {
        // Buscar el root agent de este proyecto en el snapshot
        let root_agent_id = tree_snapshot.iter()
            .find(|n| n.project_id == meta.project_id && n.parent_id.is_none())
            .map(|n| n.agent_id.to_string());

        ProjectSummaryDto {
            project_id: meta.project_id.clone(),
            name: meta.name.clone(),
            description: meta.description.clone(),
            status: meta.status.to_string(),
            root_agent_id,
        }
    }).collect();

    Json(ProjectListDto { projects })
}
```

> **Nota al Kernel Engineer:** Verificar la API real de `AppState` para acceder al
> `project_registry` — puede estar en `state.enclave` o como campo directo. Leer
> `kernel/crates/ank-http/src/state.rs` antes de implementar y ajustar el acceso según
> lo que existe. El patrón exacto puede diferir del pseudocódigo de arriba.

---

## Criterios de aceptación

- [ ] `GET /api/agents/projects` retorna `{ "projects": [] }` cuando no hay proyectos activos (HTTP 200)
- [ ] `GET /api/agents/projects` retorna la lista correcta cuando hay proyectos creados
- [ ] `ws://host/ws/agents/{tenant_id}` acepta la conexión WebSocket y envía un `TreeSnapshot` inicial
- [ ] `cargo build` sin errores

---

## Commit message

```
fix(ank-http): CORE-209 montar ws/agents y agregar GET /api/agents/projects
```

---

*Arquitecto IA — 2026-04-28*

---

## Resolución (2026-06-12)
Verificado 2026-06-12: ya implementado en main — routes/mod.rs monta /ws/agents (línea 62) y /api/agents (línea 57); GET /api/agents/projects existe en routes/agents.rs:82. Reconciliación de governance.
