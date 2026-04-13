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

*Creado: 2026-04-08 | Arquitecto IA*
*Epic 32 cerrada: 2026-04-08 — 33/33 tickets DONE*

---

## 🔧 EPIC 34: Audit Fixes — Post-Consolidación
**Status:** IN-PROGRESS — 2026-04-13
**Motivación:** Auditoría post-migración a monorepo detectó bugs de seguridad y correctness
introducidos durante la consolidación de repos legacy en Aegis-Core.

### Shell
*   **[CORE-070]** Fix: WebSocket URL hardcodeada con puerto 8000 `[DONE — 2026-04-13]`
*   **[CORE-071]** Fix: Credenciales admin en query params — migrar a headers Citadel `[DONE — 2026-04-13]`
*   **[CORE-072]** Fix: `isAdmin` determinado en el cliente por nombre de tenant `[DONE — 2026-04-13]`
*   **[CORE-073]** Fix: `sessionKey` (contraseña) persistida en localStorage `[DONE — 2026-04-13]`

### Kernel
*   **[CORE-074]** Fix: `get_sync_version` usa path relativo `VERSION` — usar `env!("CARGO_PKG_VERSION")` `[DONE — Kernel Engineer]`

---

*Epic 34 creada: 2026-04-13 — Arquitecto IA (auditoría post-consolidación)*

## 🏁 UPDATE — 2026-04-13 (EPIC 34 — SEGUNDA RONDA DE AUDITORÍA)
- CORE-075 creado: engine_config.json con path relativo — motor olvida config en cada restart
- CORE-076 creado: set_hw_profile sin autenticación real — endpoint vulnerable
- CORE-077 creado: ws/siren.rs es un mock — voz completamente rota
- CORE-078 creado: AEGIS_DEV_MASTER_BYPASS documentar y eliminar

### Kernel
*   **[CORE-075]** Fix: `engine_config.json` path relativo — persistir en `data_dir` `[DONE]`
*   **[CORE-076]** Fix: `set_hw_profile` sin autenticación real `[DONE]`
*   **[CORE-077]** Fix: `ws/siren.rs` mock — conectar al SirenRouter real `[DONE — Kernel Engineer]`
*   **[CORE-078]** Fix: Eliminar `AEGIS_DEV_MASTER_BYPASS` de producción `[DONE]`

## 🏁 UPDATE — 2026-04-13 (EPIC 34 — TERCERA RONDA DE AUDITORÍA)
- CORE-079 creado: SystemTab pasa session_key en query param — telemetría admin rota
- CORE-080 creado: ~15 métodos gRPC con unimplemented!() — CLI no funcional
- CORE-081 creado: CloudProxyDriver no soporta protocolo nativo de Anthropic
- CORE-082 creado: auth_interceptor gRPC deja pasar headers Citadel parciales

### Shell
*   **[CORE-079]** Fix: `SystemTab` pasa `session_key` en query param de `/api/status` `[DONE — 2026-04-13]`

### Kernel
*   **[CORE-080]** Fix: gRPC `server.rs` — implementar métodos `unimplemented!()` Prioridad 1 y 2 `[DONE — Kernel Engineer]`
*   **[CORE-081]** Fix: `CloudProxyDriver` no soporta protocolo nativo de Anthropic `[DONE — Kernel Engineer]`
*   **[CORE-082]** Fix: `auth_interceptor` gRPC deja pasar headers Citadel parciales `[DONE]`

## 🏁 UPDATE — 2026-04-13 (EPIC 34 — CUARTA RONDA DE AUDITORÍA)
- CORE-083 creado: ProvidersTab usa credenciales hardcodeadas y query params
- CORE-084 creado: models.yaml tiene providers sin soporte real en el driver
- CORE-085 creado: Scheduler no conecta al HAL — execution_tx siempre None → CHAT NUNCA FUNCIONA

### Shell
*   **[CORE-083]** Fix: `ProvidersTab` usa credenciales hardcodeadas y query params `[DONE — 2026-04-13]`

### Kernel
*   **[CORE-084]** Fix: `models.yaml` providers sin URL explícita en `entry_api_url` `[DONE — Kernel Engineer]`
*   **[CORE-085]** Fix: Scheduler no conecta al HAL — `execution_tx` siempre `None` — chat nunca responde `[DONE]`

## 🏁 UPDATE — 2026-04-13 (EPIC 34 — QUINTA RONDA DE AUDITORÍA — CIERRE)
- CORE-086 creado: router_api add_global_key valida admin por nombre hardcodeado y llama authenticate_tenant en lugar de authenticate_master
- CORE-087 creado: siren_api expone session_key en query params
- CORE-088 creado: ChatTerminal envía session_key en FormData del file upload
- CORE-089 creado: providers endpoint sin autenticación ni validación SSRF

### Kernel
*   **[CORE-086]** Fix: `router_api.rs` — `add_global_key` usa `authenticate_tenant` y nombre `"root"` hardcodeado `[DONE]`
*   **[CORE-087]** Fix: `siren_api.rs` expone `session_key` en query params `[DONE]`
*   **[CORE-089]** Fix: `providers.rs` — sin autenticación ni validación SSRF en `api_url` `[DONE]`

### Shell
*   **[CORE-088]** Fix: `ChatTerminal` envía `session_key` en FormData del file upload `[DONE — 2026-04-13]`

---

## 📊 EPIC 34 — AUDITORÍA COMPLETA — 2026-04-13

**Total tickets:** 21
**Críticos 🔴:** 10 | **Altos 🟠:** 9 | **Medios 🟡:** 2

#### Fase 0 — CI Integrity (Automatización Centralizada)
0. **[CORE-090]** CI — Reforzar auto-format/clippy fix en GitHub Actions como fuente de verdad `[DONE]`

### Orden de implementación recomendado (por impacto)

#### Fase 1 — El chat debe funcionar (sin esto, nada más importa)
1. **CORE-085** Kernel — Conectar scheduler al HAL (execution_tx)
2. **CORE-075** Kernel — engine_config.json en data_dir
3. **CORE-086** Kernel — router_api authenticate_master

#### Fase 2 — Seguridad de credenciales (viajan en texto plano)
4. **CORE-070** Shell — WebSocket URL hardcodeada
5. **CORE-071** Shell + Kernel — admin endpoints migrar a headers `[DONE]`
6. **CORE-073** Shell — sessionKey fuera de localStorage
7. **CORE-079** Shell — SystemTab query param
8. **CORE-083** Shell — ProvidersTab credenciales hardcodeadas `[DONE]`
9. **CORE-087** Kernel — siren_api query params
10. **CORE-088** Shell + Kernel — file upload FormData `[DONE]`

#### Fase 3 — Seguridad de backend
11. **CORE-076** Kernel — set_hw_profile sin auth
12. **CORE-078** Kernel — eliminar AEGIS_DEV_MASTER_BYPASS
13. **CORE-072** Shell + Kernel — isAdmin por nombre
14. **CORE-082** Kernel — auth_interceptor gRPC headers parciales
15. **CORE-089** Kernel — providers SSRF

#### Fase 4 — Funcionalidad y correctness
16. **CORE-077** Kernel — ws/siren mock → SirenRouter real
17. **CORE-080** Kernel — gRPC unimplemented métodos
18. **CORE-081** Kernel — Anthropic URL
19. **CORE-084** Kernel — models.yaml entry_api_url
20. **CORE-074** Kernel — get_sync_version path relativo
