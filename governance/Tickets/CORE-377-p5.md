# CORE-377-p5: Integración del Protocolo MCP (Model Context Protocol) (Fase 5)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** Fase 5 — Cutting-Edge (Otras Características Consolidadas)
- **Asignado a:** Kernel Engineer

---

> **Nota de reconciliación (`CORE-396`):** Definido originalmente como `CORE-377` en [`docs/TICKETS_CUTTING_EDGE_ROADMAP.md`](file:///e:/Aegis/Aegis-Core/docs/TICKETS_CUTTING_EDGE_ROADMAP.md) e implementado en el commit `77e6554` (*"feat(mcp): CORE-377 model context protocol client integration and tenant isolation"*). Se canoniza como `CORE-377-p5` para evitar colisión con [`CORE-377.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-377.md) (EPIC 58 — Quick Personal Setup).

## Description

Cliente nativo MCP (`ank-mcp`) con transporte `stdio` y `sse` sobre JSON-RPC 2.0, endpoints `/api/mcp/*` en `ank-http`, descubrimiento dinámico de herramientas externas y aislamiento de permisos por tenant.

## Acceptance Criteria

- [x] Crate `ank-mcp` integrado al workspace y conectado a `CognitiveHAL` (`77e6554`).
- [x] Aislamiento por `tenant_id` y verificación de políticas antes de invocar herramientas MCP.
- [x] Tests unitarios en `ank-mcp` pasando.
