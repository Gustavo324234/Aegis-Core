# Aegis Core — TICKETS MASTER

Este archivo es la fuente única de verdad para el estado de todos los tickets del proyecto.

## 🚀 Epics

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | En Curso | 80% |
| EPIC 42 | Vision Realignment & Autonomy | En Curso | 45% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | ✅ Completa | 100% |
| EPIC 44 | Developer Workspace | ✅ Completa | 100% |
| EPIC 45 | Cognitive Agent Architecture (CAA) | ✅ Completa | 100% |
| EPIC 46 | Public Launch | ✅ Completa | 100% |
| EPIC 47 | Agent Protocol v2: Tool Use | 📥 Planned | 0% |

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
| CORE-212 | fix | Shell: provider gemini en KeyManager + visibilidad de modelos en CatalogViewer | ✅ Done | Crítica |

---

### EPIC 43 — Hierarchical Multi-Agent Orchestration ✅

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

---

### EPIC 44 — Developer Workspace ✅

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

### EPIC 45 — Cognitive Agent Architecture ✅

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-190 | feat | AgentRole, AgentNode, AgentState, AgentId — tipos base | ✅ Done | Crítica | Kernel Engineer |
| CORE-191 | feat | AgentTree — estructura n-ary con serialize/restore | ✅ Done | Crítica | Kernel Engineer |
| CORE-192 | feat | AgentMessage — Dispatch, Report, Query, QueryReply | ✅ Done | Crítica | Kernel Engineer |
| CORE-193 | feat | AgentOrchestrator — ciclo de vida, routing, persist, restore | ✅ Done | Crítica | Kernel Engineer |
| CORE-194 | feat | ProjectRegistry — CRUD + SQLite + serialize/restore árbol | ✅ Done | Crítica | Kernel Engineer |
| CORE-195 | feat | PCB Extension — agent_id + task_type override | ✅ Done | Alta | Kernel Engineer |
| CORE-196 | feat | ContextBudget — presupuesto de tokens por AgentNode | ✅ Done | Alta | Kernel Engineer |
| CORE-197 | feat | InstructionLoader — agent/*.md en runtime + state summary | ✅ Done | Alta | Kernel Engineer |
| CORE-198 | feat | SYS_AGENT_SPAWN — syscall con task_type opcional | ✅ Done | Alta | Kernel Engineer |
| CORE-199 | feat | SYS_AGENT_QUERY — syscall de query descendente | ✅ Done | Alta | Kernel Engineer |
| CORE-200 | feat | AgentEvent stream — WebSocket ws/agents/{tenant_id} | ✅ Done | Alta | Kernel Engineer |
| CORE-201 | feat | Chat Agent context limiter — ventana deslizante + resumen | ✅ Done | Alta | Kernel Engineer |
| CORE-202 | feat | AgentActivityPanel — indicador colapsable en ChatTerminal | ✅ Done | Alta | Shell Engineer |
| CORE-203 | feat | AgentTreeView + ProjectList — panel en Dashboard | ✅ Done | Media | Shell Engineer |
| CORE-204 | feat | useAegisStore — agentTree + activeProjects + connectAgentStream | ✅ Done | Alta | Shell Engineer |
| CORE-205 | feat | Archivos agent/*.md — instrucciones por rol | ✅ Done | Alta | Arquitecto IA |
| CORE-206 | feat | AgentPersistence — agent_tree.json + agent_contexts/*.md | ✅ Done | Alta | Kernel Engineer |
| CORE-207 | feat | State Summary Generator — trigger al cerrar sesión | ✅ Done | Alta | Kernel Engineer |
| CORE-208 | feat | CMR per-agent — task_type al CMR por AgentNode | ✅ Done | Alta | Kernel Engineer |
| CORE-209 | fix | Montar /ws/agents en build_router y agregar GET /api/agents/projects | ✅ Done | Crítica | Kernel Engineer |
| CORE-210 | fix | Chat Agent: fallback cuando no hay proyecto activo | ✅ Done | Alta | Kernel Engineer |
| CORE-211 | fix | Shell: graceful errors en fetchActiveProjects y connectAgentStream | ✅ Done | Alta | Shell Engineer |

---

### EPIC 46 — Public Launch ✅

| ID | Tipo | Título | Estado | Prioridad | Responsable |
|---|---|---|---|---|---|
| CORE-214 | docs | CODE_OF_CONDUCT.md | ✅ Done | Alta | Arquitecto IA |
| CORE-215 | docs | SECURITY.md — política de reporte de vulnerabilidades | ✅ Done | Alta | Arquitecto IA |
| CORE-216 | docs | CHANGELOG.md — historial de versiones público | ✅ Done | Media | Arquitecto IA |
| CORE-217 | docs | Issue template: Bug Report | ✅ Done | Alta | Arquitecto IA |
| CORE-218 | docs | Issue template: Feature Request | ✅ Done | Media | Arquitecto IA |
| CORE-219 | ops | GitHub Sponsors + FUNDING.yml + sponsor page | ✅ Done | Alta | Tavo |
| CORE-220 | ops | Release — gestionado por release-please | ✅ Done | Crítica | Automático |
| CORE-221 | ops | Topics del repo | ✅ Done | Media | Tavo |
| CORE-222 | ops | Social preview image | ✅ Done | Media | Tavo |
| CORE-223 | docs | .github/CODEOWNERS | ✅ Done | Media | Arquitecto IA |
| CORE-224 | chore | Limpiar directorios temporales | 📥 Todo | Baja | Tavo |
| CORE-225 | chore | License field en Cargo.toml → MIT | 📥 Todo | Alta | Kernel Engineer |

---

### EPIC 47 — Agent Protocol v2: Tool Use

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-236 | feat | ToolRegistry — definición de herramientas + schema por proveedor | 📥 Todo | Crítica | Kernel Engineer |
| CORE-234 | feat | AgentOrchestrator — migrar de token parsing a tool use dispatch | 📥 Todo | Crítica | Kernel Engineer |
| CORE-235 | feat | SyscallExecutor — mapear tool call results a AgentMessage internos | 📥 Todo | Crítica | Kernel Engineer |
| CORE-237 | feat | Ollama fallback — detección de tool use support + modo degradado | 📥 Todo | Alta | Kernel Engineer |
| CORE-238 | docs | Agent files + PROTOCOL.md — reescritura post tool use | 📥 Todo | Alta | Arquitecto IA |

---

### Bugs pre-lanzamiento — Multi-Agent Pipeline (Kernel)

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-226 | fix | Kernel: Chat Agent usa SYSTEM_PROMPT_MASTER genérico en lugar de chat_agent.md | 📥 Todo | Crítica | Kernel Engineer |
| CORE-227 | fix | Kernel: SPAWN_INSTRUCTIONS usa sintaxis obsoleta — divergencia con parser | 📥 Todo | Crítica | Kernel Engineer |
| CORE-228 | fix | Kernel: SyscallExecutor se crea sin AgentOrchestrator — SYS_AGENT_SPAWN siempre falla | 📥 Todo | Crítica | Kernel Engineer |
| CORE-229 | fix | Installer: agents config no se despliega en producción | 📥 Todo | Alta | DevOps Engineer |

---

### Bugs pre-lanzamiento — Shell / UX

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-230 | fix | Shell: Dashboard crashea al montar y dispara AegisErrorBoundary ("Session Error") | 📥 Todo | Crítica | Shell Engineer |
| CORE-231 | fix | Shell: Micrófono falla silenciosamente en HTTP — falta feedback al usuario | 📥 Todo | Crítica | Shell Engineer |
| CORE-232 | fix | Shell: IP vs Cloudflare producen chats separados — falta aviso y pre-fill de login | 📥 Todo | Alta | Shell Engineer |
| CORE-233 | feat | Shell: Settings del tenant — simplificar y unificar configuración en 4 tabs | 📥 Todo | Alta | Shell Engineer |

---

### Bugs de infraestructura

| ID | Tipo | Título | Estado | Prioridad | Responsable |
|---|---|---|---|---|---|
| OPS-001 | ops | Re-registrar API keys Gemini/OpenRouter en DB via UI tras reinicio | 📥 Todo | Crítica | Tavo (manual) |

---

### Deuda técnica pendiente

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-213 | fix | Kernel: loguear error en key_pool.load() al arranque | 📥 Todo | Media |

---

*Leyenda:*
- 📥 **Todo:** Pendiente de inicio.
- 🚧 **In Progress:** En desarrollo activo.
- ✅ **Done:** Terminado y verificado.
- ❌ **Blocked:** Detenido por dependencias.
