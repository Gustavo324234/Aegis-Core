# CORE-227 — fix(ank-core): SPAWN_INSTRUCTIONS usa sintaxis obsoleta — divergencia con parser

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

El prompt embebido `SPAWN_INSTRUCTIONS` en `kernel/crates/ank-core/src/chal/mod.rs`
enseña al LLM a emitir esta sintaxis para spawn de agentes:

```
[SYS_CALL_SPAWN("descripción de la tarea", "rol del agente")]
```

Pero el parser (`parse_syscall()` en `kernel/crates/ank-core/src/syscalls/mod.rs`)
parsea `SYS_CALL_SPAWN` hacia `Syscall::Spawn` — una variante **diferente** a la que
conecta con el `AgentOrchestrator`. La variante correcta de Epic 45 es `Syscall::AgentSpawn`,
que tiene su propio regex `AGENT_SPAWN_RE` y espera:

```
[SYS_AGENT_SPAWN(role="supervisor", name="...", scope="...", task_type="code")]
[SYS_AGENT_SPAWN(role="specialist", scope="leer scheduler/mod.rs")]
```

Adicionalmente, el `chat_agent.md` usa `SYS_AGENT_DISPATCH(...)` — una sintaxis que
**no existe en ningún regex** del parser. El LLM la emitió en los logs del 2026-04-28
y el Kernel la ignoró silenciosamente.

Hay también una instrucción `SYS_AGENT_QUERY` en `chat_agent.md` que sí tiene regex
(`AGENT_QUERY_RE`) y sí está conectada.

---

## Causa raíz

`SPAWN_INSTRUCTIONS` y `chat_agent.md` definen diferentes sintaxis de syscall que no
coinciden con los regexes del parser ni entre sí. Acumulación de deuda técnica entre
Epic 42 (CORE-154, `SYS_CALL_SPAWN`) y Epic 45 (CORE-198, `SYS_AGENT_SPAWN`).

---

## Cambios requeridos

### 1. Reemplazar `SYS_AGENT_DISPATCH` en `chat_agent.md` por la sintaxis correcta

**Archivo:** `kernel/config/agents/chat_agent.md`

Reemplazar todas las referencias a `SYS_AGENT_DISPATCH(...)` por la syscall real.
El Chat Agent para despachar trabajo a un ProjectSupervisor debe usar:

```
[SYS_AGENT_SPAWN(role="supervisor", name="<nombre del proyecto>", scope="<descripción de la tarea>", task_type="planning")]
```

Si el proyecto ya tiene un ProjectSupervisor activo (ya fue activado), el Chat Agent
debe delegar vía Dispatch interno — documentar este caso en el `.md` como:

```
[SYS_AGENT_SPAWN(role="specialist", scope="<descripción de la tarea>")]
```

Actualizar todos los ejemplos del archivo para que usen la sintaxis correcta.

### 2. Actualizar `SPAWN_INSTRUCTIONS` en `chal/mod.rs`

Reemplazar el contenido de la constante `SPAWN_INSTRUCTIONS` para que:

- Elimine la referencia a `[SYS_CALL_SPAWN(...)]` (sintaxis obsoleta de Epic 42)
- Documente únicamente `[SYS_AGENT_SPAWN(...)]` con los formatos de Epic 45
- Aclare que `SYS_CALL_SPAWN` ya no está soportado

### 3. Mantener backward compatibility del parser para `Syscall::Spawn`

**No eliminar** el regex `SPAWN_RE` ni el match `Syscall::Spawn` del ejecutor todavía.
Marcarlo como `// DEPRECATED (Epic 42) — eliminar en post-launch` para que el Kernel
Engineer que trabaje en limpieza de deuda lo encuentre.

---

## Criterios de aceptación

- [x] `chat_agent.md` no contiene referencias a `SYS_AGENT_DISPATCH`
- [x] `chat_agent.md` usa `SYS_AGENT_SPAWN` con la sintaxis exacta que parsea `AGENT_SPAWN_RE`
- [x] `SPAWN_INSTRUCTIONS` en `chal/mod.rs` documenta únicamente `SYS_AGENT_SPAWN`
- [x] Al enviar "generá un subagente para el proyecto Aegis", el LLM emite `[SYS_AGENT_SPAWN(...)]` y el parser lo captura correctamente
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

CORE-226 debe completarse primero (para que el Chat Agent reciba `chat_agent.md`).
