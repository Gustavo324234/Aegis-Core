# BRIEFING — Kernel Engineer
**Fecha:** 2026-05-05  
**Ticket:** CORE-264  
**Prioridad:** Crítica  
**Branch:** `fix/core-264-dispatch-post-spawn`

---

## Problema

El supervisor se crea correctamente cuando el Chat Agent llama `spawn_agent`, pero **nunca recibe trabajo**. Su loop tokio queda idle esperando un `Dispatch` que nadie envía. Resultado en producción: el Chat Agent inventa respuestas porque no tiene información real del supervisor.

Evidencia en logs de hoy (2026-05-05):
```
[PROJECT] ProjectSupervisor created. agent=1cb87f56-e4c1-423e-980a-d10f963a20e1
ReAct: tool ejecutado tool=spawn_agent
ProcessCompleted → output: "¡Entendido, Tavo!..." ← respuesta inventada
```

---

## Archivos a modificar

### 1. `kernel/crates/ank-core/src/agents/orchestrator.rs`

Verificar que `dispatch()` sea `pub`. Actualmente debería serlo, pero confirmar.

---

### 2. `kernel/crates/ank-core/src/chal/mod.rs`

En `execute_tool_call_internal`, arm `"spawn_agent"`, el bloque `Ok(_)` actual descarta el `AgentId`. Reemplazarlo:

**Antes:**
```rust
match orchestrator
    .create_project(project_name.clone(), scope, task_type, pcb.tenant_id.clone())
    .await
{
    Ok(_) => format!(
        "{{\"status\":\"spawned\",\"project\":\"{}\"}}",
        project_name
    ),
    Err(e) => format!("{{\"error\":\"{}\"}}", e),
}
```

**Después:**
```rust
match orchestrator
    .create_project(project_name.clone(), scope, task_type, pcb.tenant_id.clone())
    .await
{
    Ok(agent_id) => {
        let task = pcb.memory_pointers.l1_instruction.clone();
        if !task.is_empty() {
            if let Err(e) = orchestrator.dispatch(agent_id, task, vec![]).await {
                tracing::warn!(
                    agent = %agent_id,
                    "CORE-264: dispatch post-spawn falló: {}",
                    e
                );
            } else {
                tracing::info!(
                    agent = %agent_id,
                    project = %project_name,
                    "CORE-264: Dispatch automático enviado al supervisor recién creado."
                );
            }
        }
        format!(
            "{{\"status\":\"spawned\",\"project\":\"{}\",\"agent_id\":\"{}\"}}",
            project_name,
            agent_id
        )
    }
    Err(e) => format!("{{\"error\":\"{}\"}}", e),
}
```

---

### 3. `kernel/config/agents/chat_agent.md`

Agregar al final del archivo (después de la sección "Comunicación con Supervisores" existente):

```markdown

## Gestión de Proyectos con Supervisores

Cuando el usuario pida trabajar en un proyecto, usás `spawn_agent` para crear un supervisor. El sistema automáticamente envía la tarea al supervisor y éste trabaja en segundo plano.

**Flujo esperado:**
1. Creás el supervisor con `spawn_agent` — el sistema le despacha la tarea automáticamente.
2. Informás al usuario que el supervisor está trabajando en su pedido.
3. Si el supervisor necesita información del usuario, te lo comunica via `ask_user` — vos se lo preguntás al usuario y respondés con `answer_supervisor(agent_id, respuesta)`.
4. Cuando el supervisor termine, su resultado estará disponible en el árbol de agentes.

**Reglas:**
- Nunca inventes el resultado del supervisor. Si no tenés su respuesta, decíselo al usuario.
- Usá el `agent_id` retornado por `spawn_agent` para `answer_supervisor` si el supervisor pregunta algo.
- No hagas `spawn_agent` dos veces para el mismo proyecto si ya existe un supervisor activo.
```

---

## Verificación

```bash
cargo build --workspace
```

Buscar en los logs tras el fix:
```
CORE-264: Dispatch automático enviado al supervisor recién creado.
```

---

## Commit

```
fix(ank-core): CORE-264 dispatch automático al supervisor tras spawn_agent
```

No correr `cargo test`, no pushear. Tavo maneja git.
