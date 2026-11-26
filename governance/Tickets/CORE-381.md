# CORE-381: Guía de Colaboración Rápida con Agent Sandbox y `Good First Issues` en `CONTRIBUTING.md`

- **Tipo:** docs
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 58
- **Asignado a:** Arquitecto IA

---

## Description
Compilar todo el stack nativo (Rust 1.80+ + Node 20+ + `protoc`) representa una barrera alta para colaboradores que quieren probar mejoras de prompts, nuevas herramientas personales o flujos de agentes. Documentar el simulador `tools/agent_sandbox/` y listar `Good First Issues` claras en `CONTRIBUTING.md` reduce el tiempo de incorporación de nuevos contribuidores de horas a 30 segundos.

## Acceptance Criteria
- [x] Actualizar `CONTRIBUTING.md` con una sección **"⚡ Fast Track: Contribute in 60 Seconds (Agent Sandbox)"** explicando cómo usar `tools/agent_sandbox/sandbox.py` y su benchmark `--test`.
- [x] Incorporar en `CONTRIBUTING.md` una tabla curada de **`Good First Issues` & Áreas Abiertas de Colaboración** mapeadas a los tickets de `EPIC 58` (`CORE-375` a `CORE-379` y mejoras de herramientas).
- [x] Crear `tools/agent_sandbox/README.md` explicando los comandos interactivos (`/memoria`, `/gastos`, `/chronos`, `/jobs`), arquitectura del simulador y cómo añadir nuevos escenarios en `scenarios.json`.
