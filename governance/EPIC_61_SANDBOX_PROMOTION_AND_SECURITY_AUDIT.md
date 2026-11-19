# EPIC 61 — Sandbox Promotion, Security Hardening & SRE Maturation

**Estado:** 🚧 En Progreso

## 🎯 Objetivo

Consolidar y promover al código general todas las capacidades validadas en el entorno Sandbox (`:8090`) —incluyendo **Creator Studio**, el **Hub de Módulos**, las **respuestas enriquecidas con descargas directas** (`[AEGIS_VIDEO_CARD]`) y el **cuadro de texto autoajustable**— garantizando su empaquetado completo en producción, devolviendo el pipeline de CI a verde y ejecutando un plan secuencial de endurecimiento de seguridad, rendimiento y mantenibilidad derivado de la auditoría integral del repositorio.

---

## 🏗️ Bloques de Ejecución Secuencial

### Bloque 1 — Promoción Sandbox → General, CI Verde e Higiene Git
1. **`CORE-394`** (`feat`, ✅ Done): Promoción de Mejoras del Sandbox al Código General (Creator Studio, Hub de Módulos, Respuestas con Descargas y Textarea Autoajustable).
2. **`CORE-395`** (`fix`, ✅ Done): CI Verde — Reparar `cargo clippy`, `cargo fmt` y ESLint en `shell/ui`.
3. **`CORE-396`** (`chore`, ✅ Done): Higiene Git y Gobernanza — Limpieza de Worktrees Huérfanos, Reconciliación de IDs `CORE-375..379` y Rama Limpia hacia `main`.
4. **`CORE-397`** (`feat`, ✅ Done): Paridad Sandbox → Producción en Instaladores, Updates y `ide_hook.py`.

### Bloque 2 — Hardening de Seguridad e Integridad Multi-Tenant
5. **`CORE-398`** (`fix`, ✅ Done): Hardening HTTP/WS — CORS, `is_direct_local_cli_probe`, `quick_personal_setup` y Auth Pre-Upgrade en WebSockets.
6. **`CORE-399`** (`fix`, ✅ Done): Integridad `TenantDB` y Symlink Jail (`safe_resolve` en `ReadFile`, `ScribeManager` y `fs.rs`).
7. **`CORE-400`** (`fix`, 📥 Todo): Aislamiento Multi-Tenant y SSRF (`agents.rs`, `chal/mod.rs`, `antigravity_bridge`, `vcm/mod.rs`, `video_api`, `engine.rs`, `admin.rs`).
8. **`CORE-401`** (`fix`, 📥 Todo): Supply Chain y Secretos (`runtime_deps.rs`, `whatsapp_bridge`, permisos `0600`, `deny.toml`).

### Bloque 3 — Rendimiento, Concurrencia y Frontend
9. **`CORE-402`** (`perf`, 📥 Todo): Tokens de Sesión Citadel, Argon2id en `spawn_blocking` y Pool LRU de `TenantDB` (ADR-034).
10. **`CORE-403`** (`fix`, 📥 Todo): Estabilidad y Límites SRE (`video_api` streaming/`DefaultBodyLimit`/semáforo FFmpeg, logs acotados, graceful shutdown, `env::set_var` y `AntigravityRunner`).
11. **`CORE-404`** (`perf`, 📥 Todo): Optimización Frontend (`shell/ui`: selectores `useAegisStore`, `React.lazy()`, optimización de SVGs y `apiClient.ts`).

### Bloque 4 — Modularización, Testing y OpenAPI
12. **`CORE-405`** (`refactor`, 📥 Todo): Modularización de God Files (`chal/mod.rs`, `useAegisStore.ts`, `ModulesAppHub.tsx`, `VideoStudioPanel.tsx`, scripts Python externos).
13. **`CORE-406`** (`chore`, 📥 Todo): Tests UI (Vitest), CI Extendido (`app/`, bridges, `update.ps1`, `sync_tickets_master.py`) y Actualización/Protección de OpenAPI (`/api/docs`).

---

## ✅ Criterios de Cierre de la Épica
- [ ] Todos los tickets `CORE-394`..`CORE-406` en estado `✅ Done`.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace` (flags CI), `cargo test --workspace`, `npm run lint` y `npm run build` pasando en verde.
- [ ] `python tools/sync_tickets_master.py --report` con 0 discrepancias.
- [ ] Paridad verificada entre entorno sandbox y binario/instaladores de producción.
