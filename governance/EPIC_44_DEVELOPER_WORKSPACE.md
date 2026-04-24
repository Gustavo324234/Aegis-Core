# EPIC 44 — Developer Workspace

**Estado:** PLANNED  
**Fecha de diseño:** 2026-04-24  
**Arquitecto:** Arquitecto IA  
**Repo:** Aegis-Core

---

## 1. Visión

El tenant puede trabajar con su código directamente desde Aegis: ver archivos, ver el historial de Git, gestionar Pull Requests y recibir notificaciones de merge en el chat — todo sin salir de la interfaz. Los agentes de Epic 43 ganan acceso a la terminal del servidor y al repositorio Git del proyecto.

**Cinco capacidades en una épica:**

| # | Capacidad | Descripción |
|---|---|---|
| 1 | **Terminal de Agentes** | Los agentes ejecutan comandos de sistema; el output llega al tenant en tiempo real via WebSocket |
| 2 | **Code Viewer** | El Dashboard muestra el árbol de archivos y el contenido de cualquier archivo del proyecto |
| 3 | **GitHub Identity** | Aegis aparece como colaborador (@aegis-os-bot), crea branches y commits con su identidad |
| 4 | **PR Manager** | Crea PRs, modo auto/manual, auto-fix de errores CI, control del merge |
| 5 | **Git Timeline** | Visualización de branches, commits y estado de CI en el Dashboard |

---

## 2. Arquitectura

### 2.1 Terminal de Agentes (TerminalExecutor)

Los agentes necesitan ejecutar comandos para verificar su trabajo (`cargo build`, `npm run build`, `git status`, etc.).

**Componente nuevo:** `ank-core/src/executor/terminal.rs`

```rust
pub struct TerminalExecutor {
    /// Working directory base del proyecto (configurable por tenant)
    project_root: PathBuf,
    /// Lista blanca de comandos permitidos (seguridad)
    allowed_commands: Vec<String>,
    /// Timeout por comando en segundos
    timeout_secs: u64,
}

impl TerminalExecutor {
    /// Ejecuta un comando y retorna output + exit_code.
    /// Streama las líneas de stdout/stderr via un canal Tokio
    /// para que el WebSocket las envíe en tiempo real.
    pub async fn exec(
        &self,
        command: &str,
        args: &[&str],
        tx: mpsc::Sender<TerminalLine>,
    ) -> anyhow::Result<i32> { ... }
}

pub struct TerminalLine {
    pub kind: LineKind,   // Stdout | Stderr | ExitCode
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

**Evento WebSocket nuevo:** `terminal_output`
```json
{
  "event": "terminal_output",
  "data": {
    "agent_id": "spec-scheduler",
    "command": "cargo build",
    "line": "   Compiling ank-core v0.5.0",
    "kind": "stdout",
    "timestamp": "2026-04-24T12:00:00Z"
  }
}
```

**Seguridad:** El `TerminalExecutor` tiene una allowlist de comandos configurada por el admin. Por defecto permite: `cargo`, `npm`, `git`, `python`, `pytest`, `ls`, `cat`, `find`. Nunca permite: `rm -rf`, `sudo`, `chmod`, `curl | sh`, etc.

**Syscall nueva:** `SYS_EXEC` — los agentes invocan el terminal a través del sistema de syscalls de ank-core. Misma estructura que `SYS_AGENT_SPAWN`.

### 2.2 Code Viewer (FileSystemBridge)

Para que el Dashboard muestre el código del proyecto, el backend necesita exponer el sistema de archivos del proyecto de forma controlada.

**Componente nuevo:** `ank-http/src/routes/fs.rs`

```
GET  /api/fs/tree?path=.         → árbol de archivos del proyecto (hasta 3 niveles)
GET  /api/fs/file?path=src/main.rs → contenido de un archivo
```

**Seguridad:**
- `path` siempre se resuelve relativo al `project_root` configurado — nunca rutas absolutas
- Path traversal bloqueado: `../` y rutas absolutas retornan 400
- Solo extensiones permitidas: `.rs`, `.ts`, `.tsx`, `.toml`, `.json`, `.md`, `.yaml`, `.env.example`
- Archivos con secretos explícitamente bloqueados: `.env`, `*.key`, `*.pem`, `aegis.env`

### 2.3 GitHub Identity (GitHubBridge)

Para que Aegis aparezca como colaborador, se necesita:
- Una **GitHub App** o **bot account** (`@aegis-os-bot`) con token PAT
- El token se guarda en el enclave Citadel del tenant (cifrado en SQLCipher)
- Cada operación Git se hace con `git -c user.name="Aegis OS" -c user.email="bot@aegis-os.dev"`

**Componente nuevo:** `ank-core/src/git/bridge.rs`

```rust
pub struct GitHubBridge {
    project_root: PathBuf,
    github_token: String,     // del enclave Citadel
    bot_name: String,         // "Aegis OS"
    bot_email: String,        // "bot@aegis-os.dev"
    repo_owner: String,       // "Gustavo324234"
    repo_name: String,        // "Aegis-Core"
}

impl GitHubBridge {
    /// Crea una branch nueva desde la branch base indicada
    pub async fn create_branch(&self, branch_name: &str, from: &str) -> anyhow::Result<()> { ... }

    /// Stage + commit con mensaje en formato Conventional Commits
    pub async fn commit(&self, files: &[&str], message: &str) -> anyhow::Result<String> { ... }
    // retorna el commit SHA

    /// Push de la branch al remoto
    pub async fn push(&self, branch_name: &str) -> anyhow::Result<()> { ... }

    /// Crea un Pull Request via GitHub API REST v3
    pub async fn create_pr(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<PullRequest> { ... }

    /// Retorna el estado de CI checks de un PR (via GitHub API)
    pub async fn get_pr_checks(&self, pr_number: u64) -> anyhow::Result<Vec<CiCheck>> { ... }

    /// Retorna si un PR fue mergeado
    pub async fn is_pr_merged(&self, pr_number: u64) -> anyhow::Result<bool> { ... }

    /// Lista branches y últimos commits (para Git Timeline)
    pub async fn list_branches(&self) -> anyhow::Result<Vec<BranchInfo>> { ... }

    /// Lista commits recientes de una branch
    pub async fn list_commits(&self, branch: &str, limit: usize) -> anyhow::Result<Vec<CommitInfo>> { ... }
}
```

**Configuración del token:** Se guarda en el enclave Citadel con clave `github_token`. El tenant lo configura desde el Dashboard. No se expone nunca en ningún endpoint ni log.

### 2.4 PR Manager

**Estado de un PR en Aegis:**

```rust
pub struct ManagedPr {
    pub pr_number: u64,
    pub title: String,
    pub branch: String,
    pub base: String,
    pub url: String,
    pub merge_mode: MergeMode,
    pub auto_fix_ci: bool,
    pub status: PrStatus,
    pub ci_checks: Vec<CiCheck>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub enum MergeMode {
    /// Aegis hace merge automáticamente cuando CI pasa
    Automatic,
    /// Aegis notifica al tenant y espera aprobación manual
    Manual,
}

pub enum PrStatus {
    Open,
    CiRunning,
    CiPassed,
    CiFailed,
    MergeReady,     // CI pasó, esperando aprobación del tenant (modo Manual)
    Merged,
    Closed,
}
```

**Polling de CI:** `ank-core` tiene un job Tokio que cada 30 segundos consulta el estado de CI de todos los PRs `Open` o `CiRunning`. Cuando detecta un cambio relevante, envía un evento WebSocket al tenant.

**Auto-fix de CI:** Si `auto_fix_ci = true` y CI falla, el sistema:
1. Lee los logs de CI del step fallido (via GitHub API)
2. Crea un nuevo proceso cognitivo con el error como contexto
3. El agente genera un fix, hace un nuevo commit a la misma branch
4. El nuevo commit dispara un nuevo CI run

**Eventos WebSocket nuevos:**
```json
{ "event": "pr_update", "data": { "pr_number": 42, "status": "CiPassed", "url": "..." } }
{ "event": "pr_merged", "data": { "pr_number": 42, "title": "feat: CORE-155...", "merged_by": "Aegis OS" } }
{ "event": "ci_fix_attempt", "data": { "pr_number": 42, "attempt": 1, "error_summary": "..." } }
```

### 2.5 Git Timeline (Vista en Dashboard)

Widget visual en `Dashboard.tsx` que muestra:
- Branches activas con su último commit
- Estado de CI por branch (verde/rojo/amarillo)
- PRs abiertos con su modo (auto/manual) y estado
- Click en un commit → abre el diff en el Code Viewer

---

## 3. Configuración del Tenant

El tenant necesita configurar su workspace antes de usar estas capacidades. Nueva sección en el Dashboard: **Workspace Settings**.

```
GitHub Token: [••••••••••••••••]  [Guardar]
Project Root: [/home/tavo/projects/Aegis-Core]  [Guardar]
Repo:         [Gustavo324234/Aegis-Core]  [Guardar]
Bot Identity: Aegis OS <bot@aegis-os.dev>  [solo lectura]

Terminal Allowlist:  [cargo] [npm] [git] [python] [+Agregar]

PR Defaults:
  Merge Mode:  ( ) Automático  (●) Manual
  Auto-fix CI: [✓] Activado
```

La configuración se persiste en el enclave SQLCipher del tenant en la tabla `workspace_config`.

---

## 4. ADRs de la Épica

| # | Decisión | Razón |
|---|---|---|
| ADR-WS-001 | TerminalExecutor usa allowlist de comandos, no sandboxing de SO | El sandboxing (namespaces, seccomp) agrega complejidad; la allowlist es suficiente para el caso de uso actual |
| ADR-WS-002 | El GitHub token se guarda en el enclave Citadel del tenant, nunca en el filesystem | Consistente con el modelo de seguridad existente de Aegis |
| ADR-WS-003 | GitHubBridge usa la API REST de GitHub (no la GraphQL) | La REST API tiene cobertura total para este caso de uso; GraphQL es más compleja sin beneficio adicional |
| ADR-WS-004 | El polling de CI es pull (30 seg), no webhooks | Los webhooks requieren un endpoint público expuesto; en despliegues detrás de NAT (como el de Tavo) no es viable |
| ADR-WS-005 | Auto-fix de CI crea un proceso cognitivo normal, no un agente nuevo del árbol Epic 43 | El fix de CI es una tarea puntual de una sola inferencia; no justifica toda la infraestructura de AgentNode |
| ADR-WS-006 | Code Viewer es solo lectura en el backend; la escritura va por los agentes + Git | El tenant no edita código directamente desde el Dashboard; ese es el trabajo de los agentes |
| ADR-WS-007 | Path traversal se bloquea en la capa HTTP, no en el OS | Defensa en profundidad: validar en la capa de aplicación antes de tocar el filesystem |

---

## 5. Tickets de la Épica

| ID | Título | Tipo | Asignado a |
|---|---|---|---|
| CORE-167 | workspace_config — tabla SQLCipher + endpoint de configuración | feat | Kernel Engineer |
| CORE-168 | TerminalExecutor — ejecución de comandos con streaming | feat | Kernel Engineer |
| CORE-169 | SYS_EXEC — syscall de terminal para agentes | feat | Kernel Engineer |
| CORE-170 | FileSystemBridge — endpoints /api/fs/tree y /api/fs/file | feat | Kernel Engineer |
| CORE-171 | GitHubBridge — operaciones Git e identidad del bot | feat | Kernel Engineer |
| CORE-172 | SYS_GIT_* — syscalls Git para agentes (branch, commit, push) | feat | Kernel Engineer |
| CORE-173 | PR Manager — ciclo de vida de PRs con polling de CI | feat | Kernel Engineer |
| CORE-174 | Auto-fix CI — proceso cognitivo disparado por fallo de CI | feat | Kernel Engineer |
| CORE-175 | Eventos WebSocket — terminal_output, pr_update, pr_merged | feat | Kernel Engineer |
| CORE-176 | TerminalPanel — UI de terminal en Dashboard del tenant | feat | Shell Engineer |
| CORE-177 | CodeViewer — árbol de archivos + contenido en Dashboard | feat | Shell Engineer |
| CORE-178 | GitTimeline — branches, commits y PRs en Dashboard | feat | Shell Engineer |
| CORE-179 | PRManager UI — lista de PRs con controles auto/manual | feat | Shell Engineer |
| CORE-180 | WorkspaceSettings — configuración de token, repo y opciones | feat | Shell Engineer |

---

*Documento creado por Arquitecto IA — 2026-04-24*
*Épica siguiente a EPIC 43 (Hierarchical Multi-Agent Orchestration)*
*Pre-requisito recomendado: CORE-158 (AgentOrchestrator) para CORE-169 y CORE-172*
