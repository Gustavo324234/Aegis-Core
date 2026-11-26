# CORE-372: Agent Sandbox & Personal Assistant Simulator (`tools/agent_sandbox/`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 58
- **Asignado a:** Arquitecto IA

---

## Description
Creación de un simulador interactivo y banco de pruebas aislado en `tools/agent_sandbox/` sin dependencias externas para iterar rápidamente sobre los prompts, herramientas y arquitectura de agentes de Aegis antes de compilar en Rust. Incluye la implementación completa del Asistente Personal de Chat Infinito (memoria a corto y largo plazo, inferencia semántica de gastos/recordatorios, workers en segundo plano no bloqueantes y consolidación `Chronos`).

## Acceptance Criteria
- [x] Implementar `PersonalMemoryStore` con persistencia en `aegis_memory.json` (hechos de largo plazo, libro contable con normalización semántica como `super` -> `comida`, recordatorios y ventana deslizante de historial).
- [x] Implementar herramientas personales instantáneas (`log_expense_or_entry`, `query_ledger`, `remember_fact`, `manage_reminder`) y ejecución de `spawn_agent` en hilos de segundo plano (`threading.Thread`) con notificación en vivo al usuario.
- [x] Implementar rotación automática de API keys y fallback bidireccional entre OpenRouter y Gemini ante errores HTTP 429/503.
- [x] Crear suite de pruebas automatizada en `tools/agent_sandbox/scenarios.json` verificando inferencia de gastos, memoria a largo plazo, recuperación contextual y creación de scripts en segundo plano (5/5 PASS — 100%).
