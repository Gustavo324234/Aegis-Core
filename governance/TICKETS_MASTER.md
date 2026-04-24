# Aegis Core — TICKETS MASTER

Este archivo es la fuente única de verdad para el estado de todos los tickets del proyecto.

## 🚀 Epics Activas

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | En Curso | 80% |
| EPIC 42 | Vision Realignment & Autonomy | En Curso | 40% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | Planificada | 0% |

---

## 🎫 Tickets

### EPIC 41 — UX & Onboarding

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-145 | feat | Conversational Onboarding (Name/Persona) | ✅ Done | Crítica |
| CORE-146 | feat | Remote Access via Cloudflare Tunnel + QR | ✅ Done | Alta |
| CORE-147 | fix | Hardened Tunnel & TLS Removal | ✅ Done | Media |
| CORE-148 | fix | Natural Conversational Tone (Prompt) | 🚧 In Progress | Alta |
| CORE-149 | feat | Neuronal Memory (L3) & Semantic Retrieval | ✅ Done | Crítica |

### EPIC 42 — Vision Realignment & Autonomy

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-150 | feat | Sandbox de Scripts (Maker Capability) | 📥 Todo | Crítica |
| CORE-151 | feat | Integración de Contexto de Proyecto (Git/VCM) | 🚧 In Progress | Alta |
| CORE-152 | feat | Plugins de Dominios (Ledger & Chronos) | ✅ Done | Media |
| CORE-153 | feat | Dashboard Dinámico & Kanban UI | 📥 Todo | Alta |
| CORE-154 | feat | Orquestación de Sub-Agentes especializados | 📥 Todo | Baja |

### EPIC 43 — Hierarchical Multi-Agent Orchestration

**Descripción:** Sistema multiagente jerárquico n-ary. El usuario opera como CEO; el kernel activa ProjectSupervisors por proyecto, que spawnan DomainSupervisors y SpecialistAgents dinámicamente. El CMR asigna el mejor modelo disponible a cada agente según su TaskType.

**Documento de diseño:** `governance/EPIC_43_HIERARCHICAL_AGENTS.md`

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-155 | feat | AgentNode, AgentRole, AgentState — Tipos Base | ✅ Done | Crítica | Kernel Engineer |
| CORE-156 | feat | AgentTree — Estructura en Memoria y Operaciones | ✅ Done | Crítica | Kernel Engineer |
| CORE-157 | feat | AgentMessage — Protocolo de Comunicación Inter-Agente | ✅ Done | Crítica | Kernel Engineer |
| CORE-158 | feat | AgentOrchestrator — Ciclo de Vida y Coordinación | ✅ Done | Crítica | Kernel Engineer |
| CORE-159 | feat | ProjectRegistry — Gestión de Proyectos y Supervisores Raíz | ✅ Done | Alta | Kernel Engineer |
| CORE-160 | feat | PCB Extension — campo agent_id en ProcessControlBlock | ✅ Done | Alta | Kernel Engineer |
| CORE-161 | feat | DagNode Extension — campo agent_id en DagNode | ✅ Done | Alta | Kernel Engineer |
| CORE-162 | feat | SYS_AGENT_SPAWN — Spawn Dinámico desde Syscall | ✅ Done | Alta | Kernel Engineer |
| CORE-163 | feat | HTTP Routes /api/agents/* — Árbol y Estado | ✅ Done | Media | Kernel Engineer |
| CORE-165 | feat | Model-per-Agent — CMR integration con TaskType por AgentNode | ✅ Done | Alta | Kernel Engineer |
| CORE-166 | feat | AgentTreeWidget — Árbol de Agentes en Dashboard del Tenant | ✅ Done | Alta | Shell Engineer |

> **Nota:** CORE-164 descartado (diseñado para AdminDashboard — incorrecto). Reemplazado por CORE-166 que integra el widget en `Dashboard.tsx` del tenant.

**Orden de implementación sugerido:**
1. CORE-155 (tipos base) — fundacional
2. CORE-156, CORE-157 en paralelo (árbol + mensajes)
3. CORE-160, CORE-161 en paralelo (extensiones mínimas del PCB y DagNode)
4. CORE-158 (orquestador — depende de 155, 156, 157, 160)
5. CORE-159 (project registry — puede correr en paralelo a 158)
6. CORE-162 (spawn syscall — depende de 158)
7. CORE-165 (CMR integration — depende de 158)
8. CORE-163 (HTTP routes — depende de 158)
9. CORE-166 (Shell UI en Dashboard del tenant — depende de 163, pero puede implementarse antes con mock)

---

*Leyenda:*
- 📥 **Todo:** Pendiente de inicio.
- 🚧 **In Progress:** En desarrollo activo.
- ✅ **Done:** Terminado y verificado.
- ❌ **Blocked:** Detenido por dependencias.
