# EPIC 59 — Antigravity SDK Delegation, Bidirectional IDE Bridge & Natural Voice/Identity Studio

**Estado:** ✅ Done

## 🎯 Objetivo

1. **Aprovechar los créditos y capacidades de Google Antigravity (`google-antigravity` SDK + Hooks)** como el motor de ejecución de subagentes/supervisores detrás del asistente personal: el asistente principal mantiene su respuesta instantánea en el chat (voz, memoria, finanzas), pero cuando necesita delegar tareas de código, investigación o análisis pesado (`spawn_agent` / `answer_supervisor`), utiliza sesiones del **Antigravity Python SDK** en lugar de quemar tokens de la API key externa del usuario.
2. **Puente Bidireccional Ubicuo (Hablar con Antigravity desde cualquier lugar)**: permitir que lo que Antigravity responde o pregunta sea comunicado (y leído en voz alta) por el asistente personal en la web o celular, y que el usuario pueda dictarle o escribirle al asistente qué responderle a Antigravity.
3. **Voz Natural (Masculina/Femenina) y Estudio de Identidad/Avatar (`CORE-382`)**: eliminar la complejidad de los modos de audio rotos y las voces robóticas SAPI5, ofreciendo voces neurales masculinas y femeninas, micrófono directo siempre activo y personalización completa de nombre, avatar (6 diseños + foto propia) y personalidad.

---

## 🏗️ Pilares y Criterios de Aceptación

### Pilar 1: Voz Natural (Masculina/Femenina) y Estudio de Personalización del Asistente
1. **Motor de Voz Natural y Estudio de Identidad (`CORE-382` — ✅ Done):**
   - Selección inteligente de voces neurales (`Elena`, `Tomás`, `Dalia`, `Álvaro`, `Google español`), soporte de género (`♀ Femenina` / `♂ Masculina`) y 4 perfiles de timbre.
   - Limpieza automática de Markdown/código/emojis antes de hablar y partición por oraciones para evitar el corte de 15s de Chrome.
   - Eliminación del selector confuso `[Text | Audio | Conv | Wake]` en `ChatTerminal.tsx`, dejando únicamente Parlante y Micrófono siempre activos.
   - Componente `AssistantAvatar.tsx` con 6 avatares diseñados + subida de foto propia, visible en cada mensaje del asistente y en el header superior.

### Pilar 2: Delegación de Subagentes al SDK Oficial de Google Antigravity (`google-antigravity`)
2. **Worker Daemon del SDK de Antigravity (`CORE-383` — ✅ Done):**
   - Servicio en Python (`tools/antigravity_bridge/agy_worker.py`) que envuelve `from google.antigravity import Agent, LocalAgentConfig, CapabilitiesConfig`.
   - Mantiene sesiones persistentes de `Agent` por `project_id`/`agent_id` para conservar el contexto en conversaciones de ida y vuelta, emitiendo en tiempo real `thoughts`, `tool_calls`, `tokens` y resultado final por protocolo JSON-Lines (`NDJSON`).
3. **Integración de `AntigravityRunner` en `AgentOrchestrator` (`CORE-384` — ✅ Done):**
   - Nuevo módulo `kernel/crates/ank-core/src/agents/antigravity_runner.rs` integrado en `orchestrator.rs`.
   - Cuando el asistente personal ejecuta `spawn_agent` o `answer_supervisor` y el modo Antigravity está activo, delega la tarea al `agy_worker.py` en lugar de consumir la API key del router en loops multi-nodo, transmitiendo el progreso al WebSocket del usuario y anunciando el resultado final en el chat (y por voz).

### Pilar 3: Puente en Vivo con Antigravity IDE y Control desde la Interfaz Web
4. **Puente Bidireccional con Antigravity IDE vía Lifecycle Hooks (`CORE-385` — ✅ Done):**
   - Endpoints en `ank-http` (`/api/antigravity/notify`, `/api/antigravity/reply`, `/api/antigravity/inbox`) y herramienta `send_to_antigravity` en `ToolRegistry`.
   - Script de hook `.agents/scripts/aegis_hook_bridge.py` para los eventos `Stop` y `PreInvocation` de Antigravity IDE, permitiendo que el asistente personal lea en voz alta lo que Antigravity dice y le inyecte las respuestas del usuario desde cualquier lugar.
5. **UI de Configuración y Monitoreo del Puente Antigravity (`CORE-386` — ✅ Done):**
   - Toggle en `SettingsPanel.tsx` y estado en vivo en `PersonalAssistantDrawer.tsx` para activar/desactivar la delegación a `google-antigravity` SDK, verificar si el SDK está instalado/autenticado y ver las acciones de Antigravity en tiempo real.

---

## 🛠️ Listado de Tickets Mapeados

* **CORE-382:** Motor de Voz Natural (Femenina/Masculina), Simplificación de Audio en Chat y Estudio de Avatar/Identidad (`shell/ui`) — ✅ Done
* **CORE-383:** Python Worker Daemon para `google-antigravity` SDK con Streaming de Thoughts y Tool Calls (`tools/antigravity_bridge/`) — ✅ Done
* **CORE-384:** Integración de `AntigravityRunner` en `AgentOrchestrator` para Delegación de Subagentes sin Consumo de API Externa (`ank-core`) — ✅ Done
* **CORE-385:** Puente Bidireccional con Antigravity IDE vía Lifecycle Hooks (`Stop`/`PreInvocation`) y Endpoints `/api/antigravity/*` (`ank-http`) — ✅ Done
* **CORE-386:** Panel de Control de Integración Antigravity SDK y Monitoreo en Vivo en `shell/ui` — ✅ Done
