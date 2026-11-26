# CORE-241 — fix(ank-core): filtrar __TOOL_CALL__ del output antes de enviarlo al frontend

**Tipo:feat
**Prioridad:Alta
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

Con CORE-240 implementado, el `CloudProxyDriver` emite tool calls con el prefijo
`__TOOL_CALL__` para que el `SyscallExecutor` los identifique y despache como
`AgentToolCall`. El mecanismo funciona correctamente en el kernel.

Sin embargo, el output del proceso se envía al frontend tal cual, incluyendo
el prefijo `__TOOL_CALL__` y el JSON del tool call. El usuario ve:

```
TOOL_CALL{"arguments":{"name":"Aegis","role":"project_supervisor",...},"name":"spawn_agent"}
```

Esto nunca debe ser visible al usuario.

---

## Comportamiento esperado

Cuando el output de un proceso comienza con `__TOOL_CALL__`:

1. El `SyscallExecutor` (o el runner que entrega el output al Scheduler) detecta el prefijo
2. Deserializa el JSON como `AgentToolCall`
3. Ejecuta la acción (spawn, query, report)
4. **No envía el `__TOOL_CALL__` al frontend**
5. Una vez ejecutada la acción, el Chat Agent recibe el `tool_result` y genera
   una respuesta de texto que sí se envía al usuario

---

## Cambios requeridos

### 1. Filtro en el HAL Runner o en el Scheduler

En el punto donde el output del proceso se entrega al canal de respuesta
(el `Sender` del `ProcessCompleted`), interceptar outputs que empiecen con
`__TOOL_CALL__`:

```rust
if output.starts_with("__TOOL_CALL__") {
    // Parsear y ejecutar el tool call
    let json = &output["__TOOL_CALL__".len()..];
    if let Ok(tool_call) = serde_json::from_str::<AgentToolCallPayload>(json) {
        syscall_executor.dispatch(tool_call).await;
    }
    // No propagar el output al frontend — retornar vacío o esperar el tool_result
    return;
}
```

### 2. Respuesta al usuario post-tool call

Después de ejecutar el tool call, el Chat Agent debe recibir el `tool_result`
y generar una respuesta de texto visible al usuario. La respuesta debe ser
concisa — por ejemplo:

- Spawn exitoso: "Listo, activé el equipo para el proyecto Aegis."
- Spawn fallido: "No pude activar el equipo. ¿Querés intentarlo de nuevo?"

Este comportamiento ya está parcialmente implementado en el Orchestrator
(el `tool_result` se devuelve al modelo). Verificar que el segundo turno
del modelo genere texto y que ese texto sí llegue al frontend.

---

## Criterios de aceptación

- [ ] El usuario nunca ve `__TOOL_CALL__` ni JSON de tool calls en la UI
- [ ] Después de un spawn exitoso, el usuario recibe un mensaje de texto confirmando la acción
- [ ] El `AgentNode` del ProjectSupervisor aparece en el `AgentTreeView` del dashboard
- [ ] Los logs muestran el AgentNode creado con su ID
- [ ] `cargo build` pasa sin warnings nuevos
