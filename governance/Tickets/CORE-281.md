# CORE-281 — Kernel: deduplicación de supervisores + project_name en system prompt

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/config/agents/project_supervisor.md`
**Estado:** ✅ Completo

---

## Contexto

Dos bugs relacionados identificados en logs de producción:

**Bug 1 — Múltiples supervisores del mismo proyecto**
Cada vez que el chat_agent no recibe confirmación de que el supervisor está activo,
crea uno nuevo. En una sola sesión se registraron 3 `ProjectSupervisor created`
para Aegis-Core en menos de 15 minutos. Ninguno reportó — todos compiten por el
mismo contexto y se bloquean mutuamente.

**Bug 2 — El supervisor no sabe su propio nombre de proyecto**
El `ProjectSupervisor` recibe su nombre en el campo `project` del struct, y aparece
en el log de `CORE-264`. Pero el dispatch que llega al LLM es texto libre redactado
por el chat_agent — a veces incluye el nombre, a veces no. Resultado: el supervisor
pregunta "¿Qué proyecto querés reactivar?" cuando ya tiene el nombre en su metadata.

---

## Cambios requeridos

### 1. Deduplicación en `spawn_agent` arm de `execute_tool_call_internal`

Antes de crear un nuevo `ProjectSupervisor`, verificar si ya existe uno activo
para ese `project_id` en el árbol:

```rust
// En el arm spawn_agent, cuando role == "project_supervisor":
"project_supervisor" => {
    let project_name = args["name"].as_str().unwrap_or("").to_string();

    // CORE-281: Verificar si ya existe un ProjectSupervisor activo para este proyecto
    let existing_agent_id = {
        let tree = orchestrator.tree.read().await;
        tree.all_nodes()
            .iter()
            .find(|n| {
                matches!(&n.role, AgentRole::ProjectSupervisor { name, .. }
                    if name.to_lowercase() == project_name.to_lowercase())
                && !matches!(n.state, AgentState::Complete | AgentState::Failed { .. })
            })
            .map(|n| n.id)
    };

    if let Some(existing_id) = existing_agent_id {
        // Ya existe — retornar su ID sin crear uno nuevo
        return serde_json::json!({
            "status": "already_active",
            "agent_id": existing_id.to_string(),
            "message": format!("ProjectSupervisor for '{}' is already active.", project_name)
        }).to_string();
    }

    // No existe — crear normalmente
    // ... código existente de spawn ...
}
```

### 2. Inyectar `project_name` en el system prompt del ProjectSupervisor

En `orchestrator.rs`, en `run_agent_loop` cuando el rol es `ProjectSupervisor`,
agregar un bloque estructurado al inicio del system prompt:

```rust
// Al construir los messages para el LLM del ProjectSupervisor:
let project_header = format!(
    "[PROJECT]\nName: {}\nProject ID: {}\nYour role: ProjectSupervisor\n\
     You are responsible for this specific project. Do not ask the user \
     which project to work on — you already know it is '{}'.\n\n",
    project_name,
    project_id,
    project_name
);

// Prepend al system prompt existente
let full_system = format!("{}{}", project_header, base_system_prompt);
```

### 3. `project_supervisor.md` — instrucción explícita

Agregar al inicio del archivo `kernel/config/agents/project_supervisor.md`:

```markdown
## Tu identidad de proyecto

Tu nombre de proyecto está en el header `[PROJECT]` de tu contexto.
NUNCA preguntes al usuario qué proyecto activar o cuál es tu nombre —
ya lo tenés en el contexto. Comenzá tu trabajo directamente.
```

---

## Criterios de aceptación

- [x] Si ya existe un `ProjectSupervisor` activo para un proyecto, `spawn_agent` retorna
  `{"status":"already_active","agent_id":"..."}` sin crear uno nuevo
- [x] El header `[PROJECT]` aparece al inicio del system prompt de cada `ProjectSupervisor`
  con el nombre y project_id correctos
- [x] El supervisor nunca pregunta "¿Qué proyecto querés reactivar?" cuando el nombre
  ya está en su contexto
- [x] Supervisores en estado `Complete` o `Failed` no cuentan como "activos" — 
  se puede crear uno nuevo si el anterior terminó
- [x] `cargo build --workspace` sin errores

## Dependencias

- Ninguna — bug fix aislado
