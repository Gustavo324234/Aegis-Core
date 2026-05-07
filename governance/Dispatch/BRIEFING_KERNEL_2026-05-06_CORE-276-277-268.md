# BRIEFING — Sesión 2: Approved Paths + Web Search + AgentEvents al WebSocket

**Fecha:** 2026-05-06  
**Para:** Kernel Engineer (Claude Code)  
**Épica:** EPIC 50 — Agent Inbox  
**Tickets:** CORE-276, CORE-277, CORE-268

---

## Prerequisito

**La Sesión 1 (`feat/core-275-specialist-filesystem`) debe estar mergeada a `main`
antes de empezar esta sesión.**

Hacer `git pull origin main` antes de crear el branch.

Leer los tickets antes de implementar:
- `governance/Tickets/CORE-276.md`
- `governance/Tickets/CORE-277.md`
- `governance/Tickets/CORE-268.md`
- `governance/Tickets/CORE-274.md` (el contrato del envelope WS)

---

## Branch

```
feat/core-276-approved-paths-websocket
```

---

## Objetivo

### 1. CORE-276 — Aprobación de paths externos

**Archivo:** `kernel/crates/ank-core/src/enclave/` (donde vive `TenantDB`)

Agregar tres métodos a `TenantDB`:

```rust
pub fn get_approved_paths(&self) -> anyhow::Result<Vec<String>>
pub fn add_approved_path(&self, path: &str) -> anyhow::Result<()>
pub fn remove_approved_path(&self, path: &str) -> anyhow::Result<()>
```

Almacenar como JSON bajo la clave `"approved_paths"` en la misma tabla de
configuración que usa `set_persona`. Valor: array JSON de strings.

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

Reemplazar el stub `get_approved_paths` (que retorna `vec![]`) con la
implementación real que lee del enclave del tenant.

Agregar herramienta `approve_path` al ToolRegistry para `ProjectSupervisor`
y `Supervisor`. Ver definición completa en `governance/Tickets/CORE-276.md`.

Agregar arm `approve_path` en `execute_tool_call_internal`:
- Verificar que el path existe en el filesystem
- Abrir `TenantDB` del tenant
- Llamar `add_approved_path`
- Retornar `{"status":"approved","path":"..."}`

**Archivo:** `kernel/crates/ank-http/src/routes/` (endpoint opcional)

Si hay tiempo: `DELETE /api/tenant/approved-paths` para revocar desde la shell.
No bloqueante — marcar como TODO si no da el tiempo.

---

### 2. CORE-277 — web_search para Specialists

**Antes de implementar:** buscar en el codebase cómo el VCM o SirenRouter
ejecuta búsquedas web. Verificar si hay un cliente HTTP de búsqueda existente
(Brave API, DuckDuckGo, Serper, etc.). Reutilizar ese mecanismo.

**Archivo:** `kernel/crates/ank-core/src/agents/tool_registry.rs`

Agregar `web_search` al arm `AgentRole::Specialist`:

```rust
fn web_search() -> ToolDefinition {
    ToolDefinition {
        name: "web_search",
        description: "Search the web for current information. Returns titles, URLs, and snippets. Use when you need documentation, current data, or information not available in local files.",
        parameters: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query. 3-6 words work best." },
                "max_results": { "type": "integer", "description": "Max results. Default: 5. Max: 10." }
            },
            "required": ["query"]
        }),
    }
}
```

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

Arm `web_search`:
- Validar query no vacío
- Llamar al cliente de búsqueda existente
- Retornar `{"results":[{"title":"...","url":"...","snippet":"..."}],"query":"...","count":N}`
- Si no existe cliente de búsqueda reutilizable, usar `reqwest` con la misma
  API que ya esté configurada. No agregar dependencias nuevas sin verificar primero.

**Archivo:** `kernel/config/agents/specialist.md`

Agregar sección "Web search" al final. Ver texto en `governance/Tickets/CORE-277.md`.

---

### 3. CORE-268 — Kernel emite AgentEvents por WebSocket

El enum `AgentEvent` ya existe en `agents/events.rs` (creado en Sesión 1).
Este ticket lo conecta al WebSocket.

**Archivo:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

Agregar al struct:
```rust
event_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentEvent>>,
```

Agregar métodos:
```rust
pub fn set_event_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<AgentEvent>)
pub fn emit_event(&self, event: AgentEvent)   // no-op si event_tx es None
```

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

En el arm `ask_user` de `execute_tool_call_internal`, después de registrar
el oneshot y antes de esperar la respuesta:

```rust
orchestrator.emit_event(AgentEvent::SupervisorQuestion {
    agent_id: agent_uuid,
    project_name: /* obtener del nodo en el árbol */,
    question: question.clone(),
    context: _context.clone(),
    timestamp: chrono::Utc::now(),
});
```

Después del match timeout:
- Respuesta recibida → `emit_event(AgentEvent::SupervisorResumed { agent_id })`
- Timeout → `emit_event(AgentEvent::SupervisorTimedOut { agent_id, project_name })`

En el arm `report` del `run_agent_loop` (cuando el agente completa):
```rust
orchestrator.emit_event(AgentEvent::SupervisorCompleted {
    agent_id,
    project_name,
    summary: result.summary.clone(),
});
```

**Archivo:** `kernel/crates/ank-http/src/ws/chat.rs`

En `handle_chat`, al inicializar la sesión del tenant, crear el canal y
pasarlo al orchestrator:

```rust
let (agent_event_tx, mut agent_event_rx) =
    tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

// Obtener el orchestrator del AppState y llamar set_event_channel
// Ver cómo el AppState expone el orchestrator en el código existente

// Spawn task que reenvía eventos al WebSocket
tokio::spawn(async move {
    while let Some(event) = agent_event_rx.recv().await {
        if let Ok(data) = serde_json::to_value(&event) {
            let frame = serde_json::json!({
                "event": "agent_event",
                "data": data
            });
            let _ = ws_tx.send(Message::Text(frame.to_string())).await;
        }
    }
});
```

**IMPORTANTE — formato del frame:**
El envelope es `{ "event": "agent_event", "data": { "type": "supervisor_question", ... } }`.
Igual que `kernel_event`, `syslog`, `music_play`. No enviar el JSON crudo sin envelope.
Ver `governance/Tickets/CORE-274.md` para el contrato completo.

**También en `handle_chat`:** limpiar el canal al desconectar el WebSocket
para evitar leaks. El `UnboundedSender` se dropea automáticamente al salir
del scope — verificar que la task de reenvío termina limpiamente.

---

## Verificación

```bash
cargo build --workspace
```

Sin errores ni warnings nuevos.

---

## Commit y PR

**Commit message:**
```
feat(ank-core,ank-http): CORE-276/277/268 approved paths, web_search, AgentEvents WS
```

**PR title:**
```
feat(ank-core,ank-http): CORE-276/277/268 — approved paths + web_search + AgentEvents WebSocket
```

**PR description:**
```
## Cambios

### CORE-276 — Aprobación de paths externos
- `TenantDB`: `get_approved_paths`, `add_approved_path`, `remove_approved_path`
- `get_approved_paths` en `chal/mod.rs` lee del enclave real (reemplaza stub de CORE-275)
- Herramienta `approve_path` en ToolRegistry para supervisores
- Arm `approve_path` en `execute_tool_call_internal`

### CORE-277 — web_search para specialists
- Herramienta `web_search` en ToolRegistry para Specialist
- Arm `web_search` en `execute_tool_call_internal` reutilizando cliente existente
- Sección "Web search" en `specialist.md`

### CORE-268 — AgentEvents por WebSocket
- `event_tx` en `AgentOrchestrator` + `set_event_channel` + `emit_event`
- Arm `ask_user`: emite `SupervisorQuestion` antes de esperar, `SupervisorResumed` / `SupervisorTimedOut` al terminar
- Arm `report` en `run_agent_loop`: emite `SupervisorCompleted`
- `ws/chat.rs`: crea canal por sesión, pasa al orchestrator, task de reenvío con envelope `{"event":"agent_event","data":{...}}`

## Verificación
`cargo build --workspace` ✅

## Dependencias
- Requiere Sesión 1 mergeada (`feat/core-275-specialist-filesystem`)
- CORE-269 (Shell Engineer) puede arrancar con esto mergeado
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-06*
