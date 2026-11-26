# EPIC 58 — Personal Assistant Experience, Zero-Friction UX & Community Onboarding

**Estado:** ✅ Done

## 🎯 Objetivo

Consolidar la visión central de Aegis como **un asistente personal autónomo con un único chat infinito**, memoria de corto y largo plazo, inferencia semántica de finanzas/recordatorios, aprendizaje implícito en segundo plano (`Chronos`) y delegación asíncrona no bloqueante, eliminando todas las barreras de instalación/UX y abriendo el proyecto a colaboradores externos mediante documentación clara y un simulador ligero (`tools/agent_sandbox/`).

---

## 🏗️ Pilares y Criterios de Aceptación

### Pilar 1: El Cerebro del Asistente Personal (Del Sandbox al Kernel Rust)
1. **Simulador y Benchmark de Asistente Personal (`CORE-372`):** Entorno aislado en `tools/agent_sandbox/` con memoria persistente (`aegis_memory.json`), herramientas personales instantáneas, workers en hilos de segundo plano, consolidación `Chronos` y suite de pruebas automatizada (`scenarios.json` 5/5 PASS).
2. **Despacho Multi-Agente Real en `ank-core` (`CORE-373`):** Auto-despacho inmediato al crear subagentes en `orchestrator.rs`, herramientas de ejecución directa para supervisores en `tool_registry.rs` y eliminación de instrucciones contradictorias en prompts.
3. **Herramientas Personales Nativas en Rust (`CORE-375`):** Portar `log_expense_or_entry`, `query_ledger`, `remember_fact` y `manage_reminder` al `ToolRegistry` y `AgentOrchestrator` de `ank-core`, permitiendo que frases como *"Agregá $10.000 de súper"* se clasifiquen automáticamente en `comida` dentro del enclave cifrado del usuario.
4. **Consolidación Nocturna Real en `Chronos` (`CORE-376`):** Conectar `ChronosDaemon` (`ank-core/src/chronos/mod.rs`) al historial real de conversación del usuario en lugar del texto de prueba estático, extrayendo hechos implícitos a la memoria de largo plazo (L3 Swap / Enclave).

### Pilar 2: UX "Zero-Friction" para Humanos (`shell/ui` + `ank-http`)
5. **Onboarding Personal en 1 Solo Paso (`CORE-377`):** Reemplazar el flujo empresarial de 6 pasos (`setup_token` en consola ➔ crear Admin ➔ crear Tenant ➔ relogin ➔ cambio forzado de clave ➔ configurar motor) por un único asistente de bienvenida local (*Nombre + Contraseña + API Key / Ollama*) que deje al usuario hablando con Aegis en menos de 60 segundos.
6. **Persistencia de Sesión al Refrescar (`F5`) (`CORE-378`):** Evitar que recargar el navegador en `App.tsx` borre la `sessionKey` y expulse al usuario a la pantalla de login.
7. **Interfaz Humanizada y Panel de Vida Personal (`CORE-379`):** Reemplazar la jerga críptica de kernel (*"Operator"*, *"ANK Kernel"*, *"Executing Syscall"*, *"Ring 0"*) por lenguaje natural de asistente personal y añadir un panel lateral/vista de **Memoria Recordada**, **Gastos/Finanzas** y **Tareas en Segundo Plano**.

### Pilar 3: Higiene del Repositorio y Comunidad Open Source
8. **Limpieza de Raíz y `.gitignore` (`CORE-374`):** Eliminar logs residuales de CI (`ci_*.log`, `pr_*.txt`, `token_output.log`) de la raíz del repositorio y proteger directorios de trabajo locales.
9. **Documentación orientada a Producto (`CORE-380`):** Rediseñar `README.md` y `README.es.md` presentando primero la experiencia de Asistente Personal de Chat Infinito con ejemplos cotidianos reales antes de la arquitectura interna del kernel.
10. **On-Ramp de 30 Segundos para Colaboradores (`CORE-381`):** Documentar `tools/agent_sandbox/` en `CONTRIBUTING.md` para que cualquier desarrollador pueda probar y mejorar prompts, herramientas y agentes sin necesidad de compilar todo el stack Rust + Node, junto con un listado de `Good First Issues` claras.

---

## 🛠️ Listado de Tickets Mapeados

* **CORE-372:** Agent Sandbox & Personal Assistant Simulator (`tools/agent_sandbox/`) — ✅ Done
* **CORE-373:** Kernel Multi-Agent Auto-Dispatch & Direct Supervisor Execution (`ank-core`) — ✅ Done
* **CORE-374:** Limpieza de Raíz del Repositorio y Actualización de `.gitignore` — ✅ Done
* **CORE-375:** Kernel Personal Assistant Tools (`log_expense_or_entry`, `query_ledger`, `remember_fact`, `manage_reminder`) en `ank-core` — ✅ Done
* **CORE-376:** Chronos Real Conversation Consolidation en `ank-core/src/chronos` — ✅ Done
* **CORE-377:** Zero-Friction Personal Onboarding Wizard (1-Step Local Setup) en `shell/ui` y `ank-http` — ✅ Done
* **CORE-378:** Persistencia de Sesión tras Recarga (`F5`) en `shell/ui/src/App.tsx` — ✅ Done
* **CORE-379:** Humanización de Interfaz Web y Panel de Vida Personal (Memoria, Gastos y Tareas en Segundo Plano) en `shell/ui` — ✅ Done
* **CORE-380:** Rediseño de `README.md` y `README.es.md` con Enfoque de Asistente Personal y Casos de Uso — ✅ Done
* **CORE-381:** Guía de Colaboración Rápida con Agent Sandbox y `Good First Issues` en `CONTRIBUTING.md` — ✅ Done
