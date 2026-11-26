# CORE-375-p5: Catálogo 2026 Embebido en `models.yaml` (Fase 5 — Alias Histórico de Commit `CORE-375`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** Fase 5 — Cutting-Edge (Otras Características Consolidadas)
- **Asignado a:** Kernel Engineer

---

> **Nota de reconciliación (`CORE-396`):** En el historial de Git, los commits `f7bf48b` y `eb81280` (*"feat(router): add latest 2026 models to bundled catalog (CORE-375) (#375)"*) usaron el ID `CORE-375`, el cual posteriormente fue asignado formalmente en `EPIC 58` a [`CORE-375.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-375.md). Este archivo registra el entregable histórico de Fase 5 bajo el identificador canónico `CORE-375-p5`.

## Description

Actualización del catálogo embebido `kernel/crates/ank-core/src/router/models.yaml` con la generación de modelos 2026 (GPT-4.5/o1/o3-mini, Claude 3.5/3.7/5.5, Gemini 2.0/1.5, DeepSeek V3/R1, Groq y OpenRouter) y actualización de `libz-sys` en `deny.toml`.

## Acceptance Criteria

- [x] Modelos 2026 integrados en `kernel/crates/ank-core/src/router/models.yaml` (`f7bf48b`, `eb81280`).
- [x] Colisión de ID reconciliada sin alterar la trazabilidad con `CORE-375` (EPIC 58).
