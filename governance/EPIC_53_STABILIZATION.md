# EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure

## Objetivo

Resolver todos los bugs críticos de funcionamiento descubiertos en las primeras
sesiones reales de uso, completar la observabilidad del dashboard, y dejar la
infraestructura lista para deploy en entornos multi-servicio (VPS).

Este epic no agrega features nuevas — consolida y estabiliza todo lo que está
incompleto o roto antes de abrir Aegis a más usuarios.

---

## Fase 1 — Agent Loop (Crítico)

Bugs que impiden que el sistema multi-agente funcione correctamente en producción.

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-300 | Kernel: aislamiento cross-tenant en ProjectRegistry | Kernel Engineer | Crítica |
| CORE-298 | Kernel: dispatch post-spawn falla — No channel for agent | Kernel Engineer | Crítica |
| CORE-262 | AgentOrchestrator: inferencia LLM real en run_agent_loop | Kernel Engineer | Crítica |
| CORE-297 | chat_agent.md: flujo automático de proyectos y delegación | Arquitecto IA | Crítica |
| CORE-263 | Herramienta ask_user + estado WaitingUser + enrutamiento Chat Agent | Kernel Engineer | Alta |

**Orden de implementación:** CORE-300 → CORE-298 → CORE-262 → CORE-297 → CORE-263

---

## Fase 2 — Shell Observability (Alta)

Dashboard y feedback visual con datos reales en lugar de mocks.

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-299 | Shell: timeout del cliente desincronizado con el ReAct loop | Shell + Kernel | Alta |
| CORE-301 | Shell: AgentTreeWidget unavailable | Shell Engineer | Alta |
| CORE-248 ✅ | Chat: indicador de estado enriquecido | Shell Engineer | ✅ Done |
| CORE-252 | Dashboard: header con nombre real del tenant y estado del sistema | Shell Engineer | Alta |
| CORE-249 | Dashboard: reemplazar MOCK_TICKETS con Kanban real del tenant | Shell Engineer | Alta |
| CORE-253 | Kernel: SYS_CALL_PLUGIN error legible al usuario | Kernel Engineer | Crítica |
| CORE-256 | Admin: tab Sistema — gestión del servicio desde la UI | Shell + Kernel | Alta |
| CORE-250 | Dashboard: FinancialWidget con datos reales (API Cost) | Shell Engineer | Media |
| CORE-251 | Dashboard: Chronos widget honesto (sin eventos ficticios) | Shell Engineer | Media |
| CORE-257 | Kernel: Tunnel Manager — no reintentar si cloudflared no instalado | Kernel Engineer | Media |

---

## Fase 3 — UX & Providers (Alta)

Completar la experiencia de configuración de providers y persistencia del chat.

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-247 | Historial de chat persistente: cargar al conectar, unificar IP y Cloudflare | Kernel + Shell | Crítica |
| CORE-245 | Admin: toggle habilitar/deshabilitar provider sin eliminarlo | Shell Engineer | Alta |
| CORE-246 | Tenant: visualización de modelos disponibles por provider en tab Motor | Shell Engineer | Alta |

---

## Fase 4 — Model Intelligence (Media)

Completar soporte para Ollama Cloud y scores de benchmarks.

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-292 | Kernel: provider `ollama_cloud` — URL remota + allowlist SSRF | Kernel Engineer | Alta |
| CORE-293 | `models.yaml` — agregar modelos Ollama Cloud | Arquitecto IA | Media |
| CORE-294 | Shell: CatalogViewer — columna Benchmark score + badge Ollama Cloud | Shell Engineer | Alta |

---

## Fase 5 — Infraestructura (Alta)

Dejar el deploy robusto para entornos reales (VPS multi-servicio, Windows, CLI).

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-296 | Installer: puerto HTTP configurable + soporte entorno multi-servicio | DevOps Engineer | Crítica |
| CORE-255 | Installer: registro robusto del servicio Windows + opción -Repair | DevOps Engineer | Crítica |
| CORE-258 | CLI: ank-cli multiplataforma — Windows + Linux + CI | Kernel Engineer | Alta |

---

## Fase 6 — Deuda técnica y cleanup (Baja)

| ID | Título | Asignado a | Prioridad |
|---|---|---|---|
| CORE-224 | Limpiar directorios temporales | Tavo | Baja |
| CORE-225 | License field en Cargo.toml → MIT | Kernel Engineer | Alta |
| CORE-213 | Kernel: loguear error en key_pool.load() al arranque | Kernel Engineer | Media |
| OPS-001 | Re-registrar API keys tras reinicio (workaround manual) | Tavo | Crítica |

---

## Criterios de completitud del Epic

- [ ] Fase 1 completa: el pipeline multi-agente funciona end-to-end sin errores en logs
- [ ] Fase 1 completa: no hay datos cross-tenant bajo ninguna circunstancia
- [ ] Fase 2 completa: el dashboard no muestra ningún dato mock ni widget "unavailable"
- [ ] Fase 2 completa: timeouts alineados entre cliente y servidor
- [ ] Fase 3 completa: el historial de chat persiste entre sesiones
- [ ] Fase 4 completa: Ollama Cloud configurable desde la UI
- [ ] Fase 5 completa: `install.sh` en VPS con puerto 8000 ocupado funciona sin intervención manual
- [ ] Fase 6 completa: licencia MIT en Cargo.toml, directorios limpios

---

## Notas

- EPIC 52 (Voice Quality — CORE-295) corre en paralelo, no es bloqueante
- CORE-148 (tono conversacional) sigue In Progress independientemente
- CORE-150, 151 (Maker Capability, Git Context) quedan en EPIC 42 — no se absorben aquí
