# CORE-243 — fix(ank-core): SyscallExecutor permite spawn desde Chat Agent (PCB sin agent_id)

**Tipo:feat
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

Con CORE-241 implementado, el `__TOOL_CALL__` ya no llega al frontend.
Sin embargo el usuario recibe output vacío porque el `SyscallExecutor` rechaza
el tool call con:

```
AgentToolCall requires an active agent_id in PCB
```

El Chat Agent (PCB sin `agent_id`) invoca `spawn_agent(role="project_supervisor")`
pero el `SyscallExecutor` asume que solo agentes con `agent_id` pueden spawnear.

**El Chat Agent es la raíz del árbol.** Es él quien debe poder crear
`ProjectSupervisors` — esa es exactamente su función. La validación actual
rompe el caso de uso principal del sistema.

---

## Lógica correcta

```
PCB sin agent_id + spawn_agent(role="project_supervisor") → crear proyecto raíz ✅
PCB sin agent_id + spawn_agent(role="supervisor"|"specialist") → error ❌ (inválido)
PCB con agent_id + spawn_agent(role="supervisor"|"specialist") → crear hijo en árbol ✅
PCB con agent_id + spawn_agent(role="project_supervisor") → error ❌ (inválido)
```

---

## Cambios requeridos

### En `SyscallExecutor::dispatch()` (o donde se valida el `agent_id`)

Reemplazar la validación actual:

```rust
// ANTES — rechaza todos los PCBs sin agent_id
if pcb.agent_id.is_none() {
    return Err("AgentToolCall requires an active agent_id in PCB");
}
```

Por la lógica correcta:

```rust
match &tool_call {
    AgentToolCall::Spawn { role, name, scope, task_type } => {
        match (pcb.agent_id.as_ref(), role) {
            // Chat Agent crea ProjectSupervisor — caso raíz válido
            (None, AgentRole::ProjectSupervisor) => {
                orchestrator.create_project(
                    name.clone().unwrap_or_else(|| scope.clone()),
                    scope.clone(),
                    task_type.clone(),
                    pcb.tenant_id.clone(),
                ).await?;
            }
            // Agente del árbol crea hijo — caso normal válido
            (Some(agent_id), AgentRole::Supervisor | AgentRole::Specialist) => {
                orchestrator.spawn_child(
                    agent_id,
                    role.clone(),
                    name.clone(),
                    scope.clone(),
                    task_type.clone(),
                ).await?;
            }
            // Casos inválidos
            (None, _) => {
                return Err("Chat Agent can only spawn ProjectSupervisors");
            }
            (Some(_), AgentRole::ProjectSupervisor) => {
                return Err("Only Chat Agent can create ProjectSupervisors");
            }
        }
    }
    // ... query y report
}
```

### Respuesta al usuario post-spawn

Después de crear el `ProjectSupervisor` exitosamente, el runner debe enviar
al usuario una respuesta de texto confirmando la acción. Opciones:

**Opción A — respuesta hardcodeada del kernel:**
El runner emite un mensaje fijo al canal de output:
```rust
output_tx.send("Listo, activé el equipo para el proyecto.").await;
```

**Opción B — segundo turno del modelo:**
El kernel hace una segunda llamada al LLM con el `tool_result` para que el
Chat Agent genere una respuesta natural. Más flexible pero más costoso en tokens.

**Implementar Opción A** por ahora — es simple, predecible y no consume tokens
adicionales. Se puede evolucionar a Opción B en el futuro con Tool Use completo.

---

## Criterios de aceptación

- [ ] "hola" no dispara un spawn (el modelo debe pedir confirmación o contexto)
- [ ] "trabajemos en el proyecto Aegis" crea un `ProjectSupervisor` en el `ProjectRegistry`
- [ ] El usuario recibe una respuesta de texto confirmando que el equipo fue activado
- [ ] Los logs muestran el `ProjectSupervisor` creado con su ID
- [ ] El `AgentTreeView` del dashboard muestra el nodo del proyecto
- [ ] PCB sin `agent_id` + `spawn_agent(role="supervisor")` retorna error en logs (no crash)
- [ ] `cargo build` pasa sin warnings nuevos

---

## Dependencias

- CORE-241 (filtro __TOOL_CALL__) — debe estar completo ✅
