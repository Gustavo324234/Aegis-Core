# CORE-389: CRUD Completo de Recordatorios e Historial de Chat Infinito con Limpieza Visual (`ank-core`, `ank-http`, `shell/ui`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 60
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description
Se detectaron dos necesidades clave en el uso diario del chat único infinito:
1. Los recordatorios solo soportaban alta, listado y marcado como completado (`add`, `list`, `complete`), impidiendo **editar o borrar** duplicados reales y dificultando rutinas recurrentes semanales/diarias.
2. El chat único acumulaba todos los turnos en pantalla al abrir la app; se necesitaba poder **limpiar visualmente** la pantalla manteniendo el historial guardado y cargando conversaciones previas al subir el scroll.

## Archivos Creados / Modificados
1. **`kernel/crates/ank-core/src/enclave/mod.rs` y `kernel/crates/ank-core/src/agents/tool_registry.rs`**:
   - Ampliado `manage_reminder` con acciones `update` y `delete` además de `add`, `list`, `complete` y campo `recurrence` (`none`, `daily`, `weekdays`, `weekly:...`, `monthly`).
   - Implementados `add_reminder_with_recurrence`, `update_reminder`, `delete_reminder`, `complete_reminder` y `fire_due_reminders` en `TenantDB`.
2. **`kernel/crates/ank-http/src/routes/personal.rs`**:
   - Endpoints `DELETE /api/personal/reminders/:id`, `POST /api/personal/reminders/:id/complete` y `GET /api/chronos/events`.
3. **`kernel/crates/ank-http/src/routes/chat_history.rs` y `shell/ui/src/components/ChatTerminal.tsx`**:
   - Añadida paginación por índice en `GET /api/chat/history` (`limit`, `before`, `respect_clean`), estado de vista en `GET/PATCH /api/chat/ui-state` (`visual_clear_before`, `clean_on_open`) y limpieza no destructiva en `POST /api/chat/visual-clear`.
   - Botón `"Limpiar vista"` (`clearChatView`), toggle de `"Al abrir: Vista limpia..."` (`setCleanOnOpen`) y carga progresiva hacia arriba (`HistoryLoadButton` / `triggerLoadOlder` por scroll/wheel/touch).

## Acceptance Criteria
- [x] `manage_reminder` permite agregar, listar, modificar, completar y eliminar recordatorios/rutinas recurrentes.
- [x] Pulsar `"Limpiar vista"` oculta visualmente los mensajes anteriores sin borrarlos de `chat_history.log`.
- [x] Al subir al inicio del chat o pulsar el botón de historial anterior, se cargan progresivamente las páginas guardadas.
