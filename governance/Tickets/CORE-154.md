# CORE-154 — Feat: Orquestación de Sub-Agentes Especializados

**Epic:** 42 — Vision Realignment & Autonomy
**Repo:** Aegis-Core
**Tipo:** feat
**Prioridad:** Baja
**Asignado a:** Kernel Engineer  
**Estado:** ✅ Completo

---

## Problema

Actualmente hay un solo hilo de ejecución cognitiva. Para tareas complejas (ej. "programar una característica completa"), un solo agente puede perderse. Se requiere la capacidad de despachar sub-agentes con contextos limitados y tareas específicas.

## Solución Propuesta

Implementar el patrón **Supervisor/Worker** en el Kernel.

### Requisitos Técnicos:
1.  **Spawn Syscall:** `SYS_CALL_SPAWN(task_description, role)`.
2.  **Aislamiento de Contexto:** Cada sub-agente recibe solo los archivos y la memoria necesarios para su tarea.
3.  **Protocolo de Reporte:** Los sub-agentes deben devolver un reporte estructurado al Agente Principal (Supervisor).
4.  **Paralelismo:** Capacidad de correr múltiples inferencias si el hardware lo permite (o serializadas con prioridad).

---

## Criterios de Aceptación

- [x] El Agente Principal puede asignar una sub-tarea a un "Agente Programador" y recibir el código resultante.
- [x] Los logs del kernel distinguen entre el Supervisor y los Workers.

---

*Arquitecto IA — 2026-04-23*
