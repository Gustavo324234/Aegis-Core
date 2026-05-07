# BRIEFING — Sesión 1: Specialist Filesystem Tools + ProjectLedger + AgentEvent enum

**Fecha:** 2026-05-06  
**Para:** Kernel Engineer (Claude Code)  
**Épica:** EPIC 50 — Agent Inbox  
**Tickets:** CORE-275, CORE-273, CORE-274 (parcial Rust)

---

## Contexto de sesión

Estamos implementando el bloque base del EPIC 50. Esta sesión cubre solo Rust/kernel.
No tocar shell, no tocar UI. Un solo PR al finalizar.

Antes de escribir cualquier código, leer los tickets completos:
- `governance/Tickets/CORE-275.md`
- `governance/Tickets/CORE-273.md`
- `governance/Tickets/CORE-274.md`

---

## Branch

```
feat/core-275-specialist-filesystem
```

Crear desde `main` antes de empezar.

---

## Objetivo

Implementar en este orden exacto:

### 1. CORE-275 — Specialist filesystem tools

**Archivo:** `kernel/crates/ank-core/src/agents/tool_registry.rs`

Agregar tres tools al arm `AgentRole::Specialist`:
- `read_file` — lee archivo relativo al workspace del tenant, con `offset` y `length` opcionales
- `write_file` — escribe dentro del workspace, modos `rewrite` (default) y `append`
- `list_files` — lista directorio con `depth` máximo 4, ignora `target/`, `node_modules/`, archivos ocultos

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

Agregar en `execute_tool_call_internal`:

Helper `resolve_path(workspace, input_path, approved_paths)`:
- Si el path es relativo → resolve dentro del workspace
- Llamar `canonicalize()` para resolver `..` y symlinks
- Si el resultado está dentro del workspace → OK
- Si está fuera y está en `approved_paths` → OK  
- Si está fuera y NO está aprobado → retornar `{"error":"path_requires_approval","path":"..."}`

Helper `get_tenant_workspace(pcb)`:
- Retorna `{AEGIS_DATA_DIR}/users/{tenant_id}/workspace/`

Helper `get_approved_paths(pcb)`:
- Por ahora retorna `vec![]` — CORE-276 lo completará

Arms `read_file`, `write_file`, `list_files` usando los helpers.
Ver código detallado en `governance/Tickets/CORE-275.md`.

**Archivo:** `kernel/config/agents/specialist.md`
Agregar sección "Filesystem tools" al final. Ver texto en CORE-275.

---

### 2. CORE-273 — ProjectLedger

**IMPORTANTE — diseño corregido respecto al ticket:**
El ticket CORE-273 tiene una versión vieja con categorías (`decisions`, `completed`, `pending`).
Implementar el diseño simplificado:

```rust
// kernel/crates/ank-core/src/agents/project_ledger.rs  (archivo nuevo)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLedger {
    pub project_id: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Entradas en texto libre — sin categorías. El LLM sintetiza al leer.
    pub entries: Vec<LedgerEntry>,
    /// Intercambios usuario ↔ supervisores.
    pub user_exchanges: Vec<UserExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub content: String,          // texto libre, cualquier dominio
    pub author: String,           // agent_id como string, o "user"
    pub source_agent_role: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExchange {
    pub id: Uuid,
    pub question: String,
    pub answer: Option<String>,
    pub context: Option<String>,
    pub agent_role: String,
    pub asked_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
}
```

Exportar desde `agents/mod.rs`.

**Archivo:** `kernel/crates/ank-core/src/agents/persistence.rs`

Agregar:
```rust
pub fn ledger_path(&self, tenant_id: &str, project_id: &ProjectId) -> PathBuf {
    self.project_dir(tenant_id, project_id).join("project.json")
}

pub fn save_ledger(&self, tenant_id: &str, project_id: &ProjectId, ledger: &ProjectLedger) -> anyhow::Result<()>

pub fn load_ledger(&self, tenant_id: &str, project_id: &ProjectId) -> anyhow::Result<Option<ProjectLedger>>
```

**Archivo:** `kernel/crates/ank-core/src/agents/tool_registry.rs`

Agregar herramienta `add_ledger_entry` para `ProjectSupervisor` y `Supervisor`:

```rust
fn add_ledger_entry() -> ToolDefinition {
    ToolDefinition {
        name: "add_ledger_entry",
        description: "Record something important in the project's permanent history. Use for design decisions, completed milestones, or relevant findings that the user should be able to consult later.",
        parameters: json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "What to record. Plain language, any domain."
                }
            },
            "required": ["content"]
        }),
    }
}
```

Agregar arm `add_ledger_entry` en `execute_tool_call_internal`:
- Obtener `project_id` del nodo del agente via `orchestrator.tree`
- Cargar el ledger del tenant desde disco (o crear uno nuevo si no existe)
- Agregar la entrada con timestamp, author=agent_id, source_agent_role del nodo
- Guardar inmediatamente en disco
- Retornar `{"status":"recorded"}`

**Archivo:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

En `restore_project`, después de cargar el árbol, cargar el ledger e inyectarlo
al system prompt del ProjectSupervisor:

```
[PROJECT HISTORY]
{entries ordenadas cronológicamente, una por línea}
```

Si no hay ledger, no inyectar nada.

---

### 3. CORE-274 — enum AgentEvent (solo la definición Rust)

**Archivo nuevo:** `kernel/crates/ank-core/src/agents/events.rs`

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    SupervisorQuestion {
        agent_id: Uuid,
        project_name: String,
        question: String,
        context: Option<String>,
        timestamp: DateTime<Utc>,
    },
    SupervisorResumed {
        agent_id: Uuid,
    },
    SupervisorCompleted {
        agent_id: Uuid,
        project_name: String,
        summary: String,
    },
    SupervisorTimedOut {
        agent_id: Uuid,
        project_name: String,
    },
}
```

Exportar desde `agents/mod.rs`. No conectar todavía al WebSocket — eso es Sesión 2.

---

## Verificación

```bash
cargo build --workspace
```

Sin errores. Sin warnings nuevos de clippy que no existían antes.

---

## Commit y PR

**Commit message:**
```
feat(ank-core): CORE-275/273/274 specialist filesystem tools, ProjectLedger, AgentEvent enum
```

**PR title:**
```
feat(ank-core): CORE-275/273/274 — specialist filesystem tools + ProjectLedger + AgentEvent
```

**PR description:**
```
## Cambios

### CORE-275 — Specialist filesystem tools
- `read_file`, `write_file`, `list_files` en ToolRegistry para Specialist
- Arms correspondientes en `execute_tool_call_internal`
- Helper `resolve_path` con confinamiento al workspace via `canonicalize()`
- `get_tenant_workspace`, `get_approved_paths` (stub — CORE-276 lo completa)
- Actualización de `specialist.md`

### CORE-273 — ProjectLedger
- Nuevo `agents/project_ledger.rs` con structs `ProjectLedger`, `LedgerEntry`, `UserExchange`
- `save_ledger` / `load_ledger` en `AgentPersistence`
- Herramienta `add_ledger_entry` en ToolRegistry para supervisores
- Arm `add_ledger_entry` en `execute_tool_call_internal`
- Inyección del ledger en system prompt al restaurar proyecto

### CORE-274 — AgentEvent enum
- Nuevo `agents/events.rs` con `AgentEvent` (SupervisorQuestion, SupervisorResumed, SupervisorCompleted, SupervisorTimedOut)
- Exportado desde `agents/mod.rs`

## Verificación
`cargo build --workspace` ✅

## Dependencias
- No rompe nada existente
- CORE-276 completa `get_approved_paths`
- CORE-268 conecta `AgentEvent` al WebSocket
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-06*
