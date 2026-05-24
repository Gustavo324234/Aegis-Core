# CORE-150 — Feat: Sandbox de Scripts (Maker Capability)

**Epic:** 42 — Vision Realignment & Autonomy
**Repo:** Aegis-Core
**Tipo:** feat
**Prioridad:** Crítica
**Asignado a:** Kernel Engineer
**Estado:** ✅ Completo

---

## Problema

Actualmente, el sistema de plugins de Aegis es estricto y requiere compilación Wasm y firmas criptográficas. Esto impide que el asistente cree sus propias herramientas de forma autónoma durante una conversación ("Maker Capability").

## Solución Propuesta

Implementar un **ScriptRunner Sandbox** que permita ejecutar código interpretado (ej. JavaScript o Python mínimo) en un entorno seguro y aislado.

### Requisitos Técnicos:
1.  **Nuevo Syscall:** `SYS_CALL_MAKER(script_type, code, params)`.
2.  **Aislamiento:** El script debe correr sin acceso a la red ni al sistema de archivos del host, excepto su propio `/workspace` del tenant.
3.  **Persistencia:** Aegis debe poder guardar estos scripts en su workspace y reutilizarlos como herramientas registradas dinámicamente.
4.  **Integración con LLM:** El modelo debe recibir instrucciones sobre cómo escribir estos scripts para resolver problemas (ej. "escribí un script que calcule la desviación estándar de estos datos").

---

## Criterios de Aceptación

- [x] El kernel puede ejecutar un script de prueba y devolver el resultado.
- [x] Los scripts creados por Aegis persisten entre sesiones en el workspace del tenant.
- [x] El System Prompt incluye instrucciones para el uso del `MAKER` syscall.

---

*Arquitecto IA — 2026-05-24*
