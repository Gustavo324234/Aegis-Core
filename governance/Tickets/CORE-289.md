# CORE-289 — Kernel: herramienta `get_agent_status` para el chat_agent

**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/config/agents/chat_agent.md`
**Estado:** ✅ Completo

---

## Contexto

El chat_agent crea supervisores pero no puede consultar su estado real.
Trabaja completamente ciego — no sabe si el supervisor está Running,
WaitingUser, Complete, o Failed. Ante esa incertidumbre, inventa estados
("está trabajando en segundo plano") o crea supervisores duplicados
pensando que el anterior no existe.

CORE-272 (`get_project_ledger`) resuelve el historial pero no el estado en tiempo real.
Este ticket agrega `get_agent_status` para que el chat_agent pueda consultar
el estado actual del árbol de un proyecto.

---

## Cambios requeridos

### 1. `tool_registry.rs` — herramienta `get_agent_status`

```rust
fn get_agent_status() -> ToolDefinition {
    ToolDefinition {
        name: "get_agent_status",
        description: "Get the current status of all agents working on a project. \
                      Use this before spawning a new supervisor to check if one \
                      already exists. Use when the user asks about project progress.",
        parameters: json!({
            "type": "object",
            "properties": {
                "project_name": {
                    "type": "string",
                    "description": "Name of the project to check. Leave empty to get all active projects."
                }
            },
            "required": []
        }),
    }
}
```

Agregar al arm `AgentRole::ChatAgent`:

```rust
AgentRole::ChatAgent => vec![
    Self::spawn_agent(),
    Self::answer_supervisor(),
    Self::get_project_ledger(),
    Self::get_agent_status(),  // nuevo
],
```

### 2. `chal/mod.rs` — arm `get_agent_status`

```rust
"get_agent_status" => {
    let project_name = args.get("project_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let orchestrator_opt = self.agent_orchestrator.read().await.clone();
    let orchestrator = match orchestrator_opt {
        Some(o) => o,
        None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
    };

    let snapshot = orchestrator.tree_snapshot().await;

    let filtered: Vec<_> = if project_name.is_empty() {
        snapshot
    } else {
        snapshot
            .into_iter()
            .filter(|n| {
                n.project_id.to_lowercase().contains(&project_name.to_lowercase())
                || n.role_label.to_lowercase().contains(&project_name.to_lowercase())
            })
            .collect()
    };

    if filtered.is_empty() {
        return serde_json::json!({
            "status": "no_active_agents",
            "project": project_name,
            "message": "No active agents for this project. You can spawn a new supervisor."
        }).to_string();
    }

    serde_json::to_string(&serde_json::json!({
        "agents": filtered.iter().map(|n| serde_json::json!({
            "role": n.role_label,
            "state": n.state,
            "project": n.project_id,
            "last_report": n.last_report,
            "degraded": n.degraded,
        })).collect::<Vec<_>>()
    })).unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string())
}
```

### 3. `chat_agent.md` — instrucción sobre cuándo usar `get_agent_status`

Agregar sección:

```markdown
## Verificar estado antes de spawner

Antes de crear un supervisor con `spawn_agent`, usá `get_agent_status`
para verificar si ya hay uno activo para ese proyecto.

Ejemplos de cuándo usar `get_agent_status`:
- El usuario pregunta "¿está activo el supervisor?"
- El usuario pregunta "¿cómo va el proyecto X?"
- Antes de hacer spawn — para evitar duplicados
- Cuando no recibiste respuesta del supervisor en el turno anterior

Interpretá el resultado:
- `state: "Running"` o `state: "WaitingReport"` → el supervisor está activo,
  no crear otro
- `state: "WaitingUser"` → el supervisor tiene una pregunta pendiente,
  presentársela al usuario
- `state: "Complete"` → el supervisor terminó, podés crear uno nuevo si hace falta
- `state: "Failed"` → el supervisor falló, podés crear uno nuevo
- `no_active_agents` → no hay supervisor, podés crear uno
```

---

## Criterios de aceptación

- [x] La herramienta `get_agent_status` está disponible para `AgentRole::ChatAgent`
- [x] Sin `project_name` retorna todos los agentes activos de todos los proyectos
- [x] Con `project_name` filtra por nombre de proyecto (case-insensitive, partial match)
- [x] Si no hay agentes activos retorna `{"status":"no_active_agents",...}`
- [x] El resultado incluye `state`, `role`, `project`, y `last_report`
- [x] El chat_agent puede usar este tool para evitar spawns duplicados
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-281 (deduplicación en orchestrator — complementario, no dependiente)
- CORE-286 (timeout — el estado del árbol es más preciso con cleanup activo)
