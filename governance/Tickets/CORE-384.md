# CORE-384: Integración de `AntigravityRunner` en `AgentOrchestrator` para Delegación de Subagentes sin Consumo de API Externa (`ank-core`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 59
- **Asignado a:** Kernel Engineer

---

## Description
Actualmente, cuando el asistente principal (`ChatAgent`) invoca la herramienta `spawn_agent(name, scope, task_description)` o `answer_supervisor(agent_id, answer)`, `AgentOrchestrator` en `kernel/crates/ank-core/src/agents/orchestrator.rs` levanta un `run_agent_loop` que realiza múltiples llamadas de inferencia contra el `CognitiveRouter` / `CognitiveHAL`, consumiendo la cuota de la API key externa del usuario.

Con este ticket, `AgentOrchestrator` incorpora un ejecutor `AntigravityRunner` que delega los `ProjectSupervisor` / subagentes directamente al worker `tools/antigravity_bridge/agy_worker.py` (`CORE-383`), aprovechando los créditos de Google Antigravity del usuario y reportando el progreso y resultado final al chat del asistente personal.

## Archivos a Crear / Modificar
1. **`kernel/crates/ank-core/src/agents/antigravity_runner.rs`** (Nuevo):
   - Definir `pub struct AntigravityRunner`:
     - Mantiene un proceso hijo tokio (`tokio::process::Command`) ejecutando `python tools/antigravity_bridge/agy_worker.py` con `stdin` y `stdout` pipeados.
     - Si el proceso hijo no está iniciado o terminó, lo levanta bajo demanda (buscando primero `.venv-agy`, `.venv` o `python`/`python3` del sistema).
     - Método `pub async fn check_status(&self) -> AntigravitySdkStatus` (`installed: bool`, `version: Option<String>`, `error: Option<String>`).
     - Método `pub async fn dispatch_turn(&self, session_id: &str, workspace: &Path, system_instructions: &str, prompt: &str, on_event: impl FnMut(AgyStreamEvent)) -> anyhow::Result<String>`:
       - Envía el comando `{"cmd": "chat", ...}` por `stdin` y escucha los eventos de `stdout` filtrados por `session_id` / `id`.
       - Por cada `thought` o `tool_call`, invoca `on_event` para que el orquestador emita `AgentEvent::Activity` en vivo.
       - Retorna el `full_text` al recibir `event == "completed"`.
2. **`kernel/crates/ank-core/src/agents/mod.rs`**:
   - Exportar `pub mod antigravity_runner;`.
3. **`kernel/crates/ank-core/src/agents/orchestrator.rs`**:
   - Añadir campo `pub agy_runner: Arc<AntigravityRunner>` y flag configurable `use_antigravity_sdk: std::sync::atomic::AtomicBool` (inicializado en `true` si `AEGIS_SUBAGENT_BACKEND` es `"antigravity"` o por defecto si el SDK está disponible, con fallback limpio al loop tradicional si el SDK no está instalado).
   - En `run_agent_loop` (al recibir `AgentMessage::Dispatch` o `AgentMessage::UserReply`):
     - Si `use_antigravity_sdk` está activo y el SDK está disponible:
       1. Cambiar el estado del nodo a `AgentState::Running` y emitir `AgentEvent::StateChanged`.
       2. Llamar a `agy_runner.dispatch_turn(...)` pasando el `project_id` como `session_id`.
       3. Cada `thought` / `tool_call` del SDK se publica como `AgentEvent::Activity` (visible en vivo en el panel **"Mi Asistente $\rightarrow$ Segundo Plano"**).
       4. Si la respuesta de Antigravity termina con una pregunta abierta al usuario (o marcador `[ASK_USER: ...]`), cambiar a `AgentState::WaitingUser` y emitir `AgentEvent::SupervisorQuestion` para que el asistente le avise al usuario y espere su respuesta con `answer_supervisor`.
       5. Si la tarea concluyó, guardar el reporte en `node.last_report` y en el `ProjectLedger`, cambiar a `AgentState::Complete` y emitir `AgentEvent::SupervisorCompleted` (lo que inyecta automáticamente el resumen en el chat del asistente y lo reproduce con la voz natural si la voz está activa).

## Acceptance Criteria
- [ ] Cuando `use_antigravity_sdk` está habilitado, `spawn_agent` ejecuta la tarea a través de `AntigravityRunner` (`agy_worker.py`) sin hacer llamadas LLM adicionales al `CognitiveRouter`.
- [ ] Si el usuario responde mediante `answer_supervisor` (o pide un cambio sobre el mismo proyecto), se envía al mismo `session_id` de `AntigravityRunner` conservando el contexto.
- [ ] Si `google-antigravity` no está instalado en el sistema, `AntigravityRunner` informa claramente el estado o hace fallback controlado sin provocar pánico en el kernel (`cargo test -p ank-core --lib agents`).
