# CORE-271 — Kernel: endpoint `AgentDirectMessage` para respuesta directa al supervisor

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-http/src/routes/` (nuevo endpoint)
- `kernel/crates/ank-core/src/agents/orchestrator.rs` (reusar `answer_user_question`)
**Estado:** ✅ Completo

---

## Contexto

CORE-263 implementó `answer_supervisor` como herramienta del chat_agent — el relay
pasa por el LLM. Este ticket implementa la respuesta **directa**: el usuario responde
desde el hilo dedicado (CORE-270) sin que el chat_agent intervenga.

El `AgentOrchestrator` ya tiene `answer_user_question(agent_id, answer)` — este ticket
lo expone como endpoint HTTP.

---

## Cambios requeridos

### 1. Nuevo endpoint `POST /api/agents/:agent_id/reply`

```
POST /api/agents/{agent_id}/reply
Headers: x-citadel-tenant, x-citadel-key
Body: { "answer": "Sí, podés clonar y ejecutar lo que necesites." }
```

Respuestas:
- `200 OK` → `{ "status": "delivered" }`
- `404 Not Found` → `{ "error": "no_supervisor_waiting" }` — el supervisor ya no espera
- `400 Bad Request` → `{ "error": "invalid_agent_id" }`

### 2. Handler en Axum

```rust
pub async fn reply_to_agent(
    State(state): State<AppState>,
    CitadelAuth(tenant_id): CitadelAuth,
    Path(agent_id): Path<Uuid>,
    Json(body): Json<AgentReplyBody>,
) -> impl IntoResponse {
    let orchestrator = match state.agent_orchestrator.read().await.clone() {
        Some(o) => o,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    
    if orchestrator.answer_user_question(agent_id, body.answer).await {
        Json(json!({ "status": "delivered" })).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "error": "no_supervisor_waiting" }))).into_response()
    }
}

#[derive(Deserialize)]
pub struct AgentReplyBody {
    pub answer: String,
}
```

Registrar en el router de Axum con autenticación Citadel (mismo middleware que los
otros endpoints protegidos).

### 3. Shell: llamar al endpoint desde `AgentThread`

En `AgentThread.tsx`, al enviar una respuesta:

```typescript
const handleSend = async (answer: string) => {
  const res = await fetch(`/api/agents/${agentId}/reply`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-citadel-tenant': tenant,
      'x-citadel-key': sessionKey,
    },
    body: JSON.stringify({ answer }),
  });
  
  if (res.ok) {
    // Agregar la respuesta del usuario al hilo en el store
    useAgentInboxStore.getState().addThreadMessage(agentId, {
      role: 'user',
      content: answer,
      timestamp: new Date().toISOString(),
    });
    useAgentInboxStore.getState().markAnswered(agentId);
  } else if (res.status === 404) {
    // El supervisor ya no espera — mostrar mensaje al usuario
    showToast('El supervisor ya no está esperando una respuesta.');
  }
};
```

---

## Criterios de aceptación

- [x] `POST /api/agents/:agent_id/reply` existe y está protegido por autenticación Citadel
- [x] Cuando el supervisor está en `WaitingUser`, el endpoint entrega la respuesta y retorna `200`
- [x] Cuando el supervisor ya no espera (respondió por timeout o completó), retorna `404`
- [x] El endpoint no requiere que el chat_agent esté activo — es completamente independiente
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-263 ✅ (`answer_user_question` implementado en orchestrator)
- CORE-268 (AgentOrchestrator expuesto en AppState con el canal de eventos)
