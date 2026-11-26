# CORE-268 — Kernel: emitir `AgentEvent` por WebSocket al tenant

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
- `kernel/crates/ank-core/src/agents/node.rs`
- `kernel/crates/ank-http/src/ws/chat.rs`
**Estado:** ✅ Completo

---

## Contexto

CORE-263 implementó `ask_user` en el kernel: el supervisor pausa y registra un
`oneshot` channel. Pero el evento que indica "supervisor tiene pregunta" no llega
a la UI — solo queda en el log como `CORE-263: supervisor pausado esperando respuesta`.

Este ticket agrega el canal de eventos estructurados que lleva esa información al
WebSocket del tenant en tiempo real.

---

## Cambios requeridos

### 1. Definir `AgentEvent` en `ank-core`

Crear `kernel/crates/ank-core/src/agents/events.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// El supervisor pausó y tiene una pregunta para el usuario.
    SupervisorQuestion {
        agent_id: Uuid,
        project_name: String,
        question: String,
        context: Option<String>,
        timestamp: DateTime<Utc>,
    },
    /// El supervisor recibió una respuesta y retomó trabajo.
    SupervisorResumed {
        agent_id: Uuid,
    },
    /// El supervisor completó su tarea.
    SupervisorCompleted {
        agent_id: Uuid,
        project_name: String,
        summary: String,
    },
}
```

Exportar desde `agents/mod.rs`.

### 2. Canal de eventos en `AgentOrchestrator`

Agregar al struct `AgentOrchestrator`:

```rust
/// CORE-268: Canal para emitir AgentEvents al WebSocket del tenant.
event_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentEvent>>,
```

Agregar método:

```rust
pub fn set_event_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>) {
    // Actualizar el campo — requiere interior mutability o setter en Arc<RwLock<...>>
    // Ver patrón existente de pending_user_replies para la forma correcta
}

pub fn emit_event(&self, event: AgentEvent) {
    if let Some(tx) = &self.event_tx {
        let _ = tx.send(event);
    }
}
```

### 3. Emitir evento en `execute_tool_call_internal` — arm `ask_user`

En el arm `ask_user` de `chal/mod.rs`, después de registrar el `oneshot` y antes de
esperar la respuesta, emitir el evento:

```rust
// Obtener nombre del proyecto del nodo
let project_name = {
    let tree = orchestrator.tree.read().await;
    tree.get(&agent_uuid)
        .and_then(|n| n.project_name.clone())
        .unwrap_or_else(|| "proyecto".to_string())
};

orchestrator.emit_event(AgentEvent::SupervisorQuestion {
    agent_id: agent_uuid,
    project_name,
    question: question.clone(),
    context: _context.clone(),
    timestamp: chrono::Utc::now(),
});
```

Después del `match timeout`:
- Si se recibió respuesta: emitir `AgentEvent::SupervisorResumed { agent_id: agent_uuid }`
- Si timeout: también emitir `SupervisorResumed` (el supervisor continúa con null)

### 4. Conectar el canal en `ws/chat.rs`

Al inicializar el WebSocket handler para un tenant, crear el canal y pasarlo al
`AgentOrchestrator`:

```rust
let (agent_event_tx, mut agent_event_rx) = 
    tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

orchestrator.set_event_channel(agent_event_tx);

// Spawn task que lee agent_event_rx y lo envía por WebSocket como mensaje JSON
tokio::spawn(async move {
    while let Some(event) = agent_event_rx.recv().await {
        if let Ok(json) = serde_json::to_string(&event) {
            // Enviar como frame de texto por el WebSocket
            // Usar el write half del socket — ver cómo se envían los tokens actuales
            let _ = ws_tx.send(Message::Text(json)).await;
        }
    }
});
```

El formato del frame que llega a la shell para `SupervisorQuestion`:
```json
{
  "type": "supervisor_question",
  "agent_id": "743d1e35-1070-47fc-8a80-7bb734953724",
  "project_name": "Aegis-Core",
  "question": "¿Puedo ejecutar comandos en el entorno?",
  "context": "Necesito clonar el repositorio si no está presente",
  "timestamp": "2026-05-06T22:16:52Z"
}
```

---

## Criterios de aceptación

- [x] `AgentEvent` existe en `agents/events.rs` con los tres variantes
- [x] Cuando un supervisor invoca `ask_user`, el WebSocket del tenant recibe un frame `supervisor_question` con los campos correctos
- [x] Cuando el supervisor recibe respuesta (o timeout), el WebSocket recibe un frame `supervisor_resumed`
- [x] Cuando un supervisor completa su tarea (arm `report`), el WebSocket recibe `supervisor_completed`
- [x] Si no hay canal configurado, `emit_event` es no-op — no panic
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-263 ✅ (ask_user implementado, WaitingUser state existe)
