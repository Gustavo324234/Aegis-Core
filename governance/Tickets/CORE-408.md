# CORE-408: Dynamic Tool Gating, Prompt Cache-First, Deduplicación de `chat_agent.md` y Memoria Selectiva

- **Tipo:** perf
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 62
- **Asignado a:** Kernel Engineer + AI Architect

---

## Description

Reducir entre un 60% y 75% los tokens enviados por turno al LLM y eliminar la confusión por exceso de esquemas:
- Implementar **Dynamic Tool Gating** (`select_chat_tool_names`) en `chal/context_builder.rs` y filtrado por nombre en `ToolRegistry` para enviar solo las herramientas del dominio relevante en cada turno (0 esquemas en charla conceptual pura).
- Compactar y deduplicar `kernel/config/agents/chat_agent.md` y descripciones redundantes en `ToolRegistry`, reforzando la profundidad analítica adaptativa sin repetir documentación de parámetros.
- Mantener el primer mensaje `System` 100% estático e invariante entre minutos (moviendo fecha/hora y memoria dinámica a un bloque operativo previo al último turno `User`) para habilitar Prompt Caching en Anthropic, OpenAI, DeepSeek y Gemini.
- Incorporar `TenantDB::build_contextual_personal_memory_block(query)` para inyectar solo facts/finanzas/recordatorios pertinentes al turno y compactar resultados antiguos de tools en bucles ReAct (`prune_stale_tool_outputs`).

## Acceptance Criteria

- [x] Turnos conversacionales puros omiten esquemas pesados de tools y texto redundante de módulos/MCP.
- [x] `messages[0]` (`System`) permanece byte-a-byte idéntico entre minutos/consultas con la misma persona.
- [x] Memoria personal y outputs ReAct antiguos se filtran/compactan sin romper herramientas existentes.
