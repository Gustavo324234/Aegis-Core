# CORE-226 — fix(ank-core): Chat Agent usa SYSTEM_PROMPT_MASTER genérico en lugar de chat_agent.md

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

El Chat Agent (proceso principal del tenant, `ProcessRole::Operator`) **nunca recibe las
instrucciones de rol definidas en `kernel/config/agents/chat_agent.md`**. El path de
ejecución actual es:

```
HTTP request → PCB genérico → CognitiveScheduler → CognitiveHAL::route_and_execute()
             → CognitiveHAL::build_prompt() → SYSTEM_PROMPT_MASTER (hardcodeado)
```

El `InstructionLoader` y los archivos `*.md` **sólo son usados por el `AgentOrchestrator`**
cuando crea nodos internos (ProjectSupervisor, Specialist, etc.), pero el Chat Agent principal
nunca pasa por ese path.

Evidencia en los logs (2026-04-28): el Chat Agent responde como asistente genérico
("¡Hola! Soy **Aegis**, tu asistente profesional y preciso.") en lugar de seguir las
instrucciones de `chat_agent.md` que incluyen lógica de Dispatch y Query.

---

## Causa raíz

`CognitiveHAL::build_prompt()` en `kernel/crates/ank-core/src/chal/mod.rs` usa
`SYSTEM_PROMPT_MASTER` (string hardcodeado en el mismo archivo) para todos los procesos,
sin distinción de rol.

El `AgentOrchestrator` tiene un `InstructionLoader` funcional, pero no está conectado al
path de ejecución del Chat Agent.

---

## Cambios requeridos

### 1. Conectar `InstructionLoader` al path del Chat Agent en `CognitiveHAL::build_prompt()`

En `kernel/crates/ank-core/src/chal/mod.rs`:

**a)** Agregar un campo `instruction_loader: Arc<RwLock<InstructionLoader>>` a `CognitiveHAL`.
Inicializarlo en `CognitiveHAL::new()` usando `InstructionLoader::default_from_workspace()`
con la variable de entorno `AEGIS_DATA_DIR` como workspace root, igual que hace
`AgentOrchestrator::new()`.

**b)** En `build_prompt()`, antes de construir el prompt, detectar si el PCB corresponde
al Chat Agent (ausencia de `agent_id` en el PCB, o `task_type == TaskType::Chat`, o
`role == ProcessRole::Operator`). Si es el Chat Agent, reemplazar `SYSTEM_PROMPT_MASTER`
por `instruction_loader.instructions_for(&AgentRole::ChatAgent)`.

**c)** Si el PCB tiene `agent_id` seteado (es un agente del árbol), el `InstructionLoader`
ya está siendo usado correctamente vía `AgentOrchestrator`. No cambiar ese path.

### 2. Logging de diagnóstico

En `build_prompt()`, agregar un log `INFO` que confirme qué instrucciones base se usaron:

```rust
info!(
    pid = %pcb.pid,
    source = "chat_agent.md",  // o "SYSTEM_PROMPT_MASTER" si es fallback
    chars = instructions.len(),
    "build_prompt: role instructions loaded"
);
```

### 3. Verificar que `SYSTEM_PROMPT_MASTER` queda como fallback

`SYSTEM_PROMPT_MASTER` se mantiene en el código como fallback si `InstructionLoader`
falla. El fallback compilado con `include_str!` garantiza que la constante `SYSTEM_PROMPT_MASTER`
puede ser eventualmente eliminada o reducida, pero por ahora se mantiene.

---

## Criterios de aceptación

- [x] Los logs de arranque muestran (nivel INFO): `build_prompt: role instructions loaded source=chat_agent.md` para procesos del Chat Agent
- [x] Al enviar el mensaje "hola" al Chat Agent, el prompt enviado al LLM contiene el texto de `chat_agent.md` (verificable bajando log level a DEBUG)
- [x] Al enviar "trabajemos en el proyecto Aegis", el Chat Agent emite un syscall de Dispatch en lugar de responder como asistente genérico
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

Ninguna. Este ticket es bloqueante para CORE-227 y CORE-228.
