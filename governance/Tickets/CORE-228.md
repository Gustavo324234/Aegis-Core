# CORE-228 — fix(ank-server): SyscallExecutor se crea sin AgentOrchestrator — SYS_AGENT_SPAWN siempre falla

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

En `kernel/crates/ank-server/src/main.rs`, el `SyscallExecutor` se instancia dentro
del HAL Runner loop con:

```rust
let executor = ank_core::syscalls::SyscallExecutor::new(
    plugin_manager,
    vcm,
    scribe_runner,
    swap,
    mcp_registry,
    http_client,
    scheduler_tx_runner.clone(),
);
```

El método `.with_orchestrator(orchestrator)` **nunca es llamado**. Esto significa que
`self.agent_orchestrator` es siempre `None` en el ejecutor, y cualquier syscall
`Syscall::AgentSpawn` o `Syscall::AgentQuery` falla con:

```
SyscallError::InternalError("AgentOrchestrator not configured — SYS_AGENT_SPAWN unavailable")
```

El `AgentOrchestrator` sí se inicializa correctamente más arriba en `main.rs` y se
pasa al `AppState`, pero nunca se propaga al `SyscallExecutor`.

---

## Causa raíz

Omisión en `main.rs`: al construir el `SyscallExecutor` dentro del HAL Runner loop,
no se llama a `.with_orchestrator(Arc::clone(&agent_orchestrator))`.

---

## Cambios requeridos

### 1. Pasar `agent_orchestrator` al `SyscallExecutor`

**Archivo:** `kernel/crates/ank-server/src/main.rs`

En el bloque del HAL Runner (tokio::spawn del loop `while let Some(pcb)`), capturar
el `Arc<AgentOrchestrator>` del scope externo y pasarlo al ejecutor.

Antes de entrar al loop, clonar el Arc:
```rust
let agent_orchestrator_for_hal = Arc::clone(&agent_orchestrator);
```

Dentro del spawn interno (el que procesa cada PCB), clonar nuevamente:
```rust
let orchestrator_clone = Arc::clone(&agent_orchestrator_for_hal);
```

Al construir el executor, agregar:
```rust
let executor = ank_core::syscalls::SyscallExecutor::new(
    plugin_manager,
    vcm,
    scribe_runner,
    swap,
    mcp_registry,
    http_client,
    scheduler_tx_runner.clone(),
)
.with_orchestrator(orchestrator_clone);  // ← AGREGAR ESTA LÍNEA
```

### 2. Logging de confirmación

En `SyscallExecutor::with_orchestrator()` (en `syscalls/mod.rs`), agregar un log INFO:

```rust
tracing::info!("SyscallExecutor: AgentOrchestrator connected. SYS_AGENT_SPAWN ready.");
```

---

## Criterios de aceptación

- [x] Los logs de arranque muestran: `SyscallExecutor: AgentOrchestrator connected. SYS_AGENT_SPAWN ready.`
- [x] Al ejecutarse un `SYS_AGENT_SPAWN`, el log muestra `[SPAWN] Agent spawned.` en lugar de error "AgentOrchestrator not configured"
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

CORE-226 y CORE-227 deben completarse primero para que el LLM emita `SYS_AGENT_SPAWN`
con la sintaxis correcta y así poder verificar este ticket end-to-end.
