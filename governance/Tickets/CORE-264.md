# CORE-264 — Dispatch automático al ProjectSupervisor tras spawn_agent del Chat Agent

**Tipo:** fix  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
**Estado:✅ Done

---

## Contexto

El Chat Agent puede invocar `spawn_agent` para crear un ProjectSupervisor. El flujo funciona parcialmente: el supervisor se crea en el árbol y su loop tokio queda activo (confirmado en logs de producción). Sin embargo, **nadie le hace `Dispatch`** con la tarea original del usuario.

**Evidencia en logs de producción (2026-05-05):**
```
[PROJECT] ProjectSupervisor created. project=... agent=1cb87f56-e4c1-423e-980a-d10f963a20e1
ReAct: tool ejecutado, inyectando resultado al historial pid=... tool=spawn_agent
ProcessCompleted { pid: "proc_fd3d2468", output: "¡Entendido, Tavo!..." }
```

El Chat Agent recibe `{"status":"spawned"}`, termina su ReAct loop, y responde al usuario inventando información — porque el supervisor nunca recibió trabajo. El loop del supervisor queda idle indefinidamente esperando un `AgentMessage::Dispatch` que nunca llega.

**Causa raíz — `chal/mod.rs`, arm `spawn_agent` en `execute_tool_call_internal`:**

```rust
// Estado actual: solo crea el supervisor
match orchestrator.create_project(project_name.clone(), scope, task_type, pcb.tenant_id.clone()).await {
    Ok(_) => format!("{{\"status\":\"spawned\",\"project\":\"{}\"}}", project_name),
    Err(e) => format!("{{\"error\":\"{}\"}}", e),
}
// ← El supervisor existe pero jamás recibe Dispatch con la tarea del usuario.
```

El modelo `openrouter/free` (usado en producción como modelo por defecto del Chat Agent) no tiene capacidad de tool calling robusta, por lo que tampoco invoca `answer_supervisor` para retransmitir la respuesta del supervisor al usuario. Sin Dispatch, no hay respuesta real posible.

---

## Cambios requeridos

### 1. `orchestrator.rs` — exponer `dispatch()` como `pub` (si no lo es ya)

Verificar que `dispatch()` sea `pub`. Si es `pub(crate)` o privado, hacerlo `pub`.

### 2. `chal/mod.rs` — Dispatch automático tras crear el supervisor

En `execute_tool_call_internal`, arm `"spawn_agent"`, reemplazar el bloque `Ok(_)` actual por:

```rust
Ok(agent_id) => {
    // Despachar la instrucción original del usuario al supervisor recién creado.
    // La instrucción está en pcb.memory_pointers.l1_instruction (lo que el usuario escribió).
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
    // Retornar el agent_id para que el Chat Agent pueda usarlo en answer_supervisor
    format!(
        "{{\"status\":\"spawned\",\"project\":\"{}\",\"agent_id\":\"{}\"}}",
        project_name,
        agent_id
    )
}
```

**Nota importante:** `create_project` retorna `anyhow::Result<AgentId>`. El arm actual hace `Ok(_)` descartando el `AgentId`. Cambiar a `Ok(agent_id)` para capturarlo.

### 3. `kernel/config/agents/chat_agent.md` — instrucciones sobre el flujo

Agregar al final del archivo:

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

## Criterios de aceptación

- [x] Al invocar `spawn_agent` desde el Chat Agent, el supervisor recibe un `Dispatch` automático con `pcb.memory_pointers.l1_instruction` como tarea
- [x] El resultado de `spawn_agent` incluye el `agent_id` del supervisor creado
- [x] El Chat Agent informa al usuario que el supervisor está trabajando (sin inventar resultados)
- [x] Los logs muestran `CORE-264: Dispatch automático enviado al supervisor recién creado.`
- [x] `cargo build --workspace` sin errores

---

## Dependencias

- CORE-261 ✅ (bucle ReAct del Chat Agent)
- CORE-262 ✅ (execute_agent_loop para agentes del árbol)
- CORE-263 ✅ (ask_user / answer_supervisor para comunicación bottom-up)
