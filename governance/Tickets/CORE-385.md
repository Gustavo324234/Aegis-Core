# CORE-385: Puente Bidireccional con Antigravity IDE vía Lifecycle Hooks (`Stop`/`PreInvocation`) y Endpoints `/api/antigravity/*` (`ank-http`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 59
- **Asignado a:** Kernel Engineer

---

## Description
Además de las sesiones autónomas lanzadas mediante el SDK de Python (`CORE-383`/`CORE-384`), el usuario desea poder interactuar desde cualquier lugar (por ejemplo, hablándole por voz a su asistente personal desde el celular o navegador) con la sesión que ya tiene abierta en **Antigravity IDE / Desktop**:
1. Que cuando Antigravity IDE termine una respuesta o haga una pregunta, el asistente personal se lo comunique al usuario (y se lo lea en voz alta si tiene la voz activada).
2. Que cuando el usuario le diga a su asistente *"Decile a Antigravity que avance con la opción A"*, ese mensaje llegue e impulse automáticamente el siguiente turno de Antigravity IDE.

## Archivos a Crear / Modificar
1. **`kernel/crates/ank-http/src/routes/antigravity_bridge.rs`** (Nuevo):
   - Montado bajo `/api/antigravity` en `kernel/crates/ank-http/src/routes/mod.rs`.
   - `GET /api/antigravity/status` (`CitadelAuthenticated`):
     - Retorna `{ "sdk_available": bool, "sdk_version": Option<String>, "use_antigravity_sdk": bool, "active_ide_conversations": [...], "pending_outbox_count": usize }`.
   - `POST /api/antigravity/config` (`CitadelAuthenticated`):
     - Permite activar/desactivar `use_antigravity_sdk` en `AgentOrchestrator`.
   - `POST /api/antigravity/ide/notify` (Autenticado por token local / `CitadelAuthenticated`):
     - Recibe desde el hook `Stop` de Antigravity IDE: `{ "tenant_id": "...", "conversation_id": "...", "summary": "...", "last_assistant_message": "..." }`.
     - Emite un evento `AgentEvent::SupervisorCompleted` (o notificación de chat con `speak: true`) hacia el WebSocket del tenant para que aparezca en el chat del asistente y se reproduzca por voz natural.
   - `POST /api/antigravity/ide/outbox` (`CitadelAuthenticated`):
     - Encola un mensaje del usuario dirigido a Antigravity IDE (`{ "message": "..." }`).
   - `GET /api/antigravity/ide/outbox/pop`:
     - Consultado por el hook de Antigravity IDE; si hay un mensaje encolado por el usuario a través de su asistente, lo extrae y lo devuelve para inyectarlo en el bucle de ejecución de Antigravity.
2. **`kernel/crates/ank-core/src/agents/tool_registry.rs` y `kernel/crates/ank-core/src/chal/mod.rs`**:
   - Añadir herramienta `send_to_antigravity(instruction: string)` al rol `ChatAgent` para que cuando el usuario diga *"Decile a Antigravity que..."*, el asistente despache la instrucción al puente de Antigravity (o al `AntigravityRunner` activo) y confirme verbalmente al usuario.
3. **`tools/antigravity_bridge/ide_hook.py`**:
   - Script para `.agents/hooks.json` compatible con el contrato `Stop` y `PreInvocation` de Antigravity:
     - En `Stop`: lee `transcriptPath` del JSON de `stdin`, extrae la última respuesta del modelo, la envía a `POST /api/antigravity/ide/notify` y consulta si hay respuesta pendiente en `outbox/pop` (si la hay, devuelve `{"decision": "continue", "reason": "Respuesta del usuario vía Asistente: ..."}`).

## Acceptance Criteria
- [ ] `GET /api/antigravity/status` y `POST /api/antigravity/config` funcionan autenticados con Citadel.
- [ ] `POST /api/antigravity/ide/notify` publica el mensaje de Antigravity en el chat del usuario en tiempo real.
- [ ] La herramienta `send_to_antigravity` está registrada en `ToolRegistry::definitions_for(&AgentRole::ChatAgent)` y permite enviar instrucciones a Antigravity desde una conversación hablada o escrita.
