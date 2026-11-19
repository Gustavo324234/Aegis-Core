# CORE-330 — Kernel/Shell: Clear logs and history via UI button

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer + Shell Engineer

---

## Problema

El historial de chat anterior y las trazas cognitivas se persisten en el servidor en `chat_history.log` y `agent_traces.log`. Estos archivos son leídos por el VCM (Virtual Context Manager) para alimentar el contexto de los agentes. Si hay errores o problemas anteriores en el historial, la IA puede verse influenciada negativamente o confundirse en las sesiones subsiguientes. 

Actualmente, no hay forma de limpiar estas trazas y el historial directamente desde la interfaz gráfica. El botón `clearHistory` de la UI solo limpia el estado de React en memoria, pero al refrescar la página o reabrir el WebSocket, el historial del backend se vuelve a cargar.

## Solución propuesta

1. **Backend Endpoints:**
   - Implementar `DELETE /api/chat/history` en `chat_history.rs` para truncar `chat_history.log`.
   - Implementar `DELETE /api/chat/traces` en `chat_history.rs` para truncar `agent_traces.log`.

2. **WebSocket Connection Handling:**
   - Soportar la acción `"clear_history"` en `ws/chat.rs` para vaciar el `session_history` de la conexión activa.

3. **Frontend Store:**
   - Modificar la acción `clearHistory` en `useAegisStore.ts` para realizar la llamada REST `DELETE` a `/api/chat/history` y enviar la acción WebSocket `"clear_history"`.
   - Agregar la acción `clearAgentTraces` en `useAegisStore.ts` para realizar la llamada REST `DELETE` a `/api/chat/traces`.

4. **UI Button:**
   - Agregar un botón "Borrar logs" con el ícono `Trash2` de `lucide-react` en la pestaña de Logs de `SettingsPanel.tsx`.
   - Mostrar un confirm modal antes de proceder.
   - Refrescar la vista tras borrar.

## Criterios de aceptación

- [ ] Un usuario puede hacer clic en "Borrar logs" en el historial de chat y ver cómo se limpia en pantalla y en el servidor.
- [ ] Un usuario puede hacer clic en "Borrar logs" en las trazas cognitivas y ver cómo se limpia en pantalla y en el servidor.
- [ ] Al recargar la página, el historial borrado no vuelve a aparecer.
- [ ] La IA del chat ya no recibe el contexto de mensajes anteriores tras borrar el historial.
- [ ] `cargo check --workspace` y `cargo test --workspace` compilan y pasan correctamente.
