# EPIC 42 — Aegis Vision Realignment & Autonomy

**Estado:** Planificado
**Prioridad:** Crítica
**Dueño:** Kernel & UI Engineers

---

## 1. Visión

Esta Epic tiene como objetivo re-alinear el proyecto Aegis-Core con su visión original: un asistente personal autónomo capaz de gestionar proyectos, llevar registro de la vida del usuario (gastos/reuniones) y, lo más importante, desarrollar sus propias herramientas.

## 2. Objetivos

- **Autonomía Técnica:** Permitir que Aegis escriba y ejecute código en un sandbox seguro para resolver tareas sin intervención manual.
- **Contexto de Proyecto:** Integrar el kernel con el sistema de archivos y Git para que Aegis sepa exactamente en qué punto del desarrollo está.
- **Dominios de Vida:** Implementar gestión de gastos y recordatorios.
- **Visualización Proactiva:** Añadir un Dashboard dinámico con Kanban y paneles de datos.

## 3. Tickets (Fases)

| ID | Título | Estado | Prioridad |
|---|---|---|---|
| [CORE-150](./Tickets/CORE-150.md) | Sandbox de Scripts (Maker Capability) | Pendiente | Bloqueante |
| [CORE-151](./Tickets/CORE-151.md) | Integración de Contexto de Proyecto (Git/VCM) | Pendiente | Alta |
| [CORE-152](./Tickets/CORE-152.md) | Plugins de Dominios (Ledger & Chronos) | Pendiente | Media |
| [CORE-153](./Tickets/CORE-153.md) | Dashboard Dinámico & Kanban UI | Pendiente | Alta |
| [CORE-154](./Tickets/CORE-154.md) | Orquestación de Sub-Agentes especializados | Pendiente | Baja |

---

## 4. Criterios de Aceptación Globales

- [ ] Aegis puede crear un script "hello world" y ejecutarlo como una herramienta.
- [ ] Aegis puede responder "estábamos trabajando en el ticket X" al iniciar un chat.
- [ ] El usuario puede ver un resumen de sus gastos en un panel visual.
- [ ] El usuario puede ver sus tareas pendientes en un tablero Kanban.

---

*Arquitecto IA — 2026-04-23*
