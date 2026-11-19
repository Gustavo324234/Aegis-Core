# CORE-383: Python Worker Daemon para `google-antigravity` SDK con Streaming de Thoughts y Tool Calls (`tools/antigravity_bridge/`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 59
- **Asignado a:** AI Architect / Python Engineer

---

## Description
Para que el asistente personal pueda delegar tareas complejas (programación, análisis de repositorios, ejecución de comandos, investigación multi-paso) aprovechando los créditos incluidos en la cuenta de **Google Antigravity** del usuario en lugar de consumir su API key personal en árboles de subagentes internos, necesitamos un worker persistente en Python que envuelva el SDK oficial `google-antigravity` (`pip install google-antigravity`).

El worker debe mantener instancias activas de `Agent` indexadas por `session_id` (para permitir repreguntas y conversación continua sobre la misma tarea) y comunicarse con el kernel Rust (`ank-core`) mediante JSON-Lines (`NDJSON`) por `stdin`/`stdout`.

## Archivos a Crear
1. **`tools/antigravity_bridge/requirements.txt`**:
   ```text
   google-antigravity>=0.1.23
   ```
2. **`tools/antigravity_bridge/agy_worker.py`**:
   - Lee comandos línea por línea en JSON desde `sys.stdin`:
     - **Comando `ping` / `status`**:
       `{"id": "req-1", "cmd": "status"}`
       Responde: `{"id": "req-1", "event": "status", "sdk_installed": true, "version": "0.1.23"}`
     - **Comando `chat` (iniciar o continuar sesión de agente)**:
       ```json
       {
         "id": "req-2",
         "cmd": "chat",
         "session_id": "proj-123",
         "workspace": "e:/Aegis/Aegis-Core",
         "system_instructions": "Sos el agente ejecutor de Antigravity asistiendo a Aegis...",
         "prompt": "Analizá los tests del módulo X y corregí el fallo",
         "allow_write": true
       }
       ```
     - **Comando `cancel`**:
       `{"id": "req-3", "cmd": "cancel", "session_id": "proj-123"}`
     - **Comando `close_session`**:
       `{"id": "req-4", "cmd": "close_session", "session_id": "proj-123"}`
   - Para cada comando `chat`:
     - Si `session_id` no existe en el diccionario `active_sessions`, inicializa `LocalAgentConfig(system_instructions=..., capabilities=CapabilitiesConfig() if allow_write else None)` y abre el contexto `Agent(config)` (entrando con `await agent.__aenter__()` para mantener la sesión viva entre turnos).
     - Llama a `response = await agent.chat(prompt)`.
     - Ejecuta tareas concurrentes (`asyncio.gather`) o iteración sobre:
       - `response.thoughts` $\rightarrow$ emite por `stdout`:
         `{"id": "...", "session_id": "...", "event": "thought", "text": "..."}`
       - `response.tool_calls` $\rightarrow$ emite por `stdout`:
         `{"id": "...", "session_id": "...", "event": "tool_call", "name": call.name, "args": call.args}`
       - `response` (tokens de respuesta final) $\rightarrow$ acumula y emite deltas:
         `{"id": "...", "session_id": "...", "event": "token", "text": "..."}`
     - Al terminar el stream del turno, emite:
       `{"id": "...", "session_id": "...", "event": "completed", "full_text": "..."}`
   - Si el paquete `google-antigravity` no está instalado o falla la inicialización del binario, devuelve un evento estructurado `{"id": "...", "session_id": "...", "event": "error", "code": "SDK_NOT_AVAILABLE", "message": "google-antigravity SDK no está instalado. Ejecutá: pip install google-antigravity"}` sin colgar el proceso.
3. **`tools/antigravity_bridge/test_agy_worker.py`**:
   - Tests unitarios y de integración (con mock de `google.antigravity.Agent` para CI sin credenciales y modo real si el SDK está presente) verificando:
     - Ciclo `status` $\rightarrow$ `chat` (turno 1) $\rightarrow$ `chat` (turno 2 reusando la misma sesión `session_id`) $\rightarrow$ `close_session`.
     - Emisión correcta de líneas JSON para `thought`, `tool_call`, `token` y `completed`.

## Acceptance Criteria
- [ ] `python tools/antigravity_bridge/agy_worker.py` responde al comando `{"id":"1","cmd":"status"}` en menos de 1 segundo indicando si `google-antigravity` está instalado.
- [ ] Reutiliza la misma instancia `Agent` cuando se envían múltiples mensajes con el mismo `session_id`, preservando la memoria de la sesión de Antigravity.
- [ ] `pytest tools/antigravity_bridge/test_agy_worker.py` (o `unittest`) pasa al 100%.
