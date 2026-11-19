# CORE-282 — chat_agent.md: eliminar instrucciones de token parser legacy

**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/config/agents/chat_agent.md`
- `kernel/crates/ank-core/src/chal/mod.rs` (fallback handling)

---

## Contexto

El log de producción muestra:

```
output: "...voy a intentar activar de nuevo...
[SYS_AGENT_SPAWN(role=\"project_supervisor\", name=\"Aegis-Core\", scope=\"...\")]"
```

El token `[SYS_AGENT_SPAWN(...)]` llegó visible al usuario en el chat. Dos problemas:

1. **El token es legacy** — EPIC 47 migró a tool use nativo. El `chat_agent.md` todavía
   tiene la instrucción del token parser viejo en los ejemplos de la sección
   "When to Spawn".

2. **El modelo `openrouter/free` no soporta tool use** — cuando el LLM no puede hacer
   una tool call, cae al texto libre y genera el token. El kernel no lo procesa
   (el `SYS_AGENT_SPAWN` regex no existe en `syscalls/mod.rs`) y el token llega
   al usuario como texto.

---

## Cambios requeridos

### 1. `kernel/config/agents/chat_agent.md` — eliminar tokens legacy

Reemplazar la sección "When to Spawn" que contiene ejemplos con `[SYS_AGENT_SPAWN(...)]`
por instrucciones de tool use nativo:

```markdown
## When to delegate work

Use the `spawn_agent` tool when:
- The user wants to work on something concrete ("let's work on X", "fix Y")
- The user asks a technical question about a project
- The task requires reading or modifying files, code, or any resource
- You cannot answer with what you already have in context

Call `spawn_agent` with:
- `role`: always `"project_supervisor"` when creating a new project team
- `name`: the project name (short, clear)
- `scope`: what the user wants done (one sentence)

Examples:
- "let's work on Aegis" → spawn_agent(role="project_supervisor", name="Aegis", scope="user wants to work on the Aegis project")
- "what does authenticate_tenant do?" → spawn_agent(role="project_supervisor", name="Aegis", scope="explain authenticate_tenant function")

NEVER generate [SYS_AGENT_SPAWN(...)] tokens — use the spawn_agent tool directly.
```

### 2. `kernel/crates/ank-core/src/chal/mod.rs` — fallback para modelos sin tool use

Cuando el LLM genera un token `[SYS_AGENT_SPAWN(...)]` en lugar de una tool call,
el kernel debe detectarlo y procesarlo en lugar de dejarlo pasar al output.

En la función que procesa el output del LLM (donde se emiten tokens al WebSocket),
agregar detección del token legacy y conversión a `AgentToolCall`:

```rust
// CORE-282: Detectar token legacy [SYS_AGENT_SPAWN(...)] para modelos sin tool use
// Esto ocurre cuando openrouter/free u otros modelos no soportan function calling
static LEGACY_SPAWN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_AGENT_SPAWN\(role="([^"]+)",\s*name="([^"]+)",\s*scope="([^"]+)"\)\]"#)
        .unwrap_or_else(|_| panic!("FATAL: legacy spawn regex invalid"))
});

// En el procesamiento de output tokens:
if let Some(caps) = LEGACY_SPAWN_RE.captures(&token) {
    let role_str = &caps[1];
    let name = caps[2].to_string();
    let scope = caps[3].to_string();

    // Convertir a AgentToolCall y ejecutar
    let role = match role_str {
        "project_supervisor" => AgentRole::ProjectSupervisor {
            project_id: uuid::Uuid::new_v4().to_string(),
        },
        _ => AgentRole::Supervisor { name: name.clone(), depth: 1 },
    };

    let tool_call = AgentToolCall::Spawn {
        role,
        name: Some(name),
        scope,
        task_type: None,
    };

    // Ejecutar silenciosamente — no emitir el token al WebSocket
    let _ = syscall_executor.execute_agent_tool_call(&pcb, tool_call).await;
    continue; // no emitir el token legacy al output
}
```

Si el token legacy no puede procesarse, ignorarlo silenciosamente — no emitirlo
al usuario como texto.

---

## Criterios de aceptación

- [ ] `chat_agent.md` no contiene ninguna referencia a `[SYS_AGENT_SPAWN(...)]`
- [ ] Cuando un modelo genera el token legacy, el kernel lo intercepta y ejecuta
  la tool call equivalente sin que el token llegue al usuario
- [ ] Si el token legacy no puede parsearse, se descarta silenciosamente
- [ ] El comportamiento con modelos que sí soportan tool use no cambia
- [ ] `cargo build --workspace` sin errores

## Dependencias

- Ninguna — fix independiente

---

## Resolución (2026-06-12)
Verificado 2026-06-12: ya implementado en PR #237 — chat_agent.md instruye 'NEVER generate [SYS_AGENT_SPAWN...]' y chal/mod.rs:204 intercepta el token legacy. Reconciliación de governance.
