# Aegis Core — TICKETS MASTER

## 🚀 Epics

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | ✅ Completa | 100% |
| EPIC 42 | Vision Realignment & Autonomy | ✅ Completa | 100% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | ✅ Completa | 100% |
| EPIC 44 | Developer Workspace | ✅ Completa | 100% |
| EPIC 45 | Cognitive Agent Architecture (CAA) | ✅ Completa | 100% |
| EPIC 46 | Public Launch | ⚠️ Ver nota | ~70% — items abiertos (ver EPIC_46 doc) |
| EPIC 47 | Agent Protocol v2: Tool Use | ✅ Completa | 100% |
| EPIC 48 | Shell Observability | ✅ Completa | 100% |
| EPIC 49 | Cognitive Loop: Memory layers & latency | ✅ Completa | 100% |
| EPIC 50 | Agent Inbox: Direct User-Supervisor exchanges | ✅ Completa | 100% |
| EPIC 51 | Model Intelligence: PinchBench + Ollama Cloud + CMR v2 | ✅ Completa | 100% |
| EPIC 52 | Voice Quality | ✅ Completa | 100% |
| EPIC 53 | Stabilization: Agent Loop, Observability & Infrastructure | ✅ Completa | 100% |
| EPIC 54 | Aegis Connect: Persistent WebSocket Tunneling | ✅ Completa | 100% |
| EPIC 55 | Mobile App (Orion ID & Web Redirection) | ✅ Completa | 100% |

## EPIC 49 — Cognitive Loop

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-259 | feat | CloudProxyDriver: historial de mensajes Vec<ChatMessage> en lugar de String | ✅ Done | Crítica | Kernel Engineer |
| CORE-260 | feat | PCB: historial de mensajes + SessionHistoryCache | ✅ Done | Crítica | Kernel Engineer |
| CORE-261 | feat | CognitiveHAL: bucle ReAct interno — tool call → resultado → LLM | ✅ Done | Crítica | Kernel Engineer |
| CORE-262 | feat | AgentOrchestrator: inferencia LLM real en run_agent_loop | ✅ Done | Crítica | Kernel Engineer |
| CORE-264 | fix | Dispatch automático al ProjectSupervisor tras spawn_agent del Chat Agent | ✅ Done | Crítica | Kernel Engineer |
| CORE-267 | fix | mark_rate_limited al recibir 429 en CloudProxyDriver | ✅ Done | Alta | Kernel Engineer |
| CORE-268 | feat | Kernel: emitir `AgentEvent` por WebSocket al tenant | ✅ Done | Crítica | Kernel Engineer |
| CORE-269 | feat | Shell: `AgentInbox` store + `AgentBadge` en nav | ✅ Done | Crítica | Shell Engineer |
| CORE-270 | feat | Shell: ruta `/chat/agent/:agent_id` + componente `AgentThread` | ✅ Done | Alta | Shell Engineer |
| CORE-271 | feat | Kernel: endpoint `AgentDirectMessage` para respuesta directa al supervisor | ✅ Done | Alta | Kernel Engineer |
| CORE-272 | feat | Kernel: herramienta `get_project_ledger` para el chat_agent | ✅ Done | Media | Kernel Engineer |
| CORE-273 | feat | ProjectLedger: registro de avance persistente por proyecto | ✅ Done | Alta | Kernel Engineer |
| CORE-274 | feat | WebSocket: evento `agent_event` en el protocolo existente | ✅ Done | Crítica — prerrequisito de CORE-268 | Kernel Engineer + Shell Engineer |
| CORE-275 | feat | Specialist: tools de filesystem (`read_file`, `write_file`, `list_files`) | ✅ Done | Crítica — desbloquea trabajo real de specialists | Kernel Engineer |
| CORE-276 | feat | Specialist: aprobación de paths externos por el usuario | 📥 Todo | Alta | Kernel Engineer |
| CORE-277 | feat | Specialist: tool `web_search` | ✅ Done | Alta | Kernel Engineer |
| CORE-278 | feat + fix | Shell: TTS en modo texto + simplificar configuración de voz | ✅ Done | Alta | Shell Engineer |
| CORE-279 | fix | Kernel: WebSocket keepalive (ping cada 30s) | ✅ Done | Crítica | Kernel Engineer |
| CORE-280 | feat | Installer: Caddy HTTPS automático | ✅ Done | Alta | Kernel Engineer (modificación del installer) |
| CORE-282 | fix | chat_agent.md: eliminar instrucciones de token parser legacy | 📥 Todo | Crítica | Kernel Engineer |
| CORE-283 | fix | chat_agent: honestidad cuando el supervisor no retorna datos | ✅ Done | Alta | Kernel Engineer (modificación de chat_agent.md) |
| CORE-284 | fix | Shell: botón de reply al supervisor — fix envío | ✅ Done | Alta | Shell Engineer |
| CORE-285 | feat | Installer: configuración obligatoria de modelo antes de arrancar | ✅ Done | Crítica | Kernel Engineer (installer) |
| CORE-287 | fix | Kernel: fix project_id usa scope en lugar de nombre | ✅ Done | Alta | Kernel Engineer |
| CORE-288 | fix | Kernel: fix síntesis de reportes hijos (loop infinito potencial) | 📥 Todo | Alta | Kernel Engineer |
| CORE-289 | feat | Kernel: herramienta `get_agent_status` para el chat_agent | ✅ Done | Alta | Kernel Engineer |

## EPIC 51 — Model Intelligence

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-290 | chore | `tools/update_models.py`: Script de sincronización de modelos y scores | ✅ Done | Alta | Arquitecto IA |
| CORE-291 | chore | `models.yaml`: Actualizar scores y costos con datos reales | ✅ Done | Alta | Arquitecto IA |
| CORE-292 | feat | Kernel: provider `ollama_cloud` — URL remota + allowlist SSRF | ✅ Done | Alta | Kernel Engineer |
| CORE-293 | feat | `models.yaml`: Agregar modelos Ollama Cloud | ✅ Done | Media | Arquitecto IA |
| CORE-294 | feat | Shell: CatalogViewer — columna Benchmark y badge Ollama Cloud | ✅ Done | Alta | Shell Engineer |
| CORE-296 | feat | Installer: puerto HTTP configurable + soporte entorno multi-servicio | ✅ Done | Crítica | DevOps Engineer |
| CORE-297 | fix | chat_agent.md: flujo automático de proyectos y delegación | ✅ Done | Crítica | Arquitecto IA |
| CORE-298 | feat | Kernel: CatalogSyncer actualiza modelos free de OpenRouter al registrar key | ✅ Done | Alta | Kernel Engineer |
| CORE-299 | feat | Kernel: soporte de `model_override` en WebSocket chat | ✅ Done | Crítica | Kernel Engineer |
| CORE-300 | feat | Shell: selector de modelo en el chat | ✅ Done | Crítica | Shell Engineer |
| CORE-301 | feat | CMR v2: scoring contextual + latencia real + fix peso fantasma | ✅ Done | Alta | Kernel Engineer |
| CORE-305 | feat | Kernel: Arquitectura de Ruteo Cognitivo Asimétrico (Local-First) | ✅ Done | Alta | Arquitecto IA + Kernel Engineer |
| CORE-319 | fix | Kernel: CMR Router hardening — sticky cache, señales del tracker y catálogo | ✅ Done | Crítica | Kernel Engineer |
| CORE-320 | feat | Kernel: CMR Scoring v3 — confiabilidad observada, tool-use degradado y pooling multi-key | ✅ Done | Alta | Kernel Engineer |

## EPIC 52 — Voice Quality

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-295-p2 | fix | Shell: preservar stt_provider al guardar config de voz | ✅ Done | Alta | UI Engineer |
| CORE-295 | fix | Voice: fix voz metálica (Mock fallback) + key admin como fallback TTS | ✅ Done | Crítica |  |
| CORE-302 | fix | Shell: mutear micrófono durante reproducción TTS (feedback loop) | ✅ Done | Crítica | Shell Engineer |
| CORE-304 | feat | Voice: Migrar Siren Protocol de WebSockets a WebRTC/WebTransport | ✅ Done | Alta | Kernel Engineer + Shell Engineer |

## EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-245 | feat | Admin: toggle habilitar/deshabilitar provider sin eliminarlo | ✅ Done | Alta | Shell Engineer |
| CORE-246 | feat | Tenant: visualización de modelos disponibles por provider en tab Motor | ✅ Done | Alta | Shell Engineer |
| CORE-247 | feat | Historial de chat persistente: cargar al conectar y unificar entre IP y Cloudflare | ✅ Done | Crítica | Kernel + Shell |
| CORE-248 | feat | Chat: indicador de estado enriquecido (modelo, provider, error amigable) | 📥 Todo | Crítica | Shell Engineer |
| CORE-249 | feat | Dashboard: reemplazar MOCK_TICKETS con Kanban real del tenant | ✅ Done | Alta | Shell Engineer |
| CORE-250 | feat | Dashboard: FinancialWidget con datos reales (API Cost) | ✅ Done | Media | Shell Engineer |
| CORE-251 | feat | Dashboard: Chronos widget honesto (sin eventos ficticios) | ✅ Done | Media | Shell Engineer |
| CORE-252 | feat | Dashboard: header con nombre real del tenant y estado del sistema real | ✅ Done | Alta | Shell Engineer |
| CORE-253 | fix | Kernel: SYS_CALL_PLUGIN con plugin no encontrado debe devolver error legible al usuario | ✅ Done | Crítica | Kernel Engineer |
| CORE-255 | fix | Installer: registro robusto del servicio Windows + opción de reparación | ✅ Done | Crítica | DevOps Engineer |
| CORE-256 | feat | Admin: panel de gestión del servicio del sistema (start/stop/restart/status) | ✅ Done | Alta | Shell + Kernel |
| CORE-257 | fix | Kernel: Tunnel Manager no debe reintentar si cloudflared no está instalado | ✅ Done | Media | Kernel Engineer |
| CORE-258 | feat | CLI: soporte multiplataforma (Windows + Linux) en ank-cli | ✅ Done | Alta | Kernel Engineer |
| CORE-263 | feat | Comunicación Bottom-Up: herramienta ask_user + estado WaitingUser | ✅ Done | Alta | Kernel Engineer |
| CORE-265 | fix | ank-server: leer aegis.env antes de buscar variables de entorno | ✅ Done | Crítica | Kernel Engineer |
| CORE-266 | fix | ank-server: Windows Service Control Manager handshake | ✅ Done | Crítica | Kernel Engineer |
| CORE-281 | fix | Kernel: deduplicación de supervisores + project_name en system prompt | ✅ Done | Alta | Kernel Engineer |
| CORE-286 | fix | Kernel: timeout en run_agent_loop + cleanup de tasks zombies | ✅ Done | Alta | Kernel Engineer |
| CORE-303 | feat | Kernel: Defensive Cognitive Loops & Boundary Autocorrection | ✅ Done | Crítica | Kernel Engineer |
| CORE-306 | chore | Project: Consolidar Estabilización del Kernel y Congelamiento de Características Secundarias | ✅ Done | Alta | Tavo |
| CORE-321 | chore | App/Android: deshabilitar el file-watcher nativo de Gradle (crashes JVM en Windows) | ✅ Done | Alta | DevOps Engineer |

## EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-307 | feat | Relay server WebSocket tunnel protocol | ✅ Done | Alta | DevOps / Infra |
| CORE-308 | feat | Client agent integration & heartbeat | ✅ Done | Alta | Kernel Engineer |
| CORE-309 | feat | Shell: Connect status widget & Orion ID linking | ✅ Done | Alta | Shell Engineer |

## Otras Características Consolidadas

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-148 | fix | Fix: System prompt — conversación natural, sin respuestas robóticas | ✅ Done | Alta | Kernel Engineer |
| CORE-149 | feat | Feat: Neuronal Memory (L3) & Semantic Retrieval | ✅ Done | Crítica | Kernel Engineer |
| CORE-150 | feat | Feat: Sandbox de Scripts (Maker Capability) | ✅ Done | Alta | Kernel Engineer |
| CORE-151 | feat | Feat: Integración de Contexto de Proyecto (Git/VCM) | ✅ Done | Alta | Kernel Engineer |
| CORE-152 | feat | Feat: Plugins de Dominios (Ledger & Chronos) | ✅ Done | Media | Kernel Engineer |
| CORE-153 | feat | Feat: Dashboard Dinámico & Kanban UI | ✅ Done | Alta | UI Engineer |
| CORE-154 | feat | Feat: Orquestación de Sub-Agentes Especializados | ✅ Done | Baja | Kernel Engineer |
| CORE-155 | feat | feat: AgentNode, AgentRole y AgentState — Tipos Base del Sistema Multiagente | 📥 Todo | Crítica | Kernel Engineer |
| CORE-156 | feat | feat: AgentTree — Estructura en Memoria y Operaciones | ✅ Done | Crítica | Kernel Engineer |
| CORE-157 | feat | feat: AgentMessage — Protocolo de Comunicación Inter-Agente | ✅ Done | Crítica | Kernel Engineer |
| CORE-158 | feat | feat: AgentOrchestrator — Ciclo de Vida y Coordinación | 📥 Todo | Crítica | Kernel Engineer |
| CORE-159 | feat | feat: ProjectRegistry — Gestión de Proyectos y Supervisores Raíz | ✅ Done | Alta | Kernel Engineer |
| CORE-160 | feat | feat: PCB Extension — Campo agent_id en ProcessControlBlock | 📥 Todo | Alta | Kernel Engineer |
| CORE-161 | feat | feat: DagNode Extension — Campo agent_id en DagNode | 📥 Todo | Alta | Kernel Engineer |
| CORE-162 | feat | feat: SYS_AGENT_SPAWN — Spawn Dinámico de Agentes desde Syscall | ✅ Done | Alta | Kernel Engineer |
| CORE-163 | feat | feat: HTTP Routes /api/agents/* — Árbol de Agentes y Estado | 📥 Todo | Media | Kernel Engineer |
| CORE-164 | feat | feat: AgentTreeView + AgentCard — Visualización del Árbol en Shell | ✅ Done | Media | Shell Engineer |
| CORE-165 | feat | feat: Model-per-Agent — Integración CMR con TaskType por AgentNode | ✅ Done | Alta | Kernel Engineer |
| CORE-166 | feat | feat: Agent Tree Widget — Visualización del Árbol de Agentes en Dashboard del Tenant | 📥 Todo | Alta | Shell Engineer |
| CORE-167 | feat | feat: workspace_config — Tabla SQLCipher y Endpoint de Configuración | ✅ Done | Crítica (fundacional de la épica) | Kernel Engineer |
| CORE-168 | feat | feat: TerminalExecutor — Ejecución de Comandos con Streaming | ✅ Done | Crítica | Kernel Engineer |
| CORE-169 | feat | feat: SYS_EXEC — Syscall de Terminal para Agentes | ✅ Done | Alta | Kernel Engineer |
| CORE-170 | feat | feat: FileSystemBridge — Endpoints /api/fs/tree y /api/fs/file | ✅ Done | Alta | Kernel Engineer |
| CORE-171 | feat | feat: GitHubBridge — Identidad del Bot, Branch, Commit, Push y PR | ✅ Done | Crítica | Kernel Engineer |
| CORE-172 | feat | feat: SYS_GIT_* — Syscalls Git para Agentes | ✅ Done | Alta | Kernel Engineer |
| CORE-173 | feat | feat: PR Manager — Ciclo de Vida de PRs con Polling de CI | ✅ Done | Crítica | Kernel Engineer |
| CORE-174 | feat | feat: Auto-fix CI — Proceso Cognitivo Disparado por Fallo de CI | 📥 Todo | Alta | Kernel Engineer |
| CORE-175 | feat | feat: Eventos WebSocket — terminal_output, pr_update, pr_merged, git_push, ci_fix_attempt | ✅ Done | Crítica | Kernel Engineer |
| CORE-176 | feat | feat: TerminalPanel — UI de Terminal en Dashboard del Tenant | ✅ Done | Alta | Shell Engineer |
| CORE-177 | feat | feat: CodeViewer — Árbol de Archivos y Contenido en Dashboard | 📥 Todo | Alta | Shell Engineer |
| CORE-178 | feat | feat: GitTimeline — Branches, Commits y Estado de CI en Dashboard | 📥 Todo | Alta | Shell Engineer |
| CORE-179 | feat | feat: PRManager UI — Lista de PRs con Controles Auto/Manual | ✅ Done | Alta | Shell Engineer |
| CORE-180 | feat | feat: WorkspaceSettings — Configuración de Token, Repo y Opciones | ✅ Done | Alta | Shell Engineer |
| CORE-181 | fix | fix: MakerExecutor — Boa Engine context sin top-level return ni CommonJS | ✅ Done | Alta | Kernel Engineer |
| CORE-182 | fix | fix: TTS ausente en modo local — invocar voiceService.speak en WebSocket onDone | 📥 Todo | Alta | Shell Engineer |
| CORE-183 | feat | feat: Input Mode Selector — Texto / Audio / Conversación en shell web | 📥 Todo | Alta | Shell Engineer |
| CORE-184 | fix | fix: modo conversación — sirenWs persistente + TTS loop sin botón | 📥 Todo | Crítica | Shell Engineer |
| CORE-185 | feat | feat: TTS pipeline en WebSocket Siren — sintetizar respuesta y enviar chunks al frontend | 📥 Todo | Crítica | Kernel Engineer |
| CORE-186 | feat | feat: TTS engine local con espeak-ng — sintetizar voz sin API key | 📥 Todo | Crítica | Kernel Engineer |
| CORE-187 | feat | feat: Wake Word Detection — activación por voz "Aegis" sin botón | ✅ Done | Media | Kernel Engineer (backend) + Shell Engineer (frontend) |
| CORE-188 | fix | fix: Onboarding extrae nombre completo literal en lugar de solo el nombre | 📥 Todo | Alta | Kernel Engineer |
| CORE-189 | fix | fix: Enclave se reinicializa cada 126 segundos — session keep-alive abre conexión SQLCipher innecesariamente | 📥 Todo | Alta | Kernel Engineer |
| CORE-202 | feat | feat: AgentActivityPanel — indicador colapsable en ChatTerminal | 📥 Todo | Alta | Shell Engineer |
| CORE-203 | feat | feat: AgentTreeView + ProjectList — panel en Dashboard | 📥 Todo | Media | Shell Engineer |
| CORE-204 | feat | feat: useAegisStore — agentTree state + connectAgentStream | 📥 Todo | Alta | Shell Engineer |
| CORE-209 | fix | fix(ank-http): montar /ws/agents y agregar GET /api/agents/projects en router | 📥 Todo | Crítica | Kernel Engineer |
| CORE-210 | fix | fix(chat_agent): declarar ausencia de contexto cuando no hay proyecto activo | 📥 Todo | Alta | Kernel Engineer |
| CORE-211 | fix | fix(shell): manejo graceful de errores en fetchActiveProjects y connectAgentStream | 📥 Todo | Alta | Shell Engineer |
| CORE-212 | fix | fix(shell): provider gemini en KeyManager y visibilidad de modelos en CatalogViewer | 📥 Todo | Crítica | Shell Engineer |
| CORE-213 | fix | Kernel: loguear error en key_pool.load() | ✅ Done | Media | Kernel Engineer |
| CORE-224 | chore | Limpiar directorios temporales | ✅ Done | Baja | Tavo |
| CORE-225 | chore | License field en Cargo.toml → MIT | ✅ Done | Alta | Kernel Engineer |
| CORE-226 | fix | fix(ank-core): Chat Agent usa SYSTEM_PROMPT_MASTER genérico en lugar de chat_agent.md | ✅ Done | Crítica | Kernel Engineer |
| CORE-227 | fix | fix(ank-core): SPAWN_INSTRUCTIONS usa sintaxis obsoleta — divergencia con parser | ✅ Done | Crítica | Kernel Engineer |
| CORE-228 | fix | fix(ank-server): SyscallExecutor se crea sin AgentOrchestrator — SYS_AGENT_SPAWN siempre falla | ✅ Done | Crítica | Kernel Engineer |
| CORE-229 | fix | fix(installer): agents config no se despliega en producción | ✅ Done | Alta | DevOps Engineer |
| CORE-230 | fix | fix(shell): Dashboard crashea al montar y dispara AegisErrorBoundary ("Session Error") | ✅ Done | Crítica | Shell Engineer |
| CORE-231 | fix | fix(shell): Micrófono falla silenciosamente en HTTP — falta feedback al usuario | ✅ Done | Crítica | Shell Engineer |
| CORE-232 | fix | fix(shell): IP vs Cloudflare producen chats separados — misma sesión, contexto diferente | ✅ Done | Alta | Shell Engineer |
| CORE-233 | feat (refactor UX) | feat(shell+kernel): Settings del tenant — simplificar y unificar configuración | 📥 Todo | Alta | Shell Engineer |
| CORE-234 | feat | feat(ank-core): AgentOrchestrator — migrar de token parsing a tool use dispatch | ✅ Done | Alta | Kernel Engineer |
| CORE-235 | feat | feat(ank-core): SyscallExecutor — mapear tool call results a AgentMessage internos | ✅ Done | Alta | Kernel Engineer |
| CORE-236 | feat | feat(ank-core): ToolRegistry — definición de herramientas + schema por proveedor | ✅ Done | Alta | Kernel Engineer |
| CORE-237 | feat | feat(ank-core): Ollama fallback — detección de tool use support + modo degradado | ✅ Done | Alta | Kernel Engineer |
| CORE-238 | feat | docs: Agent files + PROTOCOL.md — reescritura post tool use | ✅ Done | Alta | Kernel Engineer |
| CORE-239 | fix | fix(ank-core): CognitiveRouter — model_id con prefijo provoca 404 en APIs directas (Groq, Anthropic, etc.) | ✅ Done | Crítica | Kernel Engineer |
| CORE-240 | feat | fix(ank-core): conectar ToolRegistry → CloudProxyDriver → API request | ✅ Done | Alta | Kernel Engineer |
| CORE-241 | feat | fix(ank-core): filtrar __TOOL_CALL__ del output antes de enviarlo al frontend | ✅ Done | Alta | Kernel Engineer |
| CORE-242 | fix | fix(ank-core): eliminar MAKER_INSTRUCTIONS del prompt del Chat Agent | ✅ Done | Alta | Kernel Engineer |
| CORE-243 | feat | fix(ank-core): SyscallExecutor permite spawn desde Chat Agent (PCB sin agent_id) | ✅ Done | Alta | Kernel Engineer |
| CORE-244 | fix | HAL Runner: enviar StatusUpdate al event_broker en el path de error | 📥 Todo | Crítica | Kernel Engineer |
| CORE-314 | fix | Kernel/Shell: Deshabilitación de proveedores en KeyPool/UI no persiste | ✅ Done | Alta | Tavo + Kernel Engineer + Shell Engineer |
| CORE-315 | feat | Kernel/Shell: Exportar/importar configuración de llaves cifrada con contraseña | ✅ Done | Media | Kernel Engineer + UI Engineer |
| CORE-316 | fix | postcss >= 8.5.10 en shell/ui para mitigar XSS (GHSA-qx2v-qp2m-jg93) | ✅ Done | Alta | UI Engineer |
| CORE-317 | chore | Clippy fixes: simplificar lógicas booleanas, mut innecesarios y rangos | ✅ Done | Alta | Kernel Engineer |
| CORE-318 | chore | UX fixes: skeletons de telemetría y accesibilidad ARIA en DynamicModulePanel | ✅ Done | Alta | UI Engineer |

## EPIC 55 — Mobile App (Orion ID & Web Redirection)

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-310 | fix | Fix TypeScript Icon type compiler error (Spotify icon) | ✅ Done | Alta | UI Engineer |
| CORE-311 | feat | Orion ID login tab and automatic domain resolution | ✅ Done | Alta | App Developer |
| CORE-312 | feat | connected-accounts Web redirect button via expo-web-browser | ✅ Done | Alta | App Developer |

## Governance & Tooling

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-313 | chore | Reconciliación completa de TICKETS_MASTER + script anti-drift | 📥 Todo | Media | DevOps + Arquitecto IA |

---

*Leyenda: 📥 Todo · 🚧 In Progress · ✅ Done · ❌ Blocked · ⚠️ Revisar*
