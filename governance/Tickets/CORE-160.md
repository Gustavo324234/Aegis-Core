# CORE-160 — feat: PCB Extension — Campo agent_id en ProcessControlBlock

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155

---

## Contexto

El `ProcessControlBlock` (PCB) en `ank-core/src/pcb.rs` representa la unidad de proceso del Scheduler. Para integrar los agentes con el Scheduler existente (ADR-AGENTS-003), cada PCB necesita saber si pertenece a un agente del árbol jerárquico.

Este ticket es **mínimamente invasivo**: solo agrega un campo opcional al PCB existente sin tocar la lógica del Scheduler.

---

## Cambios Requeridos

### Modificar `ank-core/src/pcb.rs`

Agregar campo al struct `ProcessControlBlock`:

```rust
use crate::agents::node::AgentId;

pub struct ProcessControlBlock {
    // ... campos existentes, NO modificar ...
    
    /// Si este PCB corresponde a un agente en el árbol jerárquico,
    /// aquí está su AgentId. None para procesos cognitivos normales.
    pub agent_id: Option<AgentId>,
}
```

Actualizar el constructor/builder del PCB para que el nuevo campo sea `None` por defecto, manteniendo compatibilidad con todo el código existente que crea PCBs.

### Criterio de compatibilidad

Todo código existente que construye PCBs debe seguir compilando **sin modificaciones**. El campo `agent_id` debe tener valor `None` a menos que se asigne explícitamente.

Si el constructor del PCB es un struct literal (ej. `ProcessControlBlock { pid, task, ... }`), se puede usar `..Default::default()` para el nuevo campo o hacer que `agent_id` tenga un valor por defecto.

---

## Criterios de Aceptación

- [ ] `ProcessControlBlock` tiene campo `pub agent_id: Option<AgentId>`
- [ ] Todo el código existente que crea PCBs sigue compilando sin cambios
- [ ] El campo por defecto es `None` (no rompe nada existente)
- [ ] `cargo build` del workspace completo pasa sin warnings

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: PCB.agent_id existe en pcb.rs:313.
