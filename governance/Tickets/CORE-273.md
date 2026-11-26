# CORE-273 — ProjectLedger: registro de avance persistente por proyecto

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/project_ledger.rs` (nuevo)
- `kernel/crates/ank-core/src/agents/persistence.rs` (nuevo método `project.json`)
- `kernel/crates/ank-core/src/agents/orchestrator.rs` (escribir al ledger en eventos clave)
- `kernel/crates/ank-core/src/agents/tool_registry.rs` (herramienta `update_project_ledger`)
**Estado:** ✅ Completo

---

## Contexto

Hoy el proyecto es solo un `ProjectId: String`. El conocimiento acumulado vive
en `agent_contexts/{agent_id}.md` — archivos atados al UUID de un supervisor que
desaparece con cada sesión. Cuando el supervisor muere, su contexto se pierde o queda
como basura de runtime que solo sirve para restaurar el árbol.

Lo que el usuario necesita es diferente: saber **qué se decidió hacer, qué se hizo,
y qué falta** — independientemente de qué supervisores existieron en el camino.

El `ProjectLedger` es el `project.json` mencionado en `AgentPersistence` pero nunca
implementado. Es la entidad persistente del proyecto — el supervisor es efímero,
el ledger no.

---

## Estructura del `ProjectLedger`

```rust
/// Registro de avance persistente de un proyecto.
/// Sobrevive a la muerte de cualquier supervisor.
/// Ruta en disco: users/{tenant}/projects/{project_id}/project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLedger {
    pub project_id: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Qué se acordó hacer — decisiones de diseño, scope acordado con el usuario.
    pub decisions: Vec<LedgerEntry>,

    /// Qué está hecho — tareas completadas confirmadas.
    pub completed: Vec<LedgerEntry>,

    /// Qué falta — tareas pendientes o bloqueadas.
    pub pending: Vec<LedgerEntry>,

    /// Intercambios usuario ↔ supervisores (CORE-272 corregido).
    /// No es historial de chat — es registro de decisiones tomadas via diálogo.
    pub user_exchanges: Vec<UserExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub content: String,
    pub author: String,     // agent_id o "user"
    pub timestamp: DateTime<Utc>,
    pub source_agent_role: String,  // "ProjectSupervisor", "Supervisor — Kernel", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExchange {
    pub id: Uuid,
    pub question: String,           // lo que preguntó el supervisor
    pub answer: Option<String>,     // lo que respondió el usuario (None si aún pendiente)
    pub context: Option<String>,    // por qué preguntaba el supervisor
    pub agent_role: String,
    pub asked_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
}
```

---

## Comportamiento

### Cuándo el kernel escribe al ledger

| Evento | Acción en ledger |
|--------|-----------------|
| Supervisor invoca `report(status=completed, summary=...)` | Agregar entrada a `completed` |
| Supervisor invoca `ask_user(question, context)` | Agregar `UserExchange` con `answer: None` |
| Usuario responde (CORE-271) | Completar el `UserExchange` con answer + answered_at |
| Supervisor invoca herramienta nueva `update_project_ledger` | Agregar a `decisions` o `pending` según tipo |

### Herramienta `update_project_ledger` para supervisores

Los supervisores pueden escribir directamente al ledger cuando toman una decisión
de diseño o identifican trabajo pendiente. No es automático — el supervisor la invoca
cuando tiene algo concreto que registrar.

```json
{
  "name": "update_project_ledger",
  "description": "Registrar una decisión de diseño, tarea completada, o trabajo pendiente en el ledger permanente del proyecto. Usar cuando tomás una decisión importante, completás algo concreto, o identificás trabajo pendiente.",
  "parameters": {
    "type": { "enum": ["decision", "completed", "pending"] },
    "content": "Descripción clara de la decisión, logro, o tarea pendiente"
  }
}
```

### Cuándo NO se escribe al ledger

- Mensajes internos de coordinación entre agentes
- State summaries de runtime (esos siguen en `agent_contexts/`)
- Logs de debugging o trazas de ejecución
- Preguntas que el usuario nunca respondió (se marcan `answer: null`, no se eliminan)

---

## Persistencia

En `AgentPersistence`, agregar:

```rust
pub fn ledger_path(&self, tenant_id: &str, project_id: &ProjectId) -> PathBuf {
    self.project_dir(tenant_id, project_id).join("project.json")
}

pub fn save_ledger(&self, tenant_id: &str, project_id: &ProjectId, ledger: &ProjectLedger)
    -> anyhow::Result<()>;

pub fn load_ledger(&self, tenant_id: &str, project_id: &ProjectId)
    -> anyhow::Result<Option<ProjectLedger>>;
```

El ledger se guarda:
- Inmediatamente al agregar cualquier entrada (no solo al cerrar sesión)
- En `close_project` como parte del cierre normal

Al restaurar un proyecto (`restore_project`), el ledger se carga y se inyecta
como contexto al sistema prompt del ProjectSupervisor:

```
[HISTORIAL DEL PROYECTO]
Decisiones tomadas:
- Usar Tailwind para el frontend (2026-05-01)
- Migrar a monorepo antes del launch (2026-04-28)

Completado:
- Setup inicial del repositorio
- Migración de Aegis-ANK a Aegis-Core

Pendiente:
- Implementar AgentBadge en la shell (CORE-269)
- Escribir tests de integración para auth
```

---

## Endpoint GET para la shell

```
GET /api/projects/:project_id/ledger
Headers: x-citadel-tenant, x-citadel-key
```

Retorna el `ProjectLedger` serializado. La shell (CORE-270 extendido) puede mostrar
este ledger en la vista del proyecto o en el hilo de supervisor.

---

## Corrección a CORE-272

CORE-272 diseñó `ThreadHistory` en el `AgentOrchestrator` (en memoria, por agent_id).
Eso es incorrecto — el historial de intercambios usuario↔supervisor pertenece al
`ProjectLedger` (en disco, por project_id).

CORE-272 se modifica: en lugar de `get_agent_thread(agent_id)`, la herramienta
pasa a ser `get_project_ledger(project_name)` y lee del `ProjectLedger`.

---

## Criterios de aceptación

- [x] `ProjectLedger` existe en `agents/project_ledger.rs` con los structs descritos
- [x] `AgentPersistence` tiene `save_ledger` y `load_ledger` escribiendo a `project.json`
- [x] Cuando un supervisor invoca `update_project_ledger`, la entrada aparece en `project.json`
- [x] Cuando un supervisor invoca `ask_user`, se crea un `UserExchange` con `answer: None`
- [x] Cuando el usuario responde (CORE-271), el `UserExchange` se actualiza con la respuesta
- [x] Al restaurar un proyecto, el ledger se inyecta como contexto al ProjectSupervisor
- [x] `GET /api/projects/:project_id/ledger` retorna el ledger serializado
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-263 ✅ (ask_user implementado)
- CORE-268 (AgentEvents — integrar con ledger en el mismo arm)
- CORE-271 (respuesta directa — actualizar UserExchange al recibir reply)
