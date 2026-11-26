# CORE-373: Kernel Multi-Agent Auto-Dispatch & Direct Supervisor Execution (`ank-core`)

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 58
- **Asignado a:** Kernel Engineer

---

## Description
Los subagentes creados mediante `spawn_agent` en `ank-core` quedaban bloqueados esperando en `rx.recv()` porque `AgentToolCall::Spawn` solo registraba el nodo sin enviarle su tarea inicial (`dispatch`), mientras que `ProjectSupervisor` carecía de herramientas directas (`read_file`, `write_file`, `execute_command`, `web_search`) y recibía `SPAWN_INSTRUCTIONS` duplicadas y contradictorias en `chal/mod.rs`.

## Acceptance Criteria
- [x] Invocar `self.dispatch(agent_id, scope.clone(), vec![]).await` automáticamente tras `self.spawn(...)` en `kernel/crates/ank-core/src/agents/orchestrator.rs`.
- [x] Otorgar herramientas de ejecución directa (`read_file`, `write_file`, `list_files`, `execute_command`, `web_search`) a `ProjectSupervisor` y `Supervisor` en `kernel/crates/ank-core/src/agents/tool_registry.rs` y actualizar los tests unitarios (`42 passed`).
- [x] Vaciar la constante duplicada `SPAWN_INSTRUCTIONS` en `kernel/crates/ank-core/src/chal/mod.rs` y actualizar `kernel/config/agents/chat_agent.md` y `project_supervisor.md`.
