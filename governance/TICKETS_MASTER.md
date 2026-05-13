# Aegis Core — TICKETS MASTER

## 🚀 Epics

| ID | Título | Estado | Progreso |
|---|---|---|---|
| EPIC 41 | UX & Onboarding | En Curso | 80% |
| EPIC 42 | Vision Realignment & Autonomy | En Curso | 45% |
| EPIC 43 | Hierarchical Multi-Agent Orchestration | ✅ Completa | 100% |
| EPIC 44 | Developer Workspace | ✅ Completa | 100% |
| EPIC 45 | Cognitive Agent Architecture (CAA) | ✅ Completa | 100% |
| EPIC 46 | Public Launch | ✅ Completa | 100% |
| EPIC 51 | Model Intelligence: PinchBench + Ollama Cloud + CMR v2 | 🚧 In Progress | 30% |
| EPIC 52 | Voice Quality | 🚧 In Progress | 0% |

---

## 🎫 Tickets

### EPIC 51 — Model Intelligence

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-290 | chore | `tools/update_models.py` — script sincronización modelos y scores | ✅ Done | Alta | Arquitecto IA |
| CORE-291 | chore | `models.yaml` — actualizar scores y costos con datos reales | ✅ Done | Alta | Arquitecto IA |
| CORE-292 | feat | Kernel: provider `ollama_cloud` — URL remota + allowlist SSRF | 📥 Todo | Alta | Kernel Engineer |
| CORE-293 | feat | `models.yaml` — agregar modelos Ollama Cloud | 📥 Todo | Media | Arquitecto IA |
| CORE-294 | feat | Shell: CatalogViewer — columna Benchmark + badge Ollama Cloud | 📥 Todo | Alta | Shell Engineer |
| CORE-295 | fix | Voice: voz metálica + key admin fallback TTS + preservar stt_provider | 📥 Todo | Crítica | Kernel + Shell |
| CORE-297 | fix | `enginePresets.ts` — agregar `ollama_cloud` al modal Link Provider | ✅ Done | Crítica | Arquitecto IA |
| CORE-298 | feat | Kernel: CatalogSyncer — sync modelos free OpenRouter al registrar key | 📥 Todo | Alta | Kernel Engineer |
| CORE-299 | feat | Kernel: `model_override` en PCB y WebSocket chat | 📥 Todo | Crítica | Kernel Engineer |
| CORE-300 | feat | Shell: selector de modelo en barra del chat | 📥 Todo | Crítica | Shell Engineer |
| CORE-301 | feat | CMR v2: scoring contextual + latencia real + fix peso fantasma | 📥 Todo | Alta | Kernel Engineer |

---

### EPIC 52 — Voice Quality

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-295 | fix | SirenRouter fallback a key admin + preservar stt_provider al guardar | 📥 Todo | Crítica | Kernel + Shell |

---

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
| CORE-212 | fix | Shell: provider gemini en KeyManager + CatalogViewer | ✅ Done | Crítica |

---

### Deuda técnica y operaciones

| ID | Tipo | Título | Estado | Prioridad | Asignado a |
|---|---|---|---|---|---|
| CORE-225 | chore | License field en Cargo.toml → MIT | 📥 Todo | Alta | Kernel Engineer |
| CORE-213 | fix | Kernel: loguear error en key_pool.load() al arranque | 📥 Todo | Media | Kernel Engineer |
| CORE-224 | chore | Limpiar directorios temporales | 📥 Todo | Baja | Tavo |
| CORE-247 | feat | Historial de chat persistente: unificar IP y Cloudflare | 📥 Todo | Crítica | Kernel + Shell |
| CORE-245 | feat | Admin: toggle habilitar/deshabilitar provider | 📥 Todo | Alta | Shell Engineer |
| CORE-246 | feat | Tenant: modelos disponibles por provider en tab Motor | 📥 Todo | Alta | Shell Engineer |
| CORE-252 | feat | Dashboard: header con nombre real del tenant | 📥 Todo | Alta | Shell Engineer |
| CORE-253 | fix | Kernel: SYS_CALL_PLUGIN error legible al usuario | 📥 Todo | Crítica | Kernel Engineer |
| CORE-255 | fix | Installer: registro robusto servicio Windows | 📥 Todo | Crítica | DevOps Engineer |
| CORE-256 | feat | Admin: tab Sistema — gestión del servicio desde UI | 📥 Todo | Alta | Shell + Kernel |
| CORE-257 | fix | Kernel: Tunnel Manager — no reintentar sin cloudflared | 📥 Todo | Media | Kernel Engineer |
| CORE-258 | feat | CLI: ank-cli multiplataforma | 📥 Todo | Alta | Kernel Engineer |
| OPS-001 | ops | Re-registrar API keys tras reinicio (manual) | 📥 Todo | Crítica | Tavo |

---

*Leyenda: 📥 Todo · 🚧 In Progress · ✅ Done · ❌ Blocked*
