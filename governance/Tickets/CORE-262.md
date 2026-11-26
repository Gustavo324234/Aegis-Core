# CORE-262 — AgentOrchestrator: inferencia LLM real en run_agent_loop

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/crates/ank-server/src/main.rs`

**Estado:** ✅ Done
---

## Contexto

El `run_agent_loop` arm `Dispatch` hoy resuelve el modelo via CMR correctamente, pero en lugar de llamar al LLM construye un string hardcodeado y reporta al padre:

```rust
let summary = format!("[{}] Task via model {}. Awaiting execution.", role_label, model_id);
```

El HAL (CORE-261) ya tiene `execute_with_decision` con el bucle ReAct completo y `set_orchestrator()`. El `AgentOrchestrator` ya está registrado en el HAL via `hal.set_orchestrator()` en `main.rs`. Lo que falta es la dirección inversa: el orchestrator necesita acceso al HAL para llamar al LLM.

## Cambios requeridos

### 1. `orchestrator.rs` — agregar `hal: Arc<CognitiveHAL>` al struct

```rust
pub struct AgentOrchestrator {
    pub tree: Arc<RwLock<AgentTree>>,
    pub router: Arc<RwLock<CognitiveRouter>>,
    pub vcm: Arc<VirtualContextManager>,
    channels: Arc<RwLock<HashMap<AgentId, mpsc::Sender<AgentMessage>>>>,
    persistence: Arc<AgentPersistence>,
    instruction_loader: Arc<RwLock<InstructionLoader>>,
    pub hal: Arc<ank_core::CognitiveHAL>,  // Nuevo — CORE-262
}
```

Actualizar `AgentOrchestrator::new` para recibir el HAL:

```rust
pub fn new(
    router: Arc<RwLock<CognitiveRouter>>,
    vcm: Arc<VirtualContextManager>,
    workspace_root: &std::path::Path,
    hal: Arc<ank_core::CognitiveHAL>,  // Nuevo
) -> Self {
    // ... igual que hoy ...
    Self {
        // ... campos existentes ...
        hal,
    }
}
```

### 2. `main.rs` — pasar el HAL al crear el orchestrator

```rust
// Antes:
let agent_orchestrator = Arc::new(AgentOrchestrator::new(
    Arc::clone(&router),
    Arc::new(VirtualContextManager::new()),
    &resolve_data_dir(),
));

// Después:
let agent_orchestrator = Arc::new(AgentOrchestrator::new(
    Arc::clone(&router),
    Arc::new(VirtualContextManager::new()),
    &resolve_data_dir(),
    Arc::clone(&hal),
));
```

### 3. `chal/mod.rs` — agregar `execute_agent_loop` al `CognitiveHAL`

Nuevo método público que reutiliza la lógica de `execute_with_decision` para agentes del árbol. Recibe mensajes y tools directamente sin necesitar un PCB completo:

```rust
pub async fn execute_agent_loop(
    self: Arc<Self>,
    decision: crate::router::RoutingDecision,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<serde_json::Value>>,
    text_tx: tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    agent_id: uuid::Uuid,
) -> Result<(), SystemError> {
    use crate::chal::drivers::CloudProxyDriver;

    let driver = CloudProxyDriver::new(
        Arc::clone(&self.http_client),
        decision.api_url.clone(),
        decision.api_key.clone(),
        decision.model_id.clone(),
    );

    let mut messages = messages;
    const MAX_ITERATIONS: usize = 10;

    for _iteration in 0..MAX_ITERATIONS {
        let raw_stream = driver.generate_stream(messages.clone(), None, tools.clone()).await?;
        tokio::pin!(raw_stream);

        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCallRecord> = Vec::new();

        while let Some(token_result) = raw_stream.next().await {
            match token_result {
                Ok(token) if token.starts_with("__TOOL_CALL__") => {
                    let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                    if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str) {
                        tool_calls.push(ToolCallRecord {
                            id: tc.id,
                            type_: "function".to_string(),
                            function: FunctionCallRecord { name: tc.name, arguments: tc.arguments.to_string() },
                        });
                    }
                }
                Ok(text) => {
                    assistant_text.push_str(&text);
                    let _ = text_tx.send(Ok(text));
                }
                Err(e) => { let _ = text_tx.send(Err(e)); }
            }
        }

        if tool_calls.is_empty() { break; }

        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: if assistant_text.is_empty() { None } else { Some(assistant_text) },
            tool_calls: Some(tool_calls.clone()),
            ..Default::default()
        });

        // PCB mínimo para execute_tool_call_internal
        let mut mock_pcb = crate::pcb::PCB::new(format!("agent_{}", agent_id), 5, String::new());
        mock_pcb.agent_id = Some(agent_id);

        for tc in &tool_calls {
            let result = self.execute_tool_call_internal(tc, &agent_id.to_string(), &mock_pcb).await;
            messages.push(ChatMessage {
                role: ChatRole::Tool,
                content: Some(result),
                tool_call_id: Some(tc.id.clone()),
                name: Some(tc.function.name.clone()),
                ..Default::default()
            });
        }
    }

    Ok(())
}
```

### 4. `orchestrator.rs` — refactorizar arm `Dispatch` en `run_agent_loop`

`run_agent_loop` es una función estática. Agregar `hal: Arc<CognitiveHAL>` como parámetro y pasarlo desde `spawn_loop`.

En el arm `AgentMessage::Dispatch`, después de resolver el `decision` via CMR (mantener ese código igual), reemplazar el bloque del summary hardcodeado por:

```rust
// Construir mensajes del agente
let system_prompt = {
    let t = tree.read().await;
    t.get(&agent_id).map(|n| n.system_prompt.clone()).unwrap_or_default()
};

let child_reports_text = context.child_reports.iter()
    .map(|r| format!("• {}: {}", r.role_description, r.summary))
    .collect::<Vec<_>>().join("\n");

let mut messages = vec![
    ChatMessage { role: ChatRole::System, content: Some(system_prompt), ..Default::default() },
];
if !child_reports_text.is_empty() {
    messages.push(ChatMessage {
        role: ChatRole::System,
        content: Some(format!("[REPORTES DE SUBAGENTES]\n{}", child_reports_text)),
        ..Default::default()
    });
}
messages.push(ChatMessage {
    role: ChatRole::User,
    content: Some(task_description.clone()),
    ..Default::default()
});

let tools = {
    let t = tree.read().await;
    t.get(&agent_id).map(|n| {
        let provider = ProviderKind::from_string(&decision.provider);
        let defs = ToolRegistry::tools_for(&n.role, &provider);
        if defs.is_empty() { None } else { Some(defs) }
    }).flatten()
};

let (text_tx, mut text_rx) = tokio::sync::mpsc::unbounded_channel::<Result<String, ank_core::chal::ExecutionError>>();
let hal_clone = Arc::clone(&hal);
let model_id_for_meta = decision.model_id.clone();

let collect_task = tokio::spawn(async move {
    let mut full = String::new();
    while let Some(Ok(token)) = text_rx.recv().await { full.push_str(&token); }
    full
});

if let Err(e) = hal_clone.execute_agent_loop(decision, messages, tools, text_tx, agent_id).await {
    warn!("[{}] LLM execution failed: {}", role_label, e);
    Self::fail_agent(agent_id, &e.to_string(), &tree, &parent_tx).await;
    return;
}

let response = collect_task.await.unwrap_or_default();

{
    let mut t = tree.write().await;
    if let Some(n) = t.get_mut(&agent_id) {
        n.set_state(AgentState::Complete);
        n.set_last_report(response.clone());
    }
}

if let Some(ref ptx) = parent_tx {
    let report = AgentMessage::Report {
        from: agent_id,
        result: AgentResult {
            agent_id,
            role_description: role_label.clone(),
            summary: response,
            artifacts: Vec::new(),
            metadata: serde_json::json!({ "model_used": model_id_for_meta }),
        },
        status: ReportStatus::Success,
    };
    if let Err(e) = ptx.send(report).await {
        error!("[{}] Failed to report to parent: {}", role_label, e);
    }
}
```

## Criterios de aceptación

- [ ] Los supervisores llaman al LLM con su system prompt y la tarea recibida
- [ ] Los agentes pueden usar sus herramientas via el bucle ReAct del HAL
- [ ] El reporte al padre contiene la respuesta real del LLM — no un string hardcodeado
- [ ] `AgentOrchestrator::new` recibe `Arc<CognitiveHAL>` y `main.rs` lo pasa correctamente
- [ ] `cargo build --workspace` sin errores

## Dependencias

- CORE-261 ✅ (bucle ReAct en HAL — ya implementado)
