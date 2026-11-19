# CORE-274 — WebSocket: evento `agent_event` en el protocolo existente

**Tipo:** feat  
**Prioridad:** Crítica — prerrequisito de CORE-268  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer + Shell Engineer  
**Componentes:**
- `kernel/crates/ank-http/src/ws/chat.rs`
- `shell/ui/src/` (store o service que maneja el WebSocket)
**Estado:** ✅ Completo

---

## Contexto

Al revisar `ws/chat.rs` se confirma que el protocolo WebSocket ya tiene un
envelope tipado:

```json
{ "event": "kernel_event", "data": { "output": "token" } }
{ "event": "syslog",       "data": "mensaje del sistema" }
{ "event": "music_play",   "data": { "provider": "...", "track_id": "..." } }
{ "event": "music_control","data": { "action": "pause" } }
```

La preocupación de CORE-268 — que los eventos de agente y los tokens de texto
se mezclarían sin discriminador — **no aplica**. El protocolo ya los separa por
`event`. Agregar eventos de agente es trivial: un nuevo valor `"agent_event"`.

Este ticket define ese valor y especifica exactamente cómo se serializa, para que
kernel y shell lo implementen de forma consistente.

---

## Definición del nuevo evento

### Kernel — nuevo `event` en los frames WebSocket

```json
{ "event": "agent_event", "data": { ...AgentEvent... } }
```

Los `AgentEvent` que CORE-268 define siguen siendo los mismos. Lo único que
cambia es cómo se emiten: en lugar de un frame JSON crudo, van envueltos en
el envelope estándar.

En `ws/chat.rs`, la task que escucha `agent_event_rx` (CORE-268) debe emitir:

```rust
let frame = json!({
    "event": "agent_event",
    "data": serde_json::to_value(&event)?
});
let _ = ws_tx.send(Message::Text(frame.to_string())).await;
```

No como:
```rust
// INCORRECTO — frame crudo sin envelope
let _ = ws_tx.send(Message::Text(serde_json::to_string(&event)?)).await;
```

### Shell — parsing en el handler WebSocket

El handler existente ya tiene un `match` sobre `event`. Agregar el arm:

```typescript
case 'agent_event':
  handleAgentEvent(msg.data);
  break;
```

Donde `handleAgentEvent` delega al `agentInboxStore` (CORE-269):

```typescript
function handleAgentEvent(data: AgentEventPayload) {
  switch (data.type) {
    case 'supervisor_question':
      useAgentInboxStore.getState().addMessage({
        agentId: data.agent_id,
        projectName: data.project_name,
        question: data.question,
        context: data.context,
        timestamp: data.timestamp,
        status: 'pending',
      });
      break;
    case 'supervisor_resumed':
      // No action needed here — markAnswered se llama cuando el usuario responde
      break;
    case 'supervisor_completed':
      useAgentInboxStore.getState().markAnswered(data.agent_id);
      break;
    case 'supervisor_timed_out':
      useAgentInboxStore.getState().markTimedOut(data.agent_id);
      break;
  }
}
```

### Tipos TypeScript

```typescript
// shell/ui/src/types/agentEvents.ts
export type AgentEventPayload =
  | {
      type: 'supervisor_question';
      agent_id: string;
      project_name: string;
      question: string;
      context?: string;
      timestamp: string;
    }
  | { type: 'supervisor_resumed'; agent_id: string }
  | { type: 'supervisor_completed'; agent_id: string; project_name: string; summary: string }
  | { type: 'supervisor_timed_out'; agent_id: string; project_name: string };
```

---

## Lo que este ticket resuelve en CORE-268

CORE-268 puede simplificarse: no necesita preocuparse por el formato del frame.
Solo necesita usar el envelope `{ "event": "agent_event", "data": ... }` al emitir.
Los criterios de aceptación de CORE-268 se mantienen igual, pero el código de
emisión usa el patrón de este ticket.

---

## Criterios de aceptación

- [x] Los frames de tipo `agent_event` llegan al WebSocket con el envelope
  `{ "event": "agent_event", "data": { "type": "...", ... } }`
- [x] La shell parsea `agent_event` en el mismo switch/match que maneja
  `kernel_event`, `syslog`, `music_play`
- [x] `AgentEventPayload` está tipado en TypeScript
- [x] `supervisor_timed_out` está incluido como variante (ver mejora de CORE-268)
- [x] No hay cambios de ruptura en los eventos existentes
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-268 (define `AgentEvent` en Rust — este ticket define cómo se serializa al WS)
- CORE-269 (define `agentInboxStore` — este ticket define cómo lo alimenta el WS handler)

---

*Nota: Este ticket nació del análisis de `ws/chat.rs`. El problema de mezcla de
frames que motivó su creación no existe — el protocolo ya tenía envelope. El valor
de este ticket es documentar el contrato explícitamente para que kernel y shell
lo implementen de forma consistente sin asumir nada.*
