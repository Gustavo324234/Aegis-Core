# EPIC 43 — Hierarchical Multi-Agent Orchestration

**Estado:** PLANNED  
**Fecha de diseño:** 2026-04-24  
**Arquitecto:** Arquitecto IA  
**Repo:** Aegis-Core (kernel/crates/ank-core)

---

## 1. Visión

Transformar el sistema multiagente actual (Supervisor/Worker de 2 niveles, CORE-154) en una **jerarquía n-ary de agentes cognitivos**, análoga a la estructura de una empresa:

```
Usuario (CEO implícito)
    └── ProjectSupervisor "Aegis OS"
    │       ├── DomainSupervisor "Kernel Engineer"
    │       │       ├── SpecialistAgent "Rust/Scheduler"
    │       │       ├── SpecialistAgent "Rust/Auth"
    │       │       └── SpecialistAgent "Rust/MCP"
    │       └── DomainSupervisor "Shell Engineer"
    │               ├── SpecialistAgent "React/UI"
    │               └── SpecialistAgent "Python/BFF"
    └── ProjectSupervisor "Web App"
            ├── DomainSupervisor "Frontend"
            │       └── SpecialistAgent "NextJS"
            └── DomainSupervisor "Backend"
                    └── SpecialistAgent "API/DB"
```

**Principio clave:** El usuario habla con el sistema como un CEO. El sistema infiere qué proyectos y qué agentes activar. Los agentes reportan hacia arriba y coordinan lateralmente solo a través de su supervisor común.

---

## 2. Conceptos Clave

### 2.1 AgentNode
Unidad fundamental del árbol. Cada nodo tiene:
- `agent_id: AgentId` — UUID único
- `role: AgentRole` — PROJECT_SUPERVISOR | DOMAIN_SUPERVISOR | SPECIALIST
- `project_id: ProjectId` — a qué proyecto pertenece
- `parent_id: Option<AgentId>` — quién lo supervisa (None = raíz)
- `children: Vec<AgentId>` — agentes bajo su mando
- `system_prompt: String` — contexto e instrucciones de rol
- `model_preference: ModelPreference` — qué tipo de modelo usar (delega al CMR)
- `task_type: TaskType` — naturaleza cognitiva del agente (CODE, ANALYSIS, PLANNING, etc.)
- `state: AgentState` — IDLE | RUNNING | WAITING_REPORT | COMPLETE | FAILED
- `context_budget: usize` — tokens máximos disponibles (VCM enforces)

### 2.2 AgentTree
Estructura de datos en memoria que representa la jerarquía completa para una sesión de usuario.

```
AgentTree {
    roots: Vec<AgentId>,          // ProjectSupervisors activos
    nodes: HashMap<AgentId, AgentNode>,
    project_index: HashMap<ProjectId, AgentId>,  // proyecto → su supervisor raíz
}
```

### 2.3 AgentMessage
Protocolo de comunicación entre nodos:

```rust
enum AgentMessage {
    // Hacia abajo (supervisor → subordinado)
    Dispatch { task: String, context: AgentContext, reply_to: AgentId },
    
    // Hacia arriba (subordinado → supervisor)  
    Report { result: AgentResult, status: ReportStatus },
    
    // Lateral — PROHIBIDO entre agentes de distinto supervisor
    // La coordinación siempre sube y baja por la jerarquía
}
```

### 2.4 ModelRouter Integration
El CMR (CognitiveRouter existente en `ank-core/src/router/`) ya conoce `TaskType`. Cada agente delega la selección de modelo al CMR con su propio `task_type`. Esto garantiza que:
- Un SpecialistAgent de código recibe el mejor modelo de coding disponible
- Un ProjectSupervisor de planning recibe el mejor modelo de razonamiento
- El KeyPool se comparte globalmente (sin duplicación de keys)

### 2.5 Spawn Mechanism
Un AgentNode puede **spawnear subordinados dinámicamente** durante su ejecución:

```
ProjectSupervisor recibe tarea compleja
    → decide que necesita un DomainSupervisor "DevOps"
    → llama SYS_AGENT_SPAWN(role=DOMAIN_SUPERVISOR, domain="devops", parent=self)
    → el nuevo nodo se registra en AgentTree
    → el ProjectSupervisor le hace Dispatch
```

### 2.6 Context Isolation
Cada agente recibe un **contexto mínimo necesario**:
- Su system_prompt (rol + instrucciones)
- Solo los archivos relevantes a su tarea (VCM filtra)
- El resultado del último reporte de sus hijos (no el historial completo)

Esto es crítico: evita que un SpecialistAgent de UI vea el código de autenticación del kernel.

---

## 3. Arquitectura de Implementación

### 3.1 Nuevo módulo: `ank-core/src/agents/`

```
ank-core/src/agents/
├── mod.rs              # re-exports
├── node.rs             # AgentNode, AgentRole, AgentState, AgentId
├── tree.rs             # AgentTree: insert, spawn, get_children, prune
├── message.rs          # AgentMessage, AgentContext, AgentResult, ReportStatus
├── orchestrator.rs     # AgentOrchestrator: coordina el árbol, gestiona el ciclo de vida
└── project.rs          # ProjectRegistry: mapea nombres de proyecto → ProjectSupervisors
```

### 3.2 Integración con el Scheduler existente

El `CognitiveScheduler` (en `ank-core/src/scheduler/`) ya gestiona PCBs. La propuesta es que **cada AgentNode tenga un PCB asociado**. El Scheduler ya sabe cómo hacer inferencia concurrente con Tokio; los agentes simplemente son procesos cognitivos con genealogía.

**No se reemplaza el Scheduler** — se extiende el PCB con `Option<AgentId>`.

### 3.3 Integración con el DAG existente

Los sub-agentes pueden ser nodos en un DAG de tareas. El flujo es:

```
ProjectSupervisor compila un S-DAG donde cada nodo es:
    → una tarea atómica asignada a un SpecialistAgent
    → las dependencias del DAG representan el orden de ejecución
    → el Scatter-Gather Scheduler (ya existente) ejecuta en paralelo
```

**No se reemplaza el DAG** — se añade `Option<AgentId>` a `DagNode`.

### 3.4 Nuevo endpoint HTTP: `/agents`

En `ank-http`, nuevas rutas bajo `/api/agents/`:
- `GET /api/agents/tree` — visualización del árbol activo
- `POST /api/agents/spawn` — spawnear un agente manualmente (admin/debug)
- `GET /api/agents/{id}/status` — estado de un agente específico

### 3.5 Shell: AgentTreeView

Nuevo componente en `shell/ui/src/components/`:
- `AgentTreeView.tsx` — árbol colapsable con estado en tiempo real
- `AgentCard.tsx` — card individual con rol, modelo asignado, estado, último reporte

---

## 4. Flujo End-to-End (Ejemplo)

```
Usuario: "Seguimos con Aegis y además arrancamos la web app del portfolio"

1. ank-http recibe el mensaje WebSocket
2. CognitiveScheduler lo evalúa como TASK_TYPE_PLANNING
3. AgentOrchestrator detecta dos proyectos:
   - "Aegis" → ya existe ProjectSupervisor activo → reactiva
   - "Portfolio web app" → nuevo → crea ProjectSupervisor
4. Cada ProjectSupervisor recibe su fragmento del mensaje original
5. ProjectSupervisor "Aegis" decide necesitar:
   - DomainSupervisor "Kernel" → spawnea si no existe
   - DomainSupervisor "Shell" → spawnea si no existe
6. Cada DomainSupervisor genera tareas para sus SpecialistAgents
7. SpecialistAgents ejecutan → reportan resultados
8. DomainSupervisors agregan → reportan a ProjectSupervisors
9. ProjectSupervisors consolidan → respuesta unificada al usuario
```

---

## 5. Protocolo de Reportes

Cada nivel agrega y resume el trabajo de sus subordinados antes de reportar hacia arriba. La respuesta que recibe el usuario es un **resumen ejecutivo** del ProjectSupervisor, con la opción de explorar más profundo.

```
SpecialistAgent: "Refactoricé scheduler.rs. Cambios en líneas 45-89. 
                  cargo build: OK. Sin breaking changes."

DomainSupervisor: "Kernel Engineer completó: (1) scheduler refactorizado,
                   (2) auth bug resuelto. Ambos compilados. Listos para review."

ProjectSupervisor: "Aegis OS: 2 tareas kernel completadas, 1 tarea shell 
                   en progreso. ETA: ~5 min."
```

---

## 6. ADRs de la Épica

| # | Decisión | Razón |
|---|---|---|
| ADR-AGENTS-001 | AgentTree es una estructura en memoria (no persistida en SQLCipher) | Los árboles de agentes son efímeros por sesión; la persistencia agrega complejidad sin beneficio real |
| ADR-AGENTS-002 | La comunicación lateral entre agentes de distinto supervisor está prohibida | Mantiene la jerarquía limpia; toda coordinación cross-domain sube al supervisor común |
| ADR-AGENTS-003 | Cada AgentNode se mapea a un PCB existente en el Scheduler | Reutiliza la infraestructura de scheduling en lugar de crear un scheduler paralelo |
| ADR-AGENTS-004 | El ProjectSupervisor compila un S-DAG para distribuir trabajo | Reutiliza el DAG engine existente; no inventar nuevo sistema de paralelismo |
| ADR-AGENTS-005 | El CMR asigna modelo por TaskType del AgentNode, no por nivel jerárquico | El nivel es metadata organizacional; la naturaleza cognitiva de la tarea determina el modelo |
| ADR-AGENTS-006 | Context isolation es responsabilidad del AgentOrchestrator + VCM | El agente no decide su propio contexto; el orquestador filtra según scope declarado |
| ADR-AGENTS-007 | El árbol de agentes es visible en la Shell como componente de telemetría | La transparencia del sistema es un valor de Aegis; el usuario debe poder ver qué está pasando |

---

## 7. Tickets de la Épica

Ver tickets individuales en `governance/Tickets/`:

| ID | Título | Tipo | Asignado a |
|---|---|---|---|
| CORE-155 | AgentNode, AgentRole, AgentState — tipos base | feat | Kernel Engineer |
| CORE-156 | AgentTree — estructura en memoria y operaciones | feat | Kernel Engineer |
| CORE-157 | AgentMessage — protocolo de comunicación inter-agente | feat | Kernel Engineer |
| CORE-158 | AgentOrchestrator — ciclo de vida y coordinación | feat | Kernel Engineer |
| CORE-159 | ProjectRegistry — gestión de proyectos y supervisores raíz | feat | Kernel Engineer |
| CORE-160 | PCB Extension — campo agent_id en ProcessControlBlock | feat | Kernel Engineer |
| CORE-161 | DagNode Extension — campo agent_id en DagNode | feat | Kernel Engineer |
| CORE-162 | SYS_AGENT_SPAWN syscall — spawn dinámico desde agente | feat | Kernel Engineer |
| CORE-163 | HTTP routes /api/agents/* — árbol y estado | feat | Kernel Engineer |
| CORE-164 | AgentTreeView + AgentCard — UI en Shell | feat | Shell Engineer |
| CORE-165 | Model-per-Agent — CMR integration con TaskType por AgentNode | feat | Kernel Engineer |

---

*Documento creado por Arquitecto IA — 2026-04-24*
*Épica siguiente a EPIC 42 (Vision Realignment & Autonomy)*
*Pre-requisito: CORE-154 completado o esta épica lo reemplaza/extiende*
