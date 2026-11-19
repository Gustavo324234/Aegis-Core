# CORE-263 — Comunicación Bottom-Up: herramienta ask_user + estado WaitingUser

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 49 — Cognitive Loop  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs`
- `kernel/crates/ank-core/src/agents/node.rs`
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/config/agents/chat_agent.md`
**Estado:✅ Done

---

## Contexto

Los supervisores trabajan en silencio. Este ticket implementa el canal bottom-up: los supervisores pueden pausar su ejecución y hacerle preguntas al usuario via el Chat Agent.

**Flujo:**
```
Supervisor invoca ask_user("¿Tailwind o CSS normal?")
    ↓
Orchestrator registra oneshot channel + pone agente en WaitingUser
    ↓
execute_tool_call_internal espera en reply_rx (timeout 10min)
    ↓
Usuario responde en el chat → Chat Agent invoca answer_supervisor(agent_id, answer)
    ↓
execute_tool_call_internal de answer_supervisor llama orchestrator.answer_user_question()
    ↓
El oneshot se resuelve → supervisor recibe la respuesta y continúa
```

## Cambios requeridos

### 1. `tool_registry.rs` — agregar `ask_user` para Supervisores

En `ToolRegistry::tools_for()`, para `AgentRole::Supervisor` y `AgentRole::ProjectSupervisor`, agregar (verificar el formato exacto que usan las otras herramientas en ese archivo):

```json
{
  "type": "function",
  "function": {
    "name": "ask_user",
    "description": "Pausar la tarea y hacerle una pregunta al usuario via el Chat Agent. Usar cuando necesités una decisión que solo el usuario puede tomar. El supervisor queda en pausa hasta recibir la respuesta.",
    "parameters": {
      "type": "object",
      "properties": {
        "question": { "type": "string", "description": "La pregunta para el usuario" },
        "context": { "type": "string", "description": "Contexto breve de por qué necesitás esta información" }
      },
      "required": ["question"]
    }
  }
}
```

### 2. `tool_registry.rs` — agregar `answer_supervisor` para ChatAgent

Para `AgentRole::ChatAgent`:

```json
{
  "type": "function",
  "function": {
    "name": "answer_supervisor",
    "description": "Enviar la respuesta del usuario a un supervisor que está esperando input. Usar cuando el usuario responde a una pregunta de un supervisor activo.",
    "parameters": {
      "type": "object",
      "properties": {
        "agent_id": { "type": "string", "description": "UUID del agente supervisor que está esperando" },
        "answer": { "type": "string", "description": "La respuesta del usuario" }
      },
      "required": ["agent_id", "answer"]
    }
  }
}
```

### 3. `node.rs` — agregar `WaitingUser` al enum `AgentState`

```rust
pub enum AgentState {
    // ... estados existentes ...
    /// CORE-263: El agente pausó esperando input del usuario via ask_user.
    WaitingUser,
}
```

Actualizar cualquier `match` exhaustivo sobre `AgentState` que el compilador indique.

### 4. `orchestrator.rs` — `pending_user_replies` + métodos

Agregar al struct:

```rust
pub struct AgentOrchestrator {
    // ... campos existentes ...
    /// CORE-263: Canales oneshot para respuestas de usuario a supervisores pausados.
    pending_user_replies: Arc<RwLock<HashMap<AgentId, tokio::sync::oneshot::Sender<String>>>>,
}
```

Inicializar en `new()`:
```rust
pending_user_replies: Arc::new(RwLock::new(HashMap::new())),
```

Agregar dos métodos públicos:

```rust
pub async fn register_user_reply(
    &self,
    agent_id: AgentId,
    tx: tokio::sync::oneshot::Sender<String>,
) {
    self.pending_user_replies.write().await.insert(agent_id, tx);
}

pub async fn answer_user_question(&self, agent_id: AgentId, answer: String) -> bool {
    match self.pending_user_replies.write().await.remove(&agent_id) {
        Some(tx) => tx.send(answer).is_ok(),
        None => false,
    }
}
```

### 5. `chal/mod.rs` — arm `ask_user` en `execute_tool_call_internal`

```rust
"ask_user" => {
    let question = args["question"].as_str().unwrap_or("").to_string();
    let _context = args.get("context").and_then(|v| v.as_str()).map(String::from);

    // Resolver agent_id del PCB
    let agent_uuid = match pcb.agent_id {
        Some(id) => id,
        None => return "{\"error\":\"ask_user requiere agent_id en PCB\"}".to_string(),
    };

    let orchestrator_opt = self.agent_orchestrator.read().await.clone();
    let orchestrator = match orchestrator_opt {
        Some(o) => o,
        None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel::<String>();
    orchestrator.register_user_reply(agent_uuid, reply_tx).await;

    // Cambiar estado a WaitingUser
    {
        let mut tree = orchestrator.tree.write().await;
        if let Some(node) = tree.get_mut(&agent_uuid) {
            node.set_state(crate::agents::node::AgentState::WaitingUser);
        }
    }

    tracing::info!(
        agent = %agent_uuid,
        question = %question,
        "CORE-263: supervisor pausado esperando respuesta del usuario"
    );

    // Esperar respuesta con timeout de 10 minutos
    match tokio::time::timeout(std::time::Duration::from_secs(600), reply_rx).await {
        Ok(Ok(answer)) => {
            let mut tree = orchestrator.tree.write().await;
            if let Some(node) = tree.get_mut(&agent_uuid) {
                node.set_state(crate::agents::node::AgentState::Running);
            }
            format!("{{\"user_answer\": {}}}", serde_json::json!(answer))
        }
        _ => {
            let mut tree = orchestrator.tree.write().await;
            if let Some(node) = tree.get_mut(&agent_uuid) {
                node.set_state(crate::agents::node::AgentState::Running);
            }
            "{\"user_answer\": null, \"reason\": \"timeout\"}".to_string()
        }
    }
}
```

### 6. `chal/mod.rs` — arm `answer_supervisor` en `execute_tool_call_internal`

```rust
"answer_supervisor" => {
    let agent_id_str = args["agent_id"].as_str().unwrap_or("");
    let answer = args["answer"].as_str().unwrap_or("").to_string();

    let agent_uuid = match agent_id_str.parse::<uuid::Uuid>() {
        Ok(id) => id,
        Err(_) => return "{\"error\":\"agent_id inválido\"}".to_string(),
    };

    let orchestrator_opt = self.agent_orchestrator.read().await.clone();
    match orchestrator_opt {
        None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
        Some(orchestrator) => {
            if orchestrator.answer_user_question(agent_uuid, answer).await {
                "{\"status\":\"answer_delivered\"}".to_string()
            } else {
                "{\"status\":\"no_supervisor_waiting\"}".to_string()
            }
        }
    }
}
```

### 7. `kernel/config/agents/chat_agent.md` — agregar instrucciones al final

```markdown

## Comunicación con Supervisores

Cuando recibas un mensaje de sistema indicando que un supervisor hizo una pregunta, presentásela al usuario de forma natural. Ejemplo: "El equipo trabajando en el proyecto X necesita saber: ¿preferís usar Tailwind o CSS normal?"

Cuando el usuario responda, usá la herramienta `answer_supervisor` con el `agent_id` del supervisor y la respuesta. Confirmale al usuario que su respuesta fue enviada.

No expongas UUIDs al usuario — referenciá el proyecto por nombre.
```

## Criterios de aceptación

- [x] Un supervisor puede invocar `ask_user` y su ejecución se pausa
- [x] Cuando el usuario responde via `answer_supervisor`, el supervisor recibe la respuesta y continúa
- [x] Si pasan 10 minutos sin respuesta, el supervisor continúa con `user_answer: null`
- [x] `AgentState::WaitingUser` existe en `node.rs`
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-262 ✅ (AgentOrchestrator con LLM real)
