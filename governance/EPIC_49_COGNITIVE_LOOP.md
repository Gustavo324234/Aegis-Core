# EPIC 49 — Cognitive Loop: Memoria, ReAct y Comunicación Bottom-Up

**Estado:** 📥 Planned  
**Prioridad:** Crítica  
**Responsable planning:** Arquitecto IA  
**Ingeniero:** Kernel Engineer  
**Referencia arquitectónica:** Análisis adjunto (documento del usuario, 2026-05-04)

---

## Contexto

El sistema actual tiene una limitación fundamental: cada request al LLM es stateless. El `CloudProxyDriver` construye un único mensaje `{ role: "user", content: <prompt_gigante> }` con todo el contexto concatenado en un string. Cuando el LLM emite un tool call, el driver lo detecta y emite un token sintético `__TOOL_CALL__{json}`, pero el ciclo termina ahí — el resultado de la herramienta nunca vuelve al LLM en el mismo turno.

Consecuencias directas observadas en el smoke test:
- El agente "activa el proyecto" pero no puede responder preguntas sobre él en el mismo turno
- Los supervisores responden con summaries hardcodeados (run_agent_loop no llama al LLM)
- Los mensajes robóticos hardcodeados en ank-server ("Estoy solicitando al equipo...") los escribe el servidor, no el modelo

Este epic implementa el salto arquitectónico en 3 fases secuenciales.

---

## Fase 1 — Memoria Conversacional (Vec<Message>)

### CORE-259 — CloudProxyDriver: soporte de historial de mensajes `Vec<Message>`

**Componentes:**
- `kernel/crates/ank-core/src/chal/drivers/cloud.rs`
- `kernel/crates/ank-core/src/chal/mod.rs` (signature de InferenceDriver)
- `kernel/crates/ank-core/src/pcb.rs`

**Problema actual:**

`generate_stream` recibe un `prompt: String` y construye siempre:
```rust
messages: vec![Message { role: "user", content: prompt }]
```

Un solo mensaje gigante que concatena system prompt + contexto VCM + instrucción del usuario. El LLM no puede ver el historial de la conversación ni los resultados anteriores de herramientas.

**Cambios requeridos:**

1. Cambiar la firma de `InferenceDriver::generate_stream`:
```rust
// Antes:
async fn generate_stream(
    &self,
    prompt: String,
    grammar: Option<Grammar>,
    tools: Option<Vec<serde_json::Value>>,
) -> GenerateStreamResult;

// Después:
async fn generate_stream(
    &self,
    messages: Vec<ChatMessage>,  // historial completo
    grammar: Option<Grammar>,
    tools: Option<Vec<serde_json::Value>>,
) -> GenerateStreamResult;
```

2. Definir `ChatMessage` en `chal/mod.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: ChatContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,  // Para mensajes role=tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRecord>>,  // Para mensajes role=assistant con tool calls
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}
```

3. Actualizar `CloudProxyDriver::generate_stream` para serializar `Vec<ChatMessage>` directamente en lugar de construir el array de un solo elemento.

4. Actualizar `build_prompt` en `CognitiveHAL` para retornar `Vec<ChatMessage>` en lugar de `String`:
```rust
pub async fn build_messages(&self, pcb: &PCB, persona: Option<&str>) -> Vec<ChatMessage> {
    // 1. System message con el system prompt + persona + instrucciones de rol
    let system_content = /* ... system prompt + persona + instrucciones ... */;
    let mut messages = vec![
        ChatMessage { role: ChatRole::System, content: ChatContent::Text(system_content), .. }
    ];
    // 2. Historial de mensajes del PCB (si existe)
    messages.extend(pcb.message_history.clone());
    // 3. El mensaje del usuario actual
    messages.push(ChatMessage { 
        role: ChatRole::User, 
        content: ChatContent::Text(pcb.memory_pointers.l1_instruction.clone()),
        ..
    });
    messages
}
```

5. Agregar `message_history: Vec<ChatMessage>` al PCB para que el scheduler pueda acumular el historial entre turnos de la misma sesión.

**Criterios de aceptación:**
- [ ] `generate_stream` acepta `Vec<ChatMessage>` en lugar de `String`
- [ ] El sistema prompt va en `role: "system"` — nunca concatenado al `role: "user"`
- [ ] `cargo build --workspace` sin errores
- [ ] Los tests existentes actualizados para usar la nueva firma

---

### CORE-260 — PCB: acumular historial de mensajes entre turnos

**Componente:** `kernel/crates/ank-core/src/pcb.rs`, `kernel/crates/ank-core/src/scheduler/mod.rs`

**Problema actual:** Cada request crea un PCB nuevo sin contexto del turno anterior. La "memoria" de la conversación existe solo en el `chat_history.log` en disco, pero nunca se carga en el contexto del LLM.

**Cambios requeridos:**

1. Agregar `message_history: Vec<ChatMessage>` al struct `PCB`
2. En el handler del WebSocket (`ws/chat.rs`), al crear el PCB para un nuevo mensaje, cargar el historial de la sesión activa del tenant desde una cache en memoria (no desde disco — eso es para CORE-247)
3. Crear `SessionHistoryCache`: un `HashMap<tenant_id, VecDeque<ChatMessage>>` con un límite configurable (default: 20 mensajes) — mantenido en memoria en el `CognitiveScheduler` o en el state del WebSocket handler
4. Al completar un proceso, agregar el par `(user_message, assistant_response)` al cache de la sesión

**Criterios de aceptación:**
- [ ] Dentro de la misma sesión WebSocket, el LLM recibe el historial de los últimos N mensajes
- [ ] Al reconectar (nuevo WebSocket), el historial se pierde — CORE-247 resuelve la persistencia
- [ ] El historial no crece indefinidamente — se trunca a los últimos 20 mensajes
- [ ] `cargo build --workspace` sin errores

---

## Fase 2 — Bucle ReAct Interno en CognitiveHAL

### CORE-261 — CognitiveHAL: bucle ReAct — ejecutar herramientas e inyectar resultado al LLM

**Componente:** `kernel/crates/ank-core/src/chal/mod.rs` — `execute_with_decision`

**Problema actual:**

`execute_with_decision` llama a `driver.generate_stream(prompt, tools)` una sola vez. Cuando el driver emite un `__TOOL_CALL__` token, el stream termina. El `StreamInterceptor` en `ank-server/main.rs` captura ese token, ejecuta el syscall, y hardcodea un mensaje de respuesta al usuario. El LLM nunca ve el resultado de la herramienta.

**Cambios requeridos:**

Reemplazar la llamada única por un bucle en `execute_with_decision`:

```rust
async fn execute_with_decision(
    &self,
    decision: RoutingDecision,
    pcb: &PCB,
    pid: &str,
    persona: Option<&str>,
    // Nuevo: canal para enviar tokens de texto al stream de salida
    text_tx: tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
) -> Result<(), SystemError> {
    let driver = /* ... crear driver como hoy ... */;
    let mut messages = self.build_messages(pcb, persona).await;
    
    loop {
        let stream = driver.generate_stream(messages.clone(), None, tools.clone()).await?;
        
        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCallRecord> = Vec::new();
        
        // Consumir el stream
        tokio::pin!(stream);
        while let Some(token) = stream.next().await {
            match token? {
                t if t.starts_with("__TOOL_CALL__") => {
                    // Acumular tool call — no enviar al usuario
                    let json_str = t.strip_prefix("__TOOL_CALL__").unwrap_or("");
                    if let Ok(tc) = serde_json::from_str::<ToolCallRecord>(json_str) {
                        tool_calls.push(tc);
                    }
                }
                text => {
                    // Enviar texto normal al usuario via stream
                    assistant_text.push_str(&text);
                    let _ = text_tx.send(Ok(text));
                }
            }
        }
        
        if tool_calls.is_empty() {
            // No hay tool calls — el LLM terminó de responder
            break;
        }
        
        // Hay tool calls — agregar respuesta del asistente al historial
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: if assistant_text.is_empty() { ChatContent::Null } else { ChatContent::Text(assistant_text) },
            tool_calls: Some(tool_calls.clone()),
            ..Default::default()
        });
        
        // Ejecutar cada tool call y agregar resultado al historial
        for tc in &tool_calls {
            let result = self.execute_tool_call(tc, pid, pcb).await;
            messages.push(ChatMessage {
                role: ChatRole::Tool,
                content: ChatContent::Text(result),
                tool_call_id: Some(tc.id.clone()),
                ..Default::default()
            });
        }
        
        // Volver al inicio del loop — el LLM verá los resultados y responderá
    }
    
    Ok(())
}
```

El método `execute_tool_call` delega al `SyscallExecutor` actual — no hay que reimplementar la lógica de syscalls, solo conectarla al nuevo loop.

**Limpiar `ank-server/main.rs`:**

El `StreamInterceptor` que parsea `__TOOL_CALL__` tokens y los despacha debe **eliminarse** una vez que el HAL maneje el bucle internamente. El `ank-server` pasa a ser "tonto": solo recibe tokens de texto del HAL y los envía por WebSocket.

Los mensajes hardcodeados como `"Estoy solicitando al equipo..."` que el servidor inyecta también se eliminan — el LLM generará su propia respuesta al ver el resultado del tool call.

**Criterios de aceptación:**
- [ ] Cuando el LLM emite un tool call, el HAL lo ejecuta invisiblemente y vuelve a llamar al LLM con el resultado
- [ ] El usuario solo ve el texto final del LLM — nunca los tokens `__TOOL_CALL__` crudos
- [ ] El StreamInterceptor en ank-server ya no parsea ni ejecuta tool calls
- [ ] Los mensajes hardcodeados del servidor se eliminan
- [ ] Si el LLM encadena múltiples tool calls en secuencia (ReAct chain), todos se ejecutan antes de responder al usuario
- [ ] El bucle tiene un límite de iteraciones (max 10) para prevenir loops infinitos
- [ ] `cargo build --workspace` sin errores

---

### CORE-262 — AgentOrchestrator: run_agent_loop con inferencia LLM real

**Componente:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

**Problema actual:** El `run_agent_loop` en el arm `Dispatch` construye un summary hardcodeado y reporta al padre sin llamar al LLM. Los supervisores nunca hacen inferencia real.

**Cambios requeridos:**

En el arm `AgentMessage::Dispatch { task_description, context }` de `run_agent_loop`:

```rust
AgentMessage::Dispatch { task_description, context, reply_tx } => {
    // 1. Construir mensajes para el agente
    let mut messages = vec![
        ChatMessage { role: ChatRole::System, content: ChatContent::Text(agent_system_prompt) },
        ChatMessage { role: ChatRole::User, content: ChatContent::Text(task_description) },
    ];
    if let Some(ctx) = context {
        // Inyectar contexto como mensaje del sistema adicional
        messages.insert(1, ChatMessage { role: ChatRole::System, content: ChatContent::Text(ctx) });
    }
    
    // 2. Ejecutar ReAct loop para este agente (reusar execute_with_decision del HAL)
    let result = hal.execute_agent_loop(node_id, messages, &tools_for_role).await?;
    
    // 3. Reportar al padre con la respuesta real del LLM
    let _ = reply_tx.send(AgentMessage::Report {
        from: node_id.clone(),
        content: result,
    });
}
```

Esto requiere que el `AgentOrchestrator` tenga acceso al `CognitiveHAL` — actualmente no lo tiene. Pasar el HAL como dependencia al orchestrator al inicializarlo.

**Criterios de aceptación:**
- [ ] Los agentes supervisores llaman al LLM con su system prompt y la tarea recibida
- [ ] Los agentes pueden usar sus herramientas (spawn_agent, query_agent, report) via el bucle ReAct
- [ ] El reporte al padre contiene la respuesta real del LLM, no un string hardcodeado
- [ ] `cargo build --workspace` sin errores

---

## Fase 3 — Comunicación Bottom-Up (Supervisores → Chat)

### CORE-263 — Nueva herramienta `ask_user` + estado WaitingUser en AgentNode

**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs` — nueva herramienta `ask_user`
- `kernel/crates/ank-core/src/agents/node.rs` — nuevo estado `WaitingUser`
- `kernel/crates/ank-core/src/agents/orchestrator.rs` — handler del arm `AskUser`
- `kernel/crates/ank-http/src/ws/chat.rs` — enrutar preguntas del supervisor al Chat Agent

**Diseño:**

1. **Nueva herramienta `ask_user`** en el ToolRegistry para rol `Supervisor`:
```json
{
  "name": "ask_user",
  "description": "Pausar la tarea y hacerle una pregunta al usuario via el Chat Agent. Usar cuando necesites una decisión que solo el usuario puede tomar.",
  "parameters": {
    "question": "La pregunta para el usuario",
    "context": "Contexto breve de por qué necesitás esta información"
  }
}
```

2. **Nuevo estado `AgentState::WaitingUser`** en `node.rs`

3. **Handler en orchestrator:** cuando el LLM del supervisor invoca `ask_user`:
   - Cambiar el estado del nodo a `WaitingUser { question, context, reply_channel }`
   - Emitir un `AgentEvent` al WebSocket con `{ type: "supervisor_question", project_id, question, context }`
   - El loop del agente se suspende esperando en `reply_channel`

4. **En el Chat Agent:** cuando llega un `AgentEvent::SupervisorQuestion`, el Chat Agent lo presenta al usuario naturalmente: *"El supervisor del proyecto X me pregunta: ¿preferís Tailwind o CSS normal?"*

5. **Cuando el usuario responde:** el Chat Agent invoca la herramienta `answer_supervisor(project_id, answer)` → el orchestrator despierta el loop del agente e inyecta la respuesta como mensaje `role: tool` → el supervisor continúa

**Criterios de aceptación:**
- [ ] Un supervisor puede invocar `ask_user` y pausar su ejecución
- [ ] La pregunta aparece en el chat del usuario via el Chat Agent con lenguaje natural
- [ ] Cuando el usuario responde, el supervisor recibe la respuesta y continúa su tarea
- [ ] El supervisor puede hacer múltiples preguntas en secuencia
- [ ] Si el usuario tarda más de X minutos sin responder, el supervisor puede continuar con un default o cancelar
- [ ] `cargo build --workspace` sin errores

---

## Orden de ejecución

```
CORE-259 (firma Vec<Message>)
    ↓
CORE-260 (historial en PCB + cache de sesión)
    ↓
CORE-261 (bucle ReAct en HAL)
    ↓
CORE-262 (AgentOrchestrator con LLM real)
    ↓
CORE-263 (ask_user + comunicación Bottom-Up)
```

CORE-259 y CORE-260 son prerrequisitos bloqueantes de todos los demás. CORE-261 depende de 259+260. CORE-262 depende de 261. CORE-263 depende de 262.

---

## Impacto en código existente

| Archivo | Cambio |
|---|---|
| `chal/mod.rs` | Nuevo tipo `ChatMessage`, firma `build_messages`, bucle ReAct |
| `chal/drivers/cloud.rs` | `generate_stream(Vec<ChatMessage>)` en lugar de `String` |
| `pcb.rs` | Campo `message_history: Vec<ChatMessage>` |
| `agents/orchestrator.rs` | Inferencia LLM real en `run_agent_loop`, acceso al HAL |
| `agents/tool_registry.rs` | Nueva herramienta `ask_user` para Supervisor |
| `agents/node.rs` | Nuevo estado `WaitingUser` |
| `ank-server/main.rs` | **Eliminar** StreamInterceptor de tool calls y mensajes hardcodeados |
| `ws/chat.rs` | Enrutar `SupervisorQuestion` events al usuario |

El cambio más disruptivo es la firma de `InferenceDriver`. Afecta a todos los drivers (CloudProxyDriver, DummyDriver, y cualquier driver local). Hacerlo primero en CORE-259 desbloquea todo lo demás.
