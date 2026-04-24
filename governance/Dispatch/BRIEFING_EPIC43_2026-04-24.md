# BRIEFING — Kernel Engineer + Shell Engineer
# Epic 43: Hierarchical Multi-Agent Orchestration
# Fecha: 2026-04-24

---

## INSTRUCCIONES DE SESIÓN

Sos el ingeniero de implementación del proyecto Aegis-Core.
Tu trabajo en esta sesión es implementar la **Epic 43 completa**.

**Antes de escribir una sola línea de código, leé:**
1. `governance/EPIC_43_HIERARCHICAL_AGENTS.md` — arquitectura y ADRs
2. Los tickets individuales en `governance/Tickets/CORE-155.md` a `CORE-166.md`
3. El código existente relevante (scheduler, pcb, dag, router, syscalls)

---

## CONTEXTO DEL REPO

- Monorepo en `C:\Aegis\Aegis-Core`
- Kernel: `kernel/crates/ank-core/` (Rust/Tokio)
- Shell: `shell/ui/src/` (React/TypeScript/Tailwind)
- Governance: `governance/Tickets/` y `governance/TICKETS_MASTER.md`

---

## SCOPE DE LA ÉPICA

### Tickets Kernel Engineer (Rust — kernel/crates/ank-core/)

Implementar en este orden:

**Fase 1 — Tipos base (sin dependencias entre sí):**
- `CORE-155`: Crear `ank-core/src/agents/mod.rs` + `node.rs` con AgentNode, AgentRole, AgentState, AgentId
- `CORE-156`: Crear `ank-core/src/agents/tree.rs` con AgentTree y sus operaciones
- `CORE-157`: Crear `ank-core/src/agents/message.rs` con AgentMessage, AgentContext, AgentResult
- `CORE-160`: Agregar `pub agent_id: Option<AgentId>` al PCB existente (campo opcional, sin romper nada)
- `CORE-161`: Agregar `pub agent_id: Option<AgentId>` al DagNode existente (ídem)

**Fase 2 — Orquestador (depende de Fase 1):**
- `CORE-158`: Crear `ank-core/src/agents/orchestrator.rs` con AgentOrchestrator e integrarlo en el CognitiveScheduler
- `CORE-159`: Crear `ank-core/src/agents/project.rs` con ProjectRegistry y la migración de tabla SQLCipher

**Fase 3 — Features avanzados (dependen de Fase 2):**
- `CORE-162`: Implementar syscall SYS_AGENT_SPAWN en `ank-core/src/syscalls/`
- `CORE-163`: Agregar rutas HTTP `/api/agents/*` en `ank-http/src/routes/agents.rs`
- `CORE-165`: Integrar CMR con AgentOrchestrator (cada agente usa su TaskType para selección de modelo)

### Tickets Shell Engineer (TypeScript — shell/ui/src/)

- `CORE-166`: Crear `AgentTreeWidget.tsx` e integrarlo en `Dashboard.tsx` del tenant

**CORE-166 puede implementarse en paralelo con los tickets de Rust** — usa mock data cuando el backend no está listo. Ver el ticket para el mock completo.

---

## REGLAS DE IMPLEMENTACIÓN (NO NEGOCIABLES)

1. **Zero `unwrap()` / `expect()`** en código nuevo — usar `?` con `anyhow::Result`
2. **Zero warnings de Clippy** — el CI rechaza cualquier warning
3. **`cargo build` debe pasar** al final de cada ticket antes de commitear
4. **No `cargo test` local** — los tests corren en CI tras el push
5. **Un commit por ticket**, formato: `feat(ank-core): CORE-155 AgentNode tipos base`
6. **Rama**: crear `feat/epic-43-multiagent` y trabajar ahí
7. **No hacer push a main** — Tavo mergea manualmente

## CONVENCIÓN DE COMMITS

```
feat(ank-core): CORE-155 AgentNode, AgentRole, AgentState tipos base
feat(ank-core): CORE-156 AgentTree estructura en memoria
feat(ank-core): CORE-157 AgentMessage protocolo inter-agente
feat(ank-core): CORE-160 PCB extension campo agent_id
feat(ank-core): CORE-161 DagNode extension campo agent_id
feat(ank-core): CORE-158 AgentOrchestrator ciclo de vida
feat(ank-core): CORE-159 ProjectRegistry persistencia SQLCipher
feat(ank-core): CORE-162 SYS_AGENT_SPAWN syscall
feat(ank-http): CORE-163 HTTP routes api/agents
feat(ank-core): CORE-165 CMR integration por AgentNode
feat(shell): CORE-166 AgentTreeWidget en Dashboard del tenant
```

---

## ADVERTENCIAS CRÍTICAS

- **No crear un Scheduler nuevo** — el AgentOrchestrator se integra con el CognitiveScheduler **existente** como un campo adicional. Leer el código actual antes de tocar el scheduler.
- **No crear un DAG engine nuevo** — solo agregar `Option<AgentId>` al DagNode existente.
- **No crear instancias nuevas de CognitiveRouter o VCM** — el AgentOrchestrator recibe `Arc<CognitiveRouter>` y `Arc<VirtualContextManager>` del Scheduler existente.
- **La migración de SQLCipher en CORE-159 es additive** — no puede romper tenants existentes sin la tabla nueva.
- **CORE-164 está DEPRECADO** — ignorarlo. El ticket correcto de Shell es CORE-166.

---

## CÓMO EMPEZAR

```bash
# 1. Crear rama
git checkout -b feat/epic-43-multiagent

# 2. Leer el código existente relevante
# - kernel/crates/ank-core/src/pcb.rs
# - kernel/crates/ank-core/src/scheduler/
# - kernel/crates/ank-core/src/dag/
# - kernel/crates/ank-core/src/router/
# - kernel/crates/ank-core/src/syscalls/
# - kernel/crates/ank-core/src/lib.rs

# 3. Implementar en el orden de fases indicado arriba

# 4. Verificar compilación tras cada ticket
cargo build 2>&1 | head -50
```

---

*Briefing generado por Arquitecto IA — 2026-04-24*
*Epic 43 — Hierarchical Multi-Agent Orchestration*
