# Aegis Core — TICKETS MASTER

## 🚀 Epics

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | ✅ Completa | 95% |
| EPIC 42 | Vision Realignment & Autonomy | 🚧 In Progress | 60% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | ✅ Completa | 100% |
| EPIC 44 | Developer Workspace | ✅ Completa | 100% |
| EPIC 45 | Cognitive Agent Architecture (CAA) | ✅ Completa | 100% |
| EPIC 46 | Public Launch | ✅ Completa | 100% |
| EPIC 51 | Model Intelligence: PinchBench + Ollama Cloud + CMR v2 | ✅ Completa | 100% |
| EPIC 52 | Voice Quality | ✅ Completa | 100% |
| EPIC 53 | Stabilization: Agent Loop, Observability & Infrastructure | ✅ Completa | 100% |

---

## EPIC 51 — Model Intelligence

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-290 | chore | `tools/update_models.py` — script sincronización | ✅ Done | Alta |
| CORE-291 | chore | `models.yaml` — scores y costos reales | ✅ Done | Alta |
| CORE-292 | feat | Kernel: provider `ollama_cloud` | ✅ Done | Alta |
| CORE-293 | feat | `models.yaml` — modelos Ollama Cloud | ✅ Done | Media |
| CORE-294 | feat | Shell: CatalogViewer — columna Benchmark + badges | ✅ Done | Alta |
| CORE-295 | fix | SirenRouter: fallback a perfil admin (voz metálica) | ✅ Done | Crítica |
| CORE-297 | fix | `enginePresets.ts` — ollama_cloud en modal Link Provider | ✅ Done | Crítica |
| CORE-298 | feat | Kernel: CatalogSyncer — sync modelos free OpenRouter | ✅ Done | Alta |
| CORE-299 | feat | Kernel: `model_override` en PCB y WS chat | ✅ Done | Crítica |
| CORE-300 | feat | Shell: selector de modelo en barra del chat | ✅ Done | Crítica |
| CORE-301 | feat | CMR v2: scoring contextual + latencia real | ✅ Done | Alta |
| CORE-302 | fix | Shell: mutear mic durante TTS (feedback loop) | ✅ Done | Crítica |
| CORE-305 | feat | Kernel: Arquitectura de Ruteo Cognitivo Asimétrico (Local-First) | ✅ Done | Alta |

---

## EPIC 52 — Voice Quality

| ID | Tipo | Título | Estado | Prioridad |
|---|---|---|---|---|
| CORE-295 | fix | SirenRouter: fallback a perfil admin para tenants sin config | ✅ Done | Crítica |
| CORE-295-p2 | fix | Shell: preservar stt_provider al guardar config de voz | ✅ Done | Alta |
| CORE-302 | fix | Shell: mutear mic durante reproducción TTS | ✅ Done | Crítica |
| CORE-304 | feat | Voice: Migrar Siren Protocol de WebSockets a WebRTC/WebTransport | ✅ Done | Alta |

---

## EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure

### Fase 1 — Agent Loop (Crítico)

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-262 | feat | AgentOrchestrator: inferencia LLM real en run_agent_loop | ✅ Done | Crítica | Kernel Engineer |
| CORE-263 | feat | Herramienta ask_user + estado WaitingUser | ✅ Done | Alta | Kernel Engineer |
| CORE-303 | feat | Kernel: Defensive Cognitive Loops & Boundary Autocorrection | ✅ Done | Crítica | Kernel Engineer |

### Fase 2 — Shell Observability

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-252 | feat | Dashboard: header con nombre real del tenant | ✅ Done | Alta | Shell Engineer |
| CORE-249 | feat | Dashboard: Kanban real del tenant | ✅ Done | Alta | Shell Engineer |
| CORE-253 | fix | Kernel: SYS_CALL_PLUGIN error legible | ✅ Done | Crítica | Kernel Engineer |
| CORE-256 | feat | Admin: tab Sistema — gestión del servicio | ✅ Done | Alta | Shell + Kernel |
| CORE-250 | feat | Dashboard: FinancialWidget datos reales | ✅ Done | Media | Shell Engineer |
| CORE-251 | feat | Dashboard: Chronos widget honesto | ✅ Done | Media | Shell Engineer |
| CORE-257 | fix | Kernel: Tunnel Manager sin cloudflared | ✅ Done | Media | Kernel Engineer |

### Fase 3 — UX & Providers

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-247 | feat | Historial de chat persistente entre sesiones | ✅ Done | Crítica | Kernel + Shell |
| CORE-245 | feat | Admin: toggle habilitar/deshabilitar provider | ✅ Done | Alta | Shell Engineer |
| CORE-246 | feat | Tenant: modelos disponibles por provider | ✅ Done | Alta | Shell Engineer |

### Fase 4 — Model Intelligence

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-293 | feat | `models.yaml` — modelos Ollama Cloud | ✅ Done | Media | Arquitecto IA |

### Fase 5 — Infraestructura

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-255 | fix | Installer: registro robusto servicio Windows | ✅ Done | Crítica | DevOps Engineer |
| CORE-258 | feat | CLI: ank-cli multiplataforma | ✅ Done | Alta | Kernel Engineer |
| CORE-296 | feat | Installer: puerto HTTP configurable | ✅ Done | Crítica | DevOps Engineer |

### Fase 6 — Deuda técnica

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-225 | chore | License field en Cargo.toml → MIT | ✅ Done | Alta | Kernel Engineer |
| CORE-213 | fix | Kernel: loguear error en key_pool.load() | ✅ Done | Media | Kernel Engineer |
| CORE-224 | chore | Limpiar directorios temporales | ✅ Done | Baja | Tavo |
| CORE-306 | chore | Project: Consolidar Estabilización del Kernel y Congelamiento de Características Secundarias | ✅ Done | Alta | Tavo |
| OPS-001 | ops | Re-registrar API keys tras reinicio | ✅ Done | Crítica | Tavo |

---

*Leyenda: 📥 Todo · 🚧 In Progress · ✅ Done · ❌ Blocked*
