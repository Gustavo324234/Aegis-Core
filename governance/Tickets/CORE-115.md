# CORE-115 — Preemption real en `CognitiveScheduler` via `CancellationToken`

**Epic:** Epic 36 — Post-Launch Improvements (PLANNED)
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA — Fairness
**Estado:** TODO
**Origen:** REC-009 / Auditoría multi-modelo 2026-04-16

---

## Contexto

`CognitiveScheduler::handle_event(SchedulerEvent::PreemptCurrent)` está marcado
con un TODO y no interrumpe la inferencia en curso. El scheduler tiene lógica de
prioridades via `BinaryHeap`, pero esa prioridad solo se aplica en el orden de
despacho inicial — una vez que una tarea está en ejecución en el HAL Runner, no
puede ser interrumpida por ninguna tarea de mayor prioridad.

Para un sistema operativo cognitivo, la preemption es fundamental: una tarea de
alta prioridad (voz, comando crítico) no debería esperar a que termine una
inferencia larga de baja prioridad.

**Archivo afectado:** `kernel/crates/ank-core/src/scheduler/mod.rs`

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/scheduler/mod.rs`
**Archivo:** `kernel/crates/ank-server/src/main.rs` (HAL Runner)
**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs` (HAL)
**Dependencia nueva:** `tokio-util` (probablemente ya en el tree)

### Cambio 1 — Agregar `CancellationToken` al PCB

```rust
use tokio_util::sync::CancellationToken;

pub struct PCB {
    // ... campos existentes ...
    pub cancel_token: CancellationToken,
}

impl PCB {
    pub fn new(/* ... */) -> Self {
        Self {
            // ...
            cancel_token: CancellationToken::new(),
        }
    }
}
```

### Cambio 2 — HAL Runner verifica el token durante el stream

```rust
// En main.rs, HAL Runner loop
while let Some(chunk) = stream.next().await {
    if pcb.read().await.cancel_token.is_cancelled() {
        warn!("PID {} cancelled via preemption", pid);
        break;
    }
    // ... procesar chunk ...
}
```

O usando `tokio::select!` para mayor responsividad:

```rust
loop {
    tokio::select! {
        chunk = stream.next() => {
            match chunk {
                Some(token) => { /* enviar token */ }
                None => break,
            }
        }
        _ = pcb.read().await.cancel_token.cancelled() => {
            warn!("PID {} preempted", pid);
            break;
        }
    }
}
```

### Cambio 3 — Implementar `PreemptCurrent` en el scheduler

```rust
SchedulerEvent::PreemptCurrent => {
    if let Some(pid) = &self.current_running {
        if let Some(pcb) = self.process_table.get(pid) {
            let pcb = pcb.read().await;
            info!("Preempting PID {}", pid);
            pcb.cancel_token.cancel();
            // El HAL Runner detectará la cancelación y enviará ProcessCompleted
        }
    }
}
```

### Consideración de scope

Este ticket es más invasivo que CORE-111/112/113/114 ya que toca el PCB, el
scheduler, el HAL Runner y el HAL. Se recomienda implementarlo en un Epic
separado post-launch, no en el sprint actual de hardening.

**No bloquea el lanzamiento — es una mejora de comportamiento.**

## Criterios de aceptación

- [ ] `SchedulerEvent::PreemptCurrent` cancela la inferencia en curso vía `CancellationToken`
- [ ] El proceso preemptado queda en estado `Preempted` (no `Failed`) en la process table
- [ ] La tarea que disparó la preemption es despachada inmediatamente
- [ ] El stream WebSocket del proceso preemptado recibe un evento de cierre limpio
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] Test unitario en `scheduler/mod.rs` que verifica el flujo de preemption

## Dependencias

- `tokio-util` (verificar si ya está en `Cargo.toml`)
- Implementar después de CORE-111 (watchdog del scheduler)
- No depende de CORE-112/113/114
