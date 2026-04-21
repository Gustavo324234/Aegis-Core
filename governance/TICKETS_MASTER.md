# TICKETS_MASTER.md — Aegis Core

> Fuente de verdad única para todos los tickets del monorepo Aegis-Core.

---

## 🏗️ EPIC 32: Unified Binary — Sistema completo en Aegis-Core
**Status:** ✅ DONE — 2026-04-08
**Resultado:** Un único binario Rust (`ank-server`) sirve HTTP/WS (:8000) y gRPC (:50051).
Sin BFF Python. Sin dependencias de runtime externas. 33/33 tickets completados.

### Fase 1 — Fundación Rust (kernel/)
*   **[CORE-001]** Workspace Cargo.toml raíz + estructura de crates `[DONE]`
*   **[CORE-002]** ank-proto: contratos Protobuf portados desde legacy `[DONE]`
*   **[CORE-003]** ank-core: motor cognitivo portado desde legacy `[DONE]`
*   **[CORE-004]** ank-mcp: cliente MCP portado desde legacy `[DONE]`
*   **[CORE-005]** aegis-sdk: Wasm SDK portado desde legacy `[DONE]`
*   **[CORE-006]** ank-cli: CLI administrativa portada desde legacy `[DONE]`

### Fase 2 — Servidor HTTP nativo (kernel/crates/ank-http/)
*   **[CORE-010]** ank-http: scaffolding del crate + AppState `[DONE]`
*   **[CORE-011]** ank-http: CitadelLayer — middleware Axum de autenticación `[DONE]`
*   **[CORE-012]** ank-http: endpoints REST /api/auth, /api/admin/*, /api/engine/* `[DONE]`
*   **[CORE-013]** ank-http: endpoints REST /api/router/*, /api/status, /api/workspace/* `[DONE]`
*   **[CORE-014]** ank-http: WebSocket /ws/chat/{tenant_id} — streaming cognitivo `[DONE]`
*   **[CORE-015]** ank-http: WebSocket /ws/siren/{tenant_id} — audio bidireccional `[DONE]`
*   **[CORE-016]** ank-http: static file serving — SPA React embebida `[DONE]`

### Fase 3 — Entrypoint unificado (kernel/crates/ank-server/)
*   **[CORE-020]** ank-server: main.rs — levanta Axum + Tonic en mismo proceso Tokio `[DONE]`
*   **[CORE-021]** aegis-supervisor: portado y actualizado para un solo proceso `[DONE]`

### Fase 4 — Web UI (shell/)
*   **[CORE-030]** shell/ui: setup React + Vite + TypeScript + Zustand + Tailwind `[DONE]`
*   **[CORE-031]** shell/ui: stores Zustand portados desde legacy `[DONE]`
*   **[CORE-032]** shell/ui: componentes core — ChatTerminal, TelemetrySidebar, AdminDashboard `[DONE]`
*   **[CORE-033]** shell/ui: componentes auth — LoginScreen, BootstrapSetup, EngineSetupWizard `[DONE]`
*   **[CORE-034]** shell/ui: componentes providers — ProvidersTab, RouterConfig, SirenConfigTab `[DONE]`
*   **[CORE-035]** shell/ui: Siren UI — VoiceButton, TTSPlayer `[DONE]`
*   **[CORE-036]** shell/ui: build integrado — dist/ servido por ank-server `[DONE]`

### Fase 5 — Installer (installer/)
*   **[CORE-040]** installer: install.sh unificado — modo nativo + Docker `[DONE]`
*   **[CORE-041]** installer: docker-compose.yml — contenedor único `[DONE]`
*   **[CORE-042]** installer: systemd unit para modo nativo `[DONE]`
*   **[CORE-043]** installer: aegis CLI — start/stop/status/logs/update/token `[DONE]`

### Fase 6 — Mobile App (app/)
*   **[CORE-050]** app: setup React Native + Expo SDK 52 `[DONE]`
*   **[CORE-051]** app: stores, servicios y tipos portados `[DONE]`
*   **[CORE-052]** app: pantallas y navegación `[DONE]`

### Fase 7 — CI/CD y Governance
*   **[CORE-060]** CI: GitHub Actions — build + clippy + test unificado `[DONE]`
*   **[CORE-061]** CI: Docker publish — imagen única desde este repo `[DONE]`
*   **[CORE-062]** CI: Native binary publish — GitHub Releases `[DONE]`
*   **[CORE-063]** governance: AEGIS_CONTEXT.md y AEGIS_MASTER_CODEX.md `[DONE]`

---

## 🔮 EPIC 33: Linux Distribution (distro/)
**Status:** PLANNED — post smoke test en producción

*   **[CORE-100]** Definir base: Buildroot vs NixOS `[TODO]`
*   **[CORE-101]** Configuración de imagen x86_64 `[TODO]`
*   **[CORE-102]** Configuración de imagen ARM64 `[TODO]`
*   **[CORE-103]** ANK como servicio de sistema (systemd unit prioritario) `[TODO]`
*   **[CORE-104]** Root filesystem read-only + partición /data cifrada `[TODO]`

---

## 🔧 EPIC 34: Audit Fixes — Post-Consolidación
**Status:** ✅ DONE — 2026-04-13

### Shell — 7 tickets
*   **[CORE-070]** Fix: WebSocket URL hardcodeada con puerto 8000 `[DONE]`
*   **[CORE-071]** Fix: Credenciales admin en query params — migrar a headers Citadel `[DONE]`
*   **[CORE-072]** Fix: `isAdmin` determinado en el cliente por nombre de tenant `[DONE]`
*   **[CORE-073]** Fix: `sessionKey` (contraseña) persistida en localStorage `[DONE]`
*   **[CORE-079]** Fix: `SystemTab` pasa `session_key` en query param de `/api/status` `[DONE]`
*   **[CORE-083]** Fix: `ProvidersTab` usa credenciales hardcodeadas y query params `[DONE]`
*   **[CORE-088]** Fix: `ChatTerminal` envía `session_key` en FormData del file upload `[DONE]`

### Kernel — 13 tickets
*   **[CORE-074]** Fix: `get_sync_version` usa path relativo `VERSION` `[DONE]`
*   **[CORE-075]** Fix: `engine_config.json` path relativo — persistir en `data_dir` `[DONE]`
*   **[CORE-076]** Fix: `set_hw_profile` sin autenticación real `[DONE]`
*   **[CORE-077]** Fix: `ws/siren.rs` mock — conectar al SirenRouter real `[DONE]`
*   **[CORE-078]** Fix: Eliminar `AEGIS_DEV_MASTER_BYPASS` de producción `[DONE]`
*   **[CORE-080]** Fix: gRPC `server.rs` — implementar métodos Prioridad 1 y 2 `[DONE]`
*   **[CORE-081]** Fix: `CloudProxyDriver` — Anthropic via OpenRouter `[DONE]`
*   **[CORE-082]** Fix: `auth_interceptor` gRPC — headers Citadel parciales `[DONE]`
*   **[CORE-084]** Fix: `models.yaml` — URLs explícitas por provider `[DONE]`
*   **[CORE-085]** Fix: Scheduler → HAL via `execution_tx` — chat conectado `[DONE]`
*   **[CORE-086]** Fix: `router_api.rs` — `authenticate_master` en lugar de `authenticate_tenant` `[DONE]`
*   **[CORE-087]** Fix: `siren_api.rs` — `session_key` fuera de query params `[DONE]`
*   **[CORE-089]** Fix: `providers.rs` — autenticación + validación SSRF `[DONE]`
*   **[CORE-090]** Fix: WAL checkpoint race en `initialize_master` `[DONE]` — 2026-04-14

---

## 🔧 EPIC 35: Hardening Post-Launch — Recomendaciones de Auditorías Multi-Modelo
**Status:** ✅ DONE — 2026-04-16

### P1 — Seguridad y Estabilidad
*   **[CORE-091]** Rate Limiting en `/api/auth/login` `[DONE]`
*   **[CORE-092]** Watchdog HAL Runner `[DONE]`
*   **[CORE-093]** Reuse de `CloudProxyDriver` via `Arc<reqwest::Client>` `[DONE]`
*   **[CORE-094]** `tokio::sync::Mutex` en `CognitiveHAL` `[DONE]`
*   **[CORE-095]** Retry con Exponential Backoff en `CloudProxyDriver` `[DONE]`
*   **[CORE-096]** Verificar y documentar flujo SHA-256 + Argon2id `[DONE]`

### P2 — Deuda técnica
*   **[CORE-097]** Preemption real via `CancellationToken` `[DONE]`
*   **[CORE-098]** Investigación LanceDB / VCM L3 `[DONE]` — ADR-038: fast-hnsw/usearch
*   **[CORE-099]** CI: stubs gRPC Python `[N/A]`
*   **[CORE-105]** Telemetría tokens/s y costo estimado `[DONE]`
*   **[CORE-106]** Latencia real en `CognitiveRouter` `[DONE]`
*   **[CORE-107]** Documentación OpenAPI/Swagger `[DONE]`

### P3 — Experiencia y mantenibilidad
*   **[CORE-108]** Indicador UI cuando STT no está disponible `[DONE]`

### Duplicados — CLOSED
*   **[CORE-110 a CORE-120]** Duplicados cerrados — ver histórico

---

## 🔮 EPIC 36: Post-Launch Improvements — VCM L3
**Status:** ✅ DONE — 2026-04-20

*   **[CORE-109]** Implementar `usearch` en `LanceSwapManager` (VCM L3) `[DONE]` — Kernel Engineer

---

## 🐛 SMOKE TEST BUGS — 2026-04-20
**Smoke test:** PASS ✅ — chat end-to-end operativo con OpenRouter

*   **[CORE-121]** Fix: `openrouter/free` ausente en `models.yaml` `[DONE]`
*   **[CORE-122]** Installer: perfil de inferencia Cloud/Local/Hybrid + `ModelProfile` en Kernel `[DONE]`
*   **[CORE-123]** Fix: LLM generaba syscalls — `[USER_PROCESS_INSTRUCTION]` confundía al modelo `[DONE]`

---

## 🎨 EPIC 37: UX Polish — Post Smoke Test
**Status:** ✅ DONE — 2026-04-20

*   **[CORE-124]** Fix: Historial del chat se borra al recargar `[DONE]`
*   **[CORE-125]** Fix: Micrófono bloqueado en HTTP — HTTPS via Caddy `[DONE]`
*   **[CORE-126]** Fix: Panel de telemetría ilegible — tamaños aumentados `[DONE]`
*   **[CORE-127]** Fix: Catálogo Gemini desactualizado — agregado 2.5 Pro y Flash `[DONE]`

---

## 🚀 SISTEMA STATUS — 2026-04-20

| Componente | Estado |
|---|---|
| Epic 32: Monorepo Unificado | ✅ DONE |
| Epic 34: Audit Fixes | ✅ DONE |
| Epic 35: Hardening Post-Launch | ✅ DONE |
| Epic 36: VCM L3 (usearch) | ✅ DONE |
| Epic 37: UX Polish | ✅ DONE |
| Chat end-to-end | ✅ OPERATIVO |
| Smoke test producción | ✅ PASS |
| Epic 33: Linux Distribution | 🔮 PLANNED |

---

*Última actualización: 2026-04-20 — Smoke test PASS. Epic 37 completo.*
