# CORE-253 — Kernel: SYS_CALL_PLUGIN con plugin no encontrado debe devolver error legible al usuario

**Tipo:** fix  
**Prioridad:** Crítica  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/syscalls/mod.rs` — `SyscallExecutor::execute` (arm `PluginCall`)
- `kernel/crates/ank-server/src/main.rs` — HAL Runner, manejo de `SyscallError`

**Estado:** ✅ Done
---

## Contexto

Durante el smoke test del 2026-05-03, el modelo generó correctamente un syscall:

```
[SYS_CALL_PLUGIN("ledger", {"action": "get_monthly_spent", "params": {"user_id": "tavo", "month": "2026-05"}})]
```

El `StreamInterceptor` lo detectó correctamente y el `SyscallExecutor` lo ejecutó. Pero el plugin `ledger` no está registrado como WASM en el `PluginManager`, lo que devuelve `Err(SyscallError::PluginError(...))`.

El HAL Runner no maneja ese error de forma amigable — el proceso completa con el texto crudo del syscall como output visible al usuario, o con output vacío. El usuario no recibe ninguna respuesta útil.

## Causa raíz

**Bug 1 — `SyscallExecutor` (arm `PluginCall`):** Cuando `pm.execute_plugin` falla porque el plugin no existe, se devuelve `Err(SyscallError::PluginError(...))`. El HAL Runner propaga ese error sin convertirlo en un mensaje legible para el usuario.

**Bug 2 — HAL Runner:** Cuando `execute()` devuelve un `SyscallError`, el runner no inyecta un mensaje de error en el stream. El proceso completa con el output siendo el texto previo al syscall (o vacío), sin ningún feedback al usuario.

## Cambios requeridos

### Fix 1 — `syscalls/mod.rs`, arm `PluginCall`

Cuando `execute_plugin` falla porque el plugin no existe o no está disponible, **no propagar el error** — en cambio devolver un `[SYSTEM_RESULT: ...]` informativo para que el modelo pueda responder al usuario:

```rust
Syscall::PluginCall { plugin_name, args_json } => {
    let pm = self.plugin_manager.read().await;
    match pm.execute_plugin(tenant_id, &plugin_name, &args_json, pcb.session_key.as_deref()).await {
        Ok(result) => Ok(format!("[SYSTEM_RESULT: {}]", result)),
        Err(crate::plugins::PluginError::PluginNotFound(_)) => {
            // Plugin no registrado — devolver resultado informativo en lugar de error fatal
            Ok(format!(
                "[SYSTEM_RESULT: Plugin '{}' no está disponible en este momento. \
                 El administrador debe activarlo en la configuración del sistema.]",
                plugin_name
            ))
        }
        Err(e) => Err(SyscallError::PluginError(e.to_string())),
    }
}
```

**Nota:** Verificar si `PluginManager` ya tiene una variante `PluginNotFound` en su enum de errores. Si no existe, usar pattern matching genérico sobre el mensaje de error o agregar la variante.

### Fix 2 — `ank-server/src/main.rs`, HAL Runner — manejo de `SyscallError`

Cuando el executor de syscalls devuelve un error (bugs reales, no "plugin not found"), el HAL Runner debe inyectar un mensaje amigable al stream antes de completar el proceso. Esto es complementario a CORE-244.

Agregar en el path de error del HAL donde se procesa el resultado de syscalls:

```rust
Err(e) => {
    warn!(pid = %pcb.pid, "Syscall execution failed: {}", e);
    // Inyectar mensaje amigable al event_broker
    let _ = event_tx.send(TaskEvent {
        pid: pcb.pid.clone(),
        payload: Some(Payload::Output(
            "Tuve un problema ejecutando esa acción. Intentá de nuevo.".to_string()
        )),
    });
    // Enviar StatusUpdate para desbloquear stream_with_receiver
    let _ = event_tx.send(TaskEvent {
        pid: pcb.pid.clone(),
        payload: Some(Payload::StatusUpdate(Box::new(Pcb {
            state: 4,
            ..Default::default()
        }))),
    });
}
```

## Criterios de aceptación

- [x] Cuando el modelo llama a un plugin no registrado, el proceso completa con un mensaje legible: "Plugin 'X' no está disponible en este momento."
- [x] El mensaje de plugin no disponible se ve en la UI del chat (no en el log del servidor únicamente).
- [x] Cuando cualquier syscall falla por error real (no "not found"), el usuario ve "Tuve un problema ejecutando esa acción." en el chat.
- [x] El proceso no completa con output vacío ni con el texto crudo del syscall.
- [x] `cargo build --workspace` sin errores.

## Dependencias

- CORE-244 (complementario — ambos tocan el path de error del HAL Runner)
