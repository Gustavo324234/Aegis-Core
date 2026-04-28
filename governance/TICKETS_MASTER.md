# Aegis Core — TICKETS MASTER

Este archivo es la fuente única de verdad para el estado de todos los tickets del proyecto.

## 🚀 Epics

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | En Curso | 80% |
| EPIC 42 | Vision Realignment & Autonomy | En Curso | 40% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | ✅ Completa | 100% |
| EPIC 44 | Developer Workspace | ✅ Completa | 100% |
| EPIC 45 | Cognitive Agent Architecture (CAA) | En Curso | 85% |

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

---

### EPIC 42 — Vision Realignment & Autonomy

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-150 | feat | Sandbox de Scripts (Maker Capability) | 📥 Todo | Crítica |
| CORE-151 | feat | Integración de Contexto de Proyecto (Git/VCM) | 🚧 In Progress | Alta |
| CORE-152 | feat | Plugins de Dominios (Ledger & Chronos) | ✅ Done | Media |
| CORE-153 | feat | Dashboard Dinámico & Kanban UI | ✅ Done | Alta |
| CORE-154 | feat | Orquestación de Sub-Agentes especializados | ✅ Done | Baja |

---

### EPIC 43 — Hierarchical Multi-Agent Orchestration ✅

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

> **Nota:** CORE-164 descartado — era para AdminDashboard (incorrecto). Reemplazado por CORE-166.

---

### EPIC 44 — Developer Workspace ✅

**Documento de diseño:** `governance/EPIC_44_DEVELOPER_WORKSPACE.md`

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-167 | feat | workspace_config — Tabla SQLCipher y Endpoint de Configuración | ✅ Done | Crítica | Kernel Engineer |
| CORE-168 | feat | TerminalExecutor — Ejecución de Comandos con Streaming | ✅ Done | Crítica | Kernel Engineer |
| CORE-169 | feat | SYS_EXEC — Syscall de Terminal para Agentes | ✅ Done | Alta | Kernel Engineer |
| CORE-170 | feat | FileSystemBridge — Endpoints /api/fs/tree y /api/fs/file | ✅ Done | Alta | Kernel Engineer |
| CORE-171 | feat | GitHubBridge — Identidad del Bot, Branch, Commit, Push y PR | ✅ Done | Crítica | Kernel Engineer |
| CORE-172 | feat | SYS_GIT_* — Syscalls Git para Agentes | ✅ Done | Alta | Kernel Engineer |
| CORE-173 | feat | PR Manager — Ciclo de Vida de PRs con Polling de CI | ✅ Done | Crítica | Kernel Engineer |
| CORE-174 | feat | Auto-fix CI — Proceso Cognitivo Disparado por Fallo de CI | ✅ Done | Alta | Kernel Engineer |
| CORE-175 | feat | Eventos WebSocket — terminal_output, pr_update, pr_merged, git_push, ci_fix_attempt | ✅ Done | Crítica | Kernel Engineer |
| CORE-176 | feat | TerminalPanel — UI de Terminal en Dashboard del Tenant | ✅ Done | Alta | Shell Engineer |
| CORE-177 | feat | CodeViewer — Árbol de Archivos y Contenido en Dashboard | ✅ Done | Alta | Shell Engineer |
| CORE-178 | feat | GitTimeline — Branches, Commits y PRs en Dashboard | ✅ Done | Alta | Shell Engineer |
| CORE-179 | feat | PRManagerPanel — Lista de PRs con Controles Auto/Manual | ✅ Done | Alta | Shell Engineer |
| CORE-180 | feat | WorkspaceSettings — Configuración de Token, Repo y Opciones | ✅ Done | Alta | Shell Engineer |

---

### EPIC 45 — Cognitive Agent Architecture (CAA)

**Documento de diseño:** `governance/EPIC_45_COGNITIVE_AGENT_ARCHITECTURE.md`

#### Fase 1 — Tipos base (Kernel Engineer) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-190 | feat | AgentRole, AgentNode, AgentState, AgentId — tipos base con task_type y model_preference | ✅ Done | Crítica | Kernel Engineer |
| CORE-191 | feat | AgentTree — estructura n-ary con serialize/restore | ✅ Done | Crítica | Kernel Engineer |
| CORE-192 | feat | AgentMessage — Dispatch, Report, Query, QueryReply | ✅ Done | Crítica | Kernel Engineer |
| CORE-195 | feat | PCB Extension — campo agent_id + task_type override | ✅ Done | Alta | Kernel Engineer |
| CORE-196 | feat | ContextBudget — presupuesto de tokens por AgentNode | ✅ Done | Alta | Kernel Engineer |

#### Fase 2 — Orchestration + Persistencia (Kernel Engineer) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-193 | feat | AgentOrchestrator — ciclo de vida, routing, persist on close, restore on activation | ✅ Done | Crítica | Kernel Engineer |
| CORE-194 | feat | ProjectRegistry — CRUD de proyectos + tabla SQLite + serialize/restore árbol | ✅ Done | Crítica | Kernel Engineer |
| CORE-197 | feat | InstructionLoader — carga de agent/*.md en runtime + state summary template | ✅ Done | Alta | Kernel Engineer |
| CORE-198 | feat | SYS_AGENT_SPAWN — syscall con task_type opcional | ✅ Done | Alta | Kernel Engineer |
| CORE-199 | feat | SYS_AGENT_QUERY — syscall de query descendente | ✅ Done | Alta | Kernel Engineer |
| CORE-206 | feat | AgentPersistence — lectura/escritura agent_tree.json + agent_contexts/*.md | ✅ Done | Alta | Kernel Engineer |
| CORE-207 | feat | State Summary Generator — trigger al cerrar sesión, genera .md por supervisor | ✅ Done | Alta | Kernel Engineer |

#### Fase 3 — CMR Integration (Kernel Engineer) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-208 | feat | CMR per-agent — AgentOrchestrator pasa task_type al CMR al crear PCB por agente | ✅ Done | Alta | Kernel Engineer |

#### Fase 4 — Visibilidad (Kernel Engineer + Shell Engineer) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-200 | feat | AgentEvent stream — WebSocket ws/agents/{tenant_id} con evento Restored | ✅ Done | Alta | Kernel Engineer |
| CORE-202 | feat | AgentActivityPanel — indicador colapsable en ChatTerminal con modelo visible | ✅ Done | Alta | Shell Engineer |
| CORE-203 | feat | AgentTreeView + ProjectList — panel en Dashboard con árbol en tiempo real | ✅ Done | Media | Shell Engineer |
| CORE-204 | feat | useAegisStore — agentTree + activeProjects + connectAgentStream | ✅ Done | Alta | Shell Engineer |

#### Fase 5 — Chat Agent Integration (Kernel Engineer) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-201 | feat | Chat Agent context limiter — ventana deslizante + resumen automático | ✅ Done | Alta | Kernel Engineer |

#### Fase 6 — Instrucciones (Arquitecto IA) ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-205 | feat | Archivos agent/*.md — chat_agent, supervisor, specialist + state summary template | ✅ Done | Alta | Arquitecto IA |

#### Bugs detectados post-integración (2026-04-28)

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-209 | fix | Montar /ws/agents en build_router y agregar GET /api/agents/projects | 📥 Todo | Crítica | Kernel Engineer |
| CORE-210 | fix | Chat Agent: declarar ausencia de contexto cuando no hay proyecto activo | 📥 Todo | Alta | Kernel Engineer |
| CORE-211 | fix | Shell: manejo graceful de errores en fetchActiveProjects y connectAgentStream | 📥 Todo | Alta | Shell Engineer |

---

*Leyenda:*
- 📥 **Todo:** Pendiente de inicio.
- 🚧 **In Progress:** En desarrollo activo.
- ✅ **Done:** Terminado y verificado.
- ❌ **Blocked:** Detenido por dependencias.
