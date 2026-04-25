# BRIEFING — Kernel Engineer + Shell Engineer
# Epic 44: Developer Workspace
# Fecha: 2026-04-24

---

## INSTRUCCIONES DE SESIÓN

Sos el ingeniero de implementación del proyecto Aegis-Core.
Tu trabajo en esta sesión es implementar la **Epic 44 completa**.

**Antes de escribir una sola línea de código, leé:**
1. `governance/EPIC_44_DEVELOPER_WORKSPACE.md` — arquitectura y ADRs
2. Los tickets individuales `governance/Tickets/CORE-167.md` a `CORE-180.md`
3. El código existente relevante antes de tocar cualquier archivo

---

## CONTEXTO DEL REPO

- Monorepo en `C:\Aegis\Aegis-Core`
- Kernel: `kernel/crates/ank-core/` y `kernel/crates/ank-http/` (Rust/Tokio/Axum)
- Shell: `shell/ui/src/` (React/TypeScript/Tailwind)
- Governance: `governance/Tickets/` y `governance/TICKETS_MASTER.md`

---

## SCOPE COMPLETO

### Tickets Kernel Engineer — en este orden

**Fase 1 — Fundacional (sin dependencias):**
- `CORE-167`: `ank-core/src/workspace/config.rs` + tabla SQLCipher `workspace_config` + endpoints `/api/workspace/config`

**Fase 2 — Paralelas (dependen solo de 167):**
- `CORE-168`: `ank-core/src/executor/terminal.rs` — TerminalExecutor con allowlist y streaming
- `CORE-170`: `ank-http/src/routes/fs.rs` — endpoints `/api/fs/tree` y `/api/fs/file` con anti-path-traversal
- `CORE-171`: `ank-core/src/git/bridge.rs` — GitHubBridge con identidad `Aegis OS <bot@aegis-os.dev>`

**Fase 3 — Syscalls (dependen de 168 y 171):**
- `CORE-169`: syscall `SYS_EXEC` en `ank-core/src/syscalls/` + integración en `run_agent_loop`
- `CORE-172`: syscalls `SYS_GIT_BRANCH`, `SYS_GIT_COMMIT`, `SYS_GIT_PUSH` + integración en `run_agent_loop`

**Fase 4 — PR Manager (depende de 171):**
- `CORE-173`: `ank-core/src/pr_manager/` + tabla `managed_prs` + endpoints `/api/prs/*` + polling loop en tokio::spawn
- `CORE-174`: función `trigger_auto_fix` dentro del PR Manager + campo `auto_fix_attempts`

**Fase 5 — WebSocket (depende de 173):**
- `CORE-175`: nuevas variantes en `WsEvent` + nuevos `case` en `useAegisStore.ts` (backend + frontend juntos)

**Fase 6 — Endpoint adicional para Shell:**
Agregar `GET /api/git/status` que retorna branches + commits recientes + PRs activos (necesario para CORE-178).

### Tickets Shell Engineer — pueden empezar desde Fase 2

Los componentes de Shell usan **mock data** cuando el backend aún no está listo (cada ticket tiene su mock definido). Implementar en este orden:

- `CORE-180`: `WorkspaceSettings.tsx` — configuración (depende de CORE-167)
- `CORE-176`: `TerminalPanel.tsx` + `terminalStore.ts` completo (depende de CORE-175 stubs)
- `CORE-177`: `CodeViewer.tsx` (depende de CORE-170)
- `CORE-179`: `PRManagerPanel.tsx` + `prStore.ts` completo (depende de CORE-173 + CORE-175)
- `CORE-178`: `GitTimeline.tsx` (depende de CORE-173)

Todos los componentes se integran en `Dashboard.tsx` del tenant en este orden de aparición (de arriba a abajo):
1. AgentTreeWidget (CORE-166 — ya existe)
2. TerminalPanel (CORE-176)
3. CodeViewer (CORE-177)
4. GitTimeline (CORE-178)
5. PRManagerPanel (CORE-179)
6. WorkspaceSettings (CORE-180)

---

## REGLAS DE IMPLEMENTACIÓN (NO NEGOCIABLES)

1. **Zero `unwrap()` / `expect()`** — usar `?` con `anyhow::Result`
2. **Zero warnings de Clippy** — el CI los rechaza
3. **`cargo build` debe pasar** al final de cada ticket
4. **No `cargo test` local** — los tests corren en CI
5. **Un commit por ticket**, formato Conventional Commits:
   ```
   feat(ank-core): CORE-167 workspace_config tabla y endpoint
   feat(ank-core): CORE-168 TerminalExecutor streaming
   feat(ank-http): CORE-170 FileSystemBridge endpoints fs
   feat(ank-core): CORE-171 GitHubBridge identidad bot
   feat(ank-core): CORE-169 SYS_EXEC syscall terminal
   feat(ank-core): CORE-172 SYS_GIT_* syscalls para agentes
   feat(ank-core): CORE-173 PR Manager ciclo de vida y polling
   feat(ank-core): CORE-174 auto-fix CI proceso cognitivo
   feat(ank-core): CORE-175 eventos WebSocket Epic 44
   feat(shell): CORE-176 TerminalPanel dashboard tenant
   feat(shell): CORE-177 CodeViewer arbol y contenido
   feat(shell): CORE-178 GitTimeline branches y PRs
   feat(shell): CORE-179 PRManagerPanel controles auto manual
   feat(shell): CORE-180 WorkspaceSettings configuracion workspace
   ```
6. **Rama**: crear `feat/epic-44-developer-workspace` y trabajar ahí
7. **No hacer push a main** — Tavo mergea manualmente

---

## ADVERTENCIAS CRÍTICAS

- **El GitHub token NUNCA se loggea** — ni en `tracing::debug!` ni en mensajes de error. Si la URL de push lo contiene, redactarlo: `https://***@github.com/...`
- **`GET /api/workspace/config` nunca retorna el token real** — solo `"configured"` o `null`
- **Path traversal en FileSystemBridge** — siempre usar `canonicalize()` y verificar que la ruta resuelta empiece con `project_root`
- **TerminalExecutor**: validar el binario contra la allowlist ANTES de spawnear el proceso. Rechazar args con `&&`, `;`, `|`, `$(`, `../`
- **Las migraciones SQLCipher son additive** — `CREATE TABLE IF NOT EXISTS` y `ALTER TABLE ... ADD COLUMN ... DEFAULT`. Nunca `DROP` ni `ALTER` destructivos
- **El polling loop del PR Manager** debe arrancar en `tokio::spawn` al iniciar el servidor, no en cada request
- **`reqwest`** — agregar al `Cargo.toml` de `ank-core` si no está: `reqwest = { version = "0.12", features = ["json"] }`

---

## CÓMO EMPEZAR

```bash
# 1. Crear rama
git checkout -b feat/epic-44-developer-workspace

# 2. Leer el código relevante ANTES de tocar nada
#    - kernel/crates/ank-core/src/lib.rs          (qué módulos existen)
#    - kernel/crates/ank-core/src/enclave/        (cómo se usa CitadelEnclave)
#    - kernel/crates/ank-core/src/syscalls/       (patrón de syscalls existentes)
#    - kernel/crates/ank-core/src/scheduler/      (dónde agregar PrManager)
#    - kernel/crates/ank-http/src/lib.rs           (cómo se registran routes)
#    - kernel/crates/ank-http/src/routes/          (patrón de routes existentes)
#    - shell/ui/src/store/useAegisStore.ts         (switch de eventos WS)
#    - shell/ui/src/components/Dashboard.tsx       (dónde integrar los widgets)
#    - shell/ui/src/components/AgentTreeWidget.tsx (referencia de estilo visual)

# 3. Implementar en el orden de fases indicado arriba

# 4. Verificar compilación tras cada ticket
cargo build 2>&1 | head -80
```

---

*Briefing generado por Arquitecto IA — 2026-04-24*
*Epic 44 — Developer Workspace*
