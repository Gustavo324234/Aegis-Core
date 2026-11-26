# CORE-392: Failover Dinámico Multi-Proveedor en Bucle ReAct, Streaming SSE UTF-8 Seguro y Recuperación Post-Tool (`ank-core/chal`, `ank-core/router`)

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 60
- **Asignado a:** Kernel Engineer

---

## Description
Se corrigieron cuatro fallos críticos detectados durante conversaciones reales con streaming y tool calls:
1. Confirmaciones cortas como `"ya está conectado"` activaban por error el atajo `is_status_query` (`starts_with("ya esta")`), devolviendo estado de subagentes en lugar de consultar `manage_whatsapp`.
2. Chunks SSE que partían caracteres UTF-8 multibyte (`á`, `ñ`, emojis) se decodificaban con pérdida por chunk en `cloud.rs`.
3. Ante errores de proveedor o respuestas vacías después de ejecutar una tool (`stream_was_empty`), el bucle ReAct volvía a elegir el mismo modelo por culpa de la sticky cache o fallaba sin rotar de proveedor.
4. Texto entre iteraciones con tool calls podía pegarse sin salto de línea ni saneamiento de tags internos.

## Archivos Creados / Modificados
1. **`kernel/crates/ank-core/src/chal/drivers/cloud.rs`**:
   - Búfer SSE migrado a `Vec<u8>` con `decode_next_sse_line_from_buf` para decodificar líneas completas solo al encontrar `\n` sin corromper secuencias UTF-8 partidas entre chunks (con test `decode_sse_lines_preserves_split_multibyte_utf8_chunks`).
2. **`kernel/crates/ank-core/src/router/mod.rs`**:
   - Agregado `CognitiveRouter::decide_excluding` (`decide_inner`) que omite modelos ya fallidos e ignora la sticky cache cuando la decisión anterior falla o queda excluida (con test `decide_excluding_bypasses_sticky_and_skips_failed_models`).
3. **`kernel/crates/ank-core/src/chal/mod.rs`**:
   - Failover dinámico de hasta 3 saltos usando `decide_excluding` ante errores primarios, errores durante streaming o respuestas vacías en cualquier iteración (`stream_was_empty`), reconstruyendo tools para el proveedor elegido (`build_tools_for`).
   - Añadido `StreamTextSanitizer`, `SanitizerMode`, `emit_stream_text_chunk`, `consume_recovery_stream` y separadores `\n\n` entre iteraciones.
   - Endurecida `is_status_query` para requerir frases específicas (`"ya esta"`, `"ya esta listo"`, `"ya esta terminad"`, `"ya estan listo"`) en lugar de `starts_with("ya esta")` (actualizado `test_chat_agent_guard_helpers`).

## Acceptance Criteria
- [x] `"ya está conectado"` continúa normalmente por el bucle conversacional y ejecuta `manage_whatsapp`.
- [x] Chunks SSE con caracteres multibyte partidos se ensamblan intactos (`cargo test -p ank-core --lib`).
- [x] Fallos de proveedor o streams vacíos post-tool rotan dinámicamente hacia modelos alternativos disponibles sin quedar atrapados en la sticky cache.
