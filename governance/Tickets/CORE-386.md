# CORE-386: Panel de Control de Integración Antigravity SDK y Monitoreo en Vivo en `shell/ui`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 59
- **Asignado a:** Shell Engineer

---

## Description
El usuario necesita visualizar y controlar desde la interfaz web (`shell/ui`) el estado de la integración con **Google Antigravity** (si el SDK está instalado, si los subagentes están usando los créditos de Antigravity en lugar de la API externa, y qué está haciendo o pensando Antigravity en segundo plano), además de que las notificaciones entrantes de Antigravity sean leídas automáticamente por la voz natural del asistente.

## Archivos a Modificar
1. **`shell/ui/src/components/SettingsPanel.tsx`** (Pestaña `Motor`):
   - Añadir tarjeta destacada **"⚡ Delegación a Google Antigravity SDK (Subagentes y Tareas Complejas)"**:
     - Muestra estado del SDK (`GET /api/antigravity/status`): `✅ SDK Detectado` o instrucciones de 1 línea (`pip install google-antigravity`) si falta.
     - Switch para activar/desactivar `use_antigravity_sdk` (`POST /api/antigravity/config`), explicando que cuando está activo, el asistente delega el trabajo pesado a Antigravity aprovechando sus créditos sin gastar cuota de la API de chat.
2. **`shell/ui/src/components/PersonalAssistantDrawer.tsx`** (Pestaña `Segundo Plano`):
   - Mostrar insignia `⚡ Powered by Antigravity SDK` en las tareas delegadas y permitir enviar una instrucción directa a Antigravity desde un campo rápido (`POST /api/antigravity/ide/outbox` o `sendMessage`).
3. **`shell/ui/src/store/useAegisStore.ts`**:
   - En el manejador WebSocket de `agent_event` (`supervisor_completed` y `supervisor_question`), si `get().voiceEnabled` es `true`, invocar `speakNaturalText(...)` con el resumen o pregunta enviada por Antigravity para que el asistente se lo diga en voz alta al usuario aunque no esté mirando la pantalla.

## Acceptance Criteria
- [ ] En `Configuración -> Motor` se visualiza el estado de `google-antigravity` SDK y el switch para delegar subagentes a Antigravity.
- [ ] Cuando un subagente de Antigravity completa una tarea o hace una pregunta (`supervisor_completed` / `supervisor_question`) y la voz está activa, el asistente la lee en voz alta con su voz natural configurada.
- [ ] `npm run build` en `shell/ui` compila sin errores.
