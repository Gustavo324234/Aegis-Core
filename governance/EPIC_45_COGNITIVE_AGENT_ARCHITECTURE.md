# EPIC 45 — Cognitive Agent Architecture (CAA)

**Estado:** PLANNED  
**Fecha de diseño:** 2026-04-26  
**Última actualización:** 2026-04-26 (persistencia + CMR por agente)  
**Arquitecto:** Arquitecto IA  
**Reemplaza/Extiende:** EPIC 43 (Hierarchical Multi-Agent Orchestration)  
**Repos afectados:** Aegis-Core (kernel + shell/ui)

---

## 1. Visión

Transformar Aegis OS en un sistema de agentes cognitivos **n-ary dinámico** donde el usuario interactúa con un único Chat Agent conversacional liviano, y el trabajo real es delegado a una jerarquía de supervisores y specialists que se crean, coordinan y persisten dinámicamente según la complejidad de cada tarea y proyecto.

La jerarquía no tiene profundidad fija. Un nodo supervisor decide en tiempo de ejecución si su tarea requiere sub-supervisores o specialists directos. Los proyectos persisten entre sesiones — el árbol se restaura y cada supervisor recupera su estado resumido sin rediseñar nada. El modelo asignado a cada agente es seleccionado por el CMR según la naturaleza cognitiva del trabajo.

**Principio central:** El usuario habla con un asistente. El asistente coordina un ejército que aprende y recuerda.

---

## 2. Jerarquía de Agentes

### 2.1 Estructura n-ary dinámica

```
Nivel 0 — Chat Agent (siempre exactamente 1 por tenant)
│   Conversa, interpreta intención, gestiona calendario y recordatorios.
│   Contexto mínimo. Despacha trabajo. Responde al usuario.
│   Puede hacer Queries hacia abajo para responder preguntas técnicas.
│   Modelo: liviano, baja latencia, conversacional.
│
└── Nivel 1 — Project Supervisor (1 por proyecto activo, N proyectos en paralelo)
        Creado cuando el Chat Agent detecta intención de trabajo.
        Coordina dominios. Consolida reportes. Persiste entre sesiones.
        Puede comunicarse lateralmente con otros Project Supervisors.
        Modelo: razonamiento, planning.
        │
        └── Nivel 2..N — Supervisor intermedio (dinámico, ilimitado en profundidad)
                Creado cuando la tarea es demasiado compleja para delegar directamente.
                Persiste mientras el proyecto esté activo.
                Puede comunicarse lateralmente con supervisores del mismo padre.
                Modelo: según dominio (código, análisis, creativo, etc.)
                │
                └── Nivel hoja — Specialist Agent (efímero, no persiste)
                        Tarea atómica. Lee archivos, escribe código, analiza, genera.
                        Contexto mínimo: solo lo necesario para su tarea específica.
                        Reporta al supervisor padre. No tiene hijos. No persiste.
                        Modelo: el más adecuado para la tarea concreta.
```

### 2.2 Ejemplo — kernel complejo

```
Chat Agent  [modelo: liviano]
└── Project Supervisor "Aegis OS"  [modelo: planning]
        ├── Supervisor "Kernel"  [modelo: código]
        │     ├── Supervisor "Scheduler"  [modelo: código]
        │     │     ├── Specialist → scheduler/mod.rs  [modelo: código top]
        │     │     └── Specialist → scheduler/persistence.rs
        │     ├── Supervisor "Auth"  [modelo: código + seguridad]
        │     │     └── Specialist → enclave/mod.rs
        │     └── Supervisor "MCP"  [modelo: código]
        │           └── Specialist → ank-mcp/src/registry.rs
        └── Supervisor "Shell"  [modelo: código TypeScript]
              ├── Specialist → ChatTerminal.tsx
              └── Specialist → useAegisStore.ts
```

### 2.3 Ejemplo — tarea simple (sin intermediarios)

```
Chat Agent
└── Project Supervisor "Lista de compras"
        └── Specialist → generar lista semanal
```

### 2.4 Ejemplo — query técnica del usuario

```
Usuario: "¿qué hace authenticate_tenant?"

Chat Agent
→ Query → Project Supervisor "Aegis OS"
→ Query → Supervisor "Auth"
→ Query → Specialist (lee enclave/mod.rs)
← QueryReply (explicación técnica)
← QueryReply (resumen para el supervisor)
← QueryReply (resumen ejecutivo para el usuario)

Chat Agent responde al usuario.
Sin Dispatch. Sin trabajo. Solo lectura.
```

---

## 3. Protocolo de Mensajes Inter-Agente

### 3.1 Tipos de mensaje

```rust
enum AgentMessage {
    /// Hacia abajo — supervisor asigna trabajo a un subordinado.
    /// Puede resultar en spawn de nuevos agentes.
    Dispatch {
        task: String,
        context: AgentContext,
        reply_to: AgentId,
    },

    /// Hacia arriba — subordinado reporta resultado de trabajo completado.
    Report {
        result: AgentResult,
        status: ReportStatus,
        from: AgentId,
    },

    /// Hacia abajo — consulta de información sin crear trabajo nuevo.
    /// El Chat Agent la usa para responder preguntas técnicas del usuario.
    /// No genera cambios, solo lectura.
    Query {
        question: String,
        context_hint: Option<String>,
        reply_to: AgentId,
        query_id: QueryId,
    },

    /// Hacia arriba — respuesta a una Query.
    /// Cada nivel condensa antes de reenviar hacia arriba.
    QueryReply {
        answer: String,
        query_id: QueryId,
        from: AgentId,
    },
}
```

### 3.2 Reglas de comunicación

- **Lateral permitida** solo entre nodos con el mismo `parent_id`
- **Nunca salta niveles** — ni Dispatch ni Report ni Query van de nivel 0 a nivel 3 directamente
- **Query no crea trabajo** — un nodo que recibe una Query no puede hacer Dispatch como consecuencia
- **Condensación por nivel** — cada supervisor resume un QueryReply antes de reenviarlo. El Chat Agent recibe siempre un resumen ejecutivo

---

## 4. Gestión de Contexto por Nivel

### 4.1 Chat Agent (Nivel 0)

**Recibe:**
- Ventana deslizante del historial de conversación (últimos N tokens, configurable)
- Metadata de proyectos activos: `{nombre, estado, último_reporte_resumido}`
- Calendario y recordatorios del tenant
- QueryReplies resumidas de los Project Supervisors

**No recibe:** código, archivos, detalles técnicos, historial de trabajo de agentes.

**Presupuesto:** bajo — el chat es potencialmente infinito.

### 4.2 Project Supervisor (Nivel 1)

**Recibe:**
- Descripción del proyecto
- Estado actual de sus supervisores hijos
- Su `state_summary` (contexto persistido de sesiones anteriores)
- La instrucción del Chat Agent

**No recibe:** historial de conversación del usuario, trabajo de otros proyectos.

### 4.3 Supervisor intermedio (Nivel 2..N)

**Recibe:**
- Su scope declarado
- Resultados de sus agentes hijos
- Su `state_summary` (contexto persistido)
- La instrucción de su padre

**No recibe:** trabajo de otros dominios del mismo nivel.

### 4.4 Specialist Agent (Nivel hoja)

**Recibe:** solo los archivos necesarios (VCM filtra) + la instrucción exacta del padre.

**Presupuesto:** medio-alto. No persiste contexto — es efímero.

### 4.5 Instrucciones por nivel

```
kernel/config/agents/
├── chat_agent.md          ← instrucciones del Chat Agent
├── project_supervisor.md  ← instrucciones del Project Supervisor
├── supervisor.md          ← instrucciones genéricas para supervisores
└── specialist.md          ← instrucciones genéricas para specialists
```

Editables sin recompilar. La Persona del tenant se inyecta solo en el Chat Agent.

---

## 5. CMR por Agente — Modelo según Complejidad y Tipo de Trabajo

Cada `AgentNode` tiene su propio `task_type` y `model_preference`. El CMR existente
selecciona el modelo óptimo para cada agente de forma independiente.

### 5.1 Mapeo de roles a TaskType

```rust
impl AgentRole {
    pub fn default_task_type(&self) -> TaskType {
        match self {
            AgentRole::ChatAgent => TaskType::Chat,
            AgentRole::ProjectSupervisor { .. } => TaskType::Planning,
            AgentRole::Supervisor { .. } => TaskType::Analysis,  // el padre puede override
            AgentRole::Specialist { .. } => TaskType::Code,      // el padre puede override
        }
    }

    pub fn default_model_preference(&self) -> ModelPreference {
        match self {
            // El Chat Agent necesita baja latencia — siempre cloud, modelo liviano
            AgentRole::ChatAgent => ModelPreference::CloudOnly,
            // Supervisores pueden usar modelos más capaces
            AgentRole::ProjectSupervisor { .. } => ModelPreference::HybridSmart,
            AgentRole::Supervisor { .. } => ModelPreference::HybridSmart,
            // Specialists usan el mejor modelo disponible para su tarea
            AgentRole::Specialist { .. } => ModelPreference::HybridSmart,
        }
    }
}
```

### 5.2 Override por el padre

Cuando un supervisor spawea un hijo, puede especificar el `task_type` explícitamente:

```
[SYS_AGENT_SPAWN(role="specialist", scope="refactorizar scheduler.rs", task_type="code")]
[SYS_AGENT_SPAWN(role="supervisor", name="UX", scope="diseño visual", task_type="creative")]
[SYS_AGENT_SPAWN(role="specialist", scope="analizar logs de error", task_type="analysis")]
```

El CMR usa el `task_type` del nodo para puntuar modelos del catálogo y seleccionar el óptimo disponible según el pool de keys del tenant.

### 5.3 Visibilidad del modelo en UI

El modelo asignado a cada nodo es visible en el `AgentTreeView` del Dashboard:
```
▼ Kernel  •  Supervisor  [claude-opus-4 / code]
  ▶ scheduler/mod.rs  •  Specialist  [claude-sonnet-4 / code]
```

---

## 6. Persistencia de Proyectos entre Sesiones

### 6.1 Principio

Los proyectos son entidades de larga vida. El árbol de agentes de un proyecto **se serializa al cerrar la sesión** y **se reconstituye al reactivar el proyecto**. Cada supervisor genera un resumen de estado antes de cerrarse. El resumen es su "memoria" para la próxima sesión.

Los Specialists **no persisten** — son efímeros. Solo persisten supervisores (nivel 1..N).

### 6.2 Estructura de almacenamiento

```
/var/lib/aegis/users/{tenant_id}/projects/{project_id}/
├── project.json          ← metadata del proyecto (nombre, descripción, creado_at)
├── agent_tree.json       ← estructura del árbol serializada (nodos, roles, scopes, jerarquía)
│                            NO incluye contexto de conversación — solo la estructura
└── agent_contexts/
    ├── {agent_id}.md     ← resumen de estado generado por cada supervisor al cerrar
    └── ...               ← un archivo por supervisor activo
```

### 6.3 Ciclo de vida de un proyecto

**Primera activación:**
```
Usuario: "trabajemos en Aegis OS"
→ Chat Agent no encuentra proyecto "Aegis OS" en ProjectRegistry
→ Crea nuevo Project Supervisor
→ Project Supervisor diseña el árbol según la descripción del proyecto
→ Crea supervisores intermedios según complejidad
→ Árbol guardado en agent_tree.json
```

**Cierre de sesión:**
```
Sesión termina (logout o timeout)
→ AgentOrchestrator notifica a cada supervisor activo: "generá tu state summary"
→ Cada supervisor produce un resumen en lenguaje natural de:
   - Qué se completó
   - Qué está en progreso
   - Qué decisiones se tomaron
   - Qué falta hacer
→ Resumen guardado en agent_contexts/{agent_id}.md
→ Árbol serializado en agent_tree.json
→ Supervisores destruidos de memoria
```

**Reactivación:**
```
Usuario: "seguimos con Aegis OS"
→ Chat Agent encuentra proyecto "Aegis OS" en ProjectRegistry
→ AgentOrchestrator carga agent_tree.json → reconstituye la estructura exacta
   (mismos nodos, mismos nombres, mismos scopes, misma jerarquía)
→ Cada supervisor es recreado con su agent_contexts/{agent_id}.md como contexto inicial
→ Los supervisores "saben dónde quedaron" sin rediseñar el árbol
→ Árbol activo en memoria, listo para recibir nuevas tareas
```

### 6.4 Formato del state summary

Cada supervisor genera su resumen con este template (inyectado en specialist.md):

```markdown
## Estado al {fecha}

### Completado
{lista de lo que se terminó}

### En progreso
{lo que estaba en curso al cerrar}

### Decisiones tomadas
{decisiones arquitectónicas o de diseño relevantes}

### Pendiente
{lo que falta hacer en este dominio}

### Contexto importante
{información que el supervisor necesita recordar para continuar}
```

### 6.5 ProjectRegistry persistido

El `ProjectRegistry` mantiene la lista de proyectos del tenant en SQLite (tabla `projects` en el `TenantDB` existente):

```sql
CREATE TABLE IF NOT EXISTS projects (
    project_id   TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    status       TEXT NOT NULL DEFAULT 'active',  -- active | archived
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
```

El árbol y los contextos viven en el filesystem (no en SQLite) por eficiencia — los archivos `.md` y `.json` son más fáciles de debuggear y no bloquean la DB.

### 6.6 AgentNode extendido con persistencia

```rust
struct AgentNode {
    id: AgentId,
    role: AgentRole,
    state: AgentState,
    parent_id: Option<AgentId>,
    children: Vec<AgentId>,
    pcb_id: Option<String>,
    task_type: TaskType,              // para el CMR
    model_preference: ModelPreference,
    context_budget: usize,
    last_report: Option<String>,

    // Persistencia
    project_id: ProjectId,            // proyecto al que pertenece
    persisted_context_path: Option<PathBuf>,  // path al .md de state summary
    is_restored: bool,                // true si fue cargado desde disk
}
```

---

## 7. Arquitectura de Implementación

### 7.1 Nuevo módulo `ank-core/src/agents/`

```
ank-core/src/agents/
├── mod.rs              # re-exports públicos
├── node.rs             # AgentNode, AgentRole, AgentState, AgentId
├── tree.rs             # AgentTree: insert, spawn, prune, query_path, serialize, restore
├── message.rs          # AgentMessage, AgentContext, AgentResult, QueryId
├── orchestrator.rs     # AgentOrchestrator: ciclo de vida, routing, persist, restore
├── project.rs          # ProjectRegistry: CRUD de proyectos, serialize/restore árbol
├── context.rs          # ContextBudget: presupuesto de tokens por nivel
├── persistence.rs      # AgentPersistence: lectura/escritura de agent_tree.json y .md
└── instructions.rs     # InstructionLoader: carga agent/*.md en runtime
```

### 7.2 Integración con el Scheduler existente

El Scheduler no se reemplaza. Cada `AgentNode` que necesita inferencia crea un PCB normal con `agent_id: Option<AgentId>`. Cuando el Scheduler completa un proceso, el AgentOrchestrator intercepta el evento y lo convierte en un `Report`.

### 7.3 Integración con el VCM existente

El VCM se extiende para respetar el `context_budget` por `AgentNode`.

### 7.4 Nuevo WebSocket de telemetría

```
ws/agents/{tenant_id}   ← stream de AgentEvent para la UI
```

```rust
enum AgentEvent {
    Spawned { agent_id: AgentId, role: AgentRole, parent_id: Option<AgentId>, model: String },
    StateChanged { agent_id: AgentId, state: AgentState },
    Activity { agent_id: AgentId, description: String },
    Reported { agent_id: AgentId, summary: String },
    Pruned { agent_id: AgentId },
    Restored { project_id: ProjectId, node_count: usize },  // árbol restaurado
    TreeSnapshot { tree: Vec<AgentNodeSummary> },
}
```

---

## 8. Shell — Visibilidad en Tiempo Real

### 8.1 En el Chat (colapsable)

```
▶ Aegis OS  •  3 agentes activos               [expandir]
```

Expandido:
```
▼ Aegis OS  •  Project Supervisor  [claude-opus-4]    [✓ consolidando]
  ▼ Kernel  •  Supervisor  [claude-opus-4]            [⟳ en progreso]
    ▶ Scheduler  •  Supervisor                        [⟳ escribiendo mod.rs]
    ▶ Auth  •  Supervisor                             [✓ completado]
  ▼ Shell  •  Supervisor  [claude-sonnet-4]           [⟳ en progreso]
    ▶ ChatTerminal.tsx  •  Specialist                 [⟳ analizando]
```

Nuevo componente: `shell/ui/src/components/AgentActivityPanel.tsx`

### 8.2 En el Dashboard

Panel "Projects & Agents":
- Lista de proyectos activos con estado
- Árbol de agentes de cada proyecto con estado en tiempo real
- Modelo asignado por nodo
- Botón "Ver detalle" → drawer con último state summary
- Botón "Archivar proyecto"

Nuevos componentes:
- `shell/ui/src/components/AgentTreeView.tsx`
- `shell/ui/src/components/ProjectList.tsx`

### 8.3 Store extension

```typescript
// useAegisStore.ts — nuevos campos
agentTree: AgentNodeSummary[];
activeProjects: ProjectSummary[];
connectAgentStream: () => void;
disconnectAgentStream: () => void;
```

---

## 9. ADRs de la Épica

| # | Decisión | Razón |
|---|---|---|
| ADR-CAA-001 | Jerarquía n-ary sin límite de profundidad | La complejidad del trabajo real no es predecible |
| ADR-CAA-002 | Chat Agent con contexto explícitamente limitado | El chat es potencialmente infinito |
| ADR-CAA-003 | Query como mensaje de primera clase | Permite responder preguntas técnicas sin lanzar trabajo completo |
| ADR-CAA-004 | Instrucciones en archivos .md editables en runtime | Comportamiento ajustable sin recompilar |
| ~~ADR-CAA-005~~ | ~~AgentTree efímero~~ | **REVOCADO** — los proyectos persisten entre sesiones |
| ADR-CAA-005v2 | Árbol serializado + state summary por supervisor | El árbol es la estructura (se restaura exacto); el contexto es un resumen (no el historial completo) |
| ADR-CAA-006 | Comunicación lateral solo entre hermanos (mismo padre) | Mantiene la jerarquía limpia |
| ADR-CAA-007 | Specialist no persiste — es efímero | Las hojas son ejecutores puntuales; su estado es el reporte que genera |
| ADR-CAA-008 | AgentEvent stream separado del stream de chat | El stream de chat tiene latencia crítica |
| ADR-CAA-009 | context_budget por AgentNode, no por tenant | Permite calibrar contexto según rol |
| ADR-CAA-010 | Persona del tenant solo en Chat Agent | Los agentes de trabajo no tienen personalidad, solo rol |
| ADR-CAA-011 | Condensación de QueryReply por nivel | El Chat Agent no recibe dumps técnicos |
| ADR-CAA-012 | task_type y model_preference por AgentNode | CMR selecciona el modelo óptimo según la naturaleza cognitiva de cada agente individualmente |
| ADR-CAA-013 | Árbol en filesystem (JSON + .md), proyectos en SQLite | El árbol es grande y debe ser debuggeable; la lista de proyectos es metadata ligera que va en la DB existente |
| ADR-CAA-014 | El padre define el task_type del hijo al spawnearlo | El supervisor conoce la naturaleza del trabajo que asigna mejor que un valor por defecto |

---

## 10. Tickets de la Épica

### Fase 1 — Tipos base (Kernel Engineer)

| ID | Título | Tipo |
|---|---|---|
| CORE-190 | AgentRole, AgentNode, AgentState, AgentId — tipos base con task_type y model_preference | feat |
| CORE-191 | AgentTree — estructura n-ary con serialize/restore | feat |
| CORE-192 | AgentMessage — Dispatch, Report, Query, QueryReply | feat |
| CORE-195 | PCB Extension — campo agent_id + task_type override | feat |
| CORE-196 | ContextBudget — presupuesto de tokens por AgentNode | feat |

### Fase 2 — Orchestration + Persistencia (Kernel Engineer)

| ID | Título | Tipo |
|---|---|---|
| CORE-193 | AgentOrchestrator — ciclo de vida, routing, persist on close, restore on activation | feat |
| CORE-194 | ProjectRegistry — CRUD de proyectos + tabla SQLite + serialize/restore árbol | feat |
| CORE-197 | InstructionLoader — carga de agent/*.md en runtime + state summary template | feat |
| CORE-198 | SYS_AGENT_SPAWN — syscall con task_type opcional | feat |
| CORE-199 | SYS_AGENT_QUERY — syscall de query descendente | feat |
| CORE-206 | AgentPersistence — lectura/escritura agent_tree.json + agent_contexts/*.md | feat |
| CORE-207 | State Summary Generator — trigger al cerrar sesión, genera .md por supervisor | feat |

### Fase 3 — CMR Integration (Kernel Engineer)

| ID | Título | Tipo |
|---|---|---|
| CORE-208 | CMR per-agent — AgentOrchestrator pasa task_type al CMR al crear PCB por agente | feat |

### Fase 4 — Visibilidad (Kernel Engineer + Shell Engineer, paralelo)

| ID | Título | Tipo |
|---|---|---|
| CORE-200 | AgentEvent stream — WebSocket ws/agents/{tenant_id} con evento Restored | feat |
| CORE-204 | useAegisStore — agentTree + activeProjects + connectAgentStream | feat |
| CORE-202 | AgentActivityPanel — indicador colapsable en ChatTerminal con modelo visible | feat |
| CORE-203 | AgentTreeView + ProjectList — panel en Dashboard con árbol en tiempo real | feat |

### Fase 5 — Chat Agent Integration (Kernel Engineer)

| ID | Título | Tipo |
|---|---|---|
| CORE-201 | Chat Agent context limiter — ventana deslizante + resumen automático | feat |

### Fase 6 — Instrucciones (Arquitecto IA — ya completado)

| ID | Título | Estado |
|---|---|---|
| CORE-205 | Archivos agent/*.md — chat_agent, supervisor, specialist + state summary template | ✅ DONE |

---

## 11. Orden de implementación

```
Fase 1: CORE-190 → CORE-191 → CORE-192 → CORE-195 → CORE-196
Fase 2: CORE-193 → CORE-194 → CORE-197 → CORE-198 → CORE-199 → CORE-206 → CORE-207
Fase 3: CORE-208  (depende de Fase 2)
Fase 4: CORE-200 (Kernel) ‖ CORE-204 → CORE-202 → CORE-203 (Shell)  [paralelo con Fase 3]
Fase 5: CORE-201  (depende de Fase 2)
Fase 6: CORE-205  ✅ ya completado
```

---

*Documento creado por Arquitecto IA — 2026-04-26*  
*Actualizado 2026-04-26: persistencia de proyectos (ADR-CAA-005 revocado → ADR-CAA-005v2), CMR por agente (ADR-CAA-012/014), tickets CORE-206/207/208 agregados.*  
*Basado en la visión de Tavo + aprendizajes de opencode-dev.*  
*Extiende y reemplaza Epic 43.*
