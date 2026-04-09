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
