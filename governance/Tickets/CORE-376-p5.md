# CORE-376-p5: Soporte Nativo de Tool Calling / Function Calling en Router Cognitivo (Fase 5)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** Fase 5 — Cutting-Edge (Otras Características Consolidadas)
- **Asignado a:** Kernel Engineer

---

> **Nota de reconciliación (`CORE-396`):** Definido originalmente como `CORE-376` en [`docs/TICKETS_CUTTING_EDGE_ROADMAP.md`](file:///e:/Aegis/Aegis-Core/docs/TICKETS_CUTTING_EDGE_ROADMAP.md) e implementado en el commit `ffb4e62` (*"feat(router): CORE-376 native tool calling schemas and PCB execution suspension"*). Se canoniza como `CORE-376-p5` para evitar colisión con [`CORE-376.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-376.md) (EPIC 58 — Chronos).

## Description

Soporte unificado de `ToolDefinition` / `ToolCall` en `ank-core::pcb`, serializadores/deserializadores para OpenAI-compatible, Anthropic y Gemini Function Calling, y suspensión/reanudación del PCB para ejecución de herramientas vía Citadel.

## Acceptance Criteria

- [x] Estructuras `ToolDefinition` y `ToolCall` implementadas en `ank-core::pcb`.
- [x] Adaptadores multi-proveedor y bucle ReAct con suspensión de ejecución (`ffb4e62`).
- [x] Tests unitarios en `pcb.rs` y `ank-core` en verde.
