# CORE-151 — Feat: Integración de Contexto de Proyecto (Git/VCM)

**Epic:** 42 — Vision Realignment & Autonomy
**Repo:** Aegis-Core
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer

---

## Problema

Cuando el usuario dice "Sigamos trabajando en Aegis", el asistente no sabe automáticamente en qué punto del desarrollo se quedaron, qué tickets están abiertos o cuál es el estado actual de las ramas de Git.

## Solución Propuesta

Extender el **Virtual Context Manager (VCM)** para que pueda inyectar metadatos del proyecto de forma proactiva.

### Requisitos Técnicos:
1.  **Git Driver:** Implementar un driver que lea `git status`, `git branch` y los últimos mensajes de commit.
2.  **Governance Crawler:** Una herramienta que escanee `governance/Tickets/` para identificar tickets "In Progress".
3.  **Inyección en Contexto:** Al detectar la intención "desarrollo/proyecto", el VCM debe inyectar un resumen: "Estás en la rama X, trabajando en el ticket Y. El último cambio fue Z."
4.  **Coordinación:** Aegis debe poder actualizar el estado de los tickets en `TICKETS_MASTER.md` automáticamente.

---

## Criterios de Aceptación

- [x] Al preguntar "¿En qué quedamos?", Aegis responde con el estado real del repositorio y los tickets pendientes.
- [x] Integración asíncrona para no penalizar el tiempo de respuesta.

---

*Arquitecto IA — 2026-04-23*
