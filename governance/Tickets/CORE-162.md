# CORE-162 — feat: SYS_AGENT_SPAWN — Spawn Dinámico de Agentes desde Syscall

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-158
**Estado:** ✅ Completo

---

## Contexto

Un agente en ejecución puede determinar que necesita subordinados especializados. Por ejemplo, un `ProjectSupervisor` procesando una tarea compleja puede decidir crear un nuevo `DomainSupervisor` de "devops" que antes no existía.

Este ticket implementa `SYS_AGENT_SPAWN`: una syscall cognitiva que un agente puede invocar (a través de su output estructurado) para solicitar la creación de un nuevo agente subordinado.

El mecanismo se integra con el sistema de syscalls existente en `ank-core/src/syscalls/`.

---

## Cambios Requeridos

### 1. Definir la syscall en `ank-core/src/syscalls/`

Agregar al enum o mapa de syscalls existente:

```rust
/// SYS_AGENT_SPAWN: un agente solicita crear un subordinado.
/// Parámetros:
///   - role: "PROJECT_SUPERVISOR" | "DOMAIN_SUPERVISOR" | "SPECIALIST"
///   - domain: nombre del dominio (ej: "devops", "frontend", "testing")
///   - task_type: tipo cognitivo del nuevo agente (ver TaskType)
///   - system_prompt_hint: descripción del rol para generar el prompt base
pub const SYS_AGENT_SPAWN: &str = "SYS_AGENT_SPAWN";
```

### 2. Implementar el handler de la syscall

El handler debe:
1. Validar que el agente solicitante existe en el `AgentTree`
2. Validar que el `role` solicitado es igual o inferior al del solicitante (un Specialist no puede crear DomainSupervisors; un DomainSupervisor no puede crear ProjectSupervisors)
3. Llamar a `AgentOrchestrator::spawn_agent()` con el `parent_id` del agente solicitante
4. Retornar el `AgentId` del nuevo agente como resultado de la syscall

### 3. Parsing del output del LLM en `run_agent_loop`

El `run_agent_loop` del `AgentOrchestrator` (CORE-158) debe detectar en el output del LLM si hay una solicitud de spawn. El formato esperado en el output del LLM:

```json
{
  "syscall": "SYS_AGENT_SPAWN",
  "params": {
    "role": "SPECIALIST",
    "domain": "rust-testing",
    "task_type": "CODE",
    "system_prompt_hint": "Especialista en tests de integración en Rust"
  }
}
```

Si el output contiene este bloque JSON, el loop lo parsea y ejecuta la syscall antes de continuar con el texto normal.

---

## Criterios de Aceptación

- [x] Un agente con rol `DomainSupervisor` puede spawnear un `Specialist` exitosamente
- [x] Un agente con rol `Specialist` NO puede spawnear ningún agente (retorna error)
- [x] Un agente con rol `ProjectSupervisor` puede spawnear `DomainSupervisor` y `Specialist`
- [x] El nuevo agente spawneado queda registrado en el `AgentTree` con el `parent_id` correcto
- [x] El syscall handler retorna el `AgentId` del nuevo agente al agente solicitante
- [x] Si el output del LLM no contiene el bloque JSON de syscall, el loop funciona normalmente sin intentar parsear
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*