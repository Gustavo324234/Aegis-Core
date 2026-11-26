# CORE-288 — Kernel: fix síntesis de reportes hijos (loop infinito potencial)

**Tipo:** fix  
**Prioridad:** Alta  
**Estado:** ✅ Done  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs` (arm `Report` en `run_agent_loop`)

---

## Contexto

En el arm `AgentMessage::Report` de `run_agent_loop`, cuando todos los hijos completan,
el supervisor se manda un `Dispatch` a sí mismo:

```rust
let synth = AgentMessage::Dispatch {
    task_description: "Synthesize all child reports.".to_string(),
    context: AgentContext { child_reports: child_results, ... },
    reply_to: agent_id,
    deadline_ms: None,
};
let self_tx = { channels.read().await.get(&agent_id).cloned() };
if let Some(stx) = self_tx {
    let _ = stx.send(synth).await;
}
```

El supervisor recibe su propio Dispatch, llama al LLM, genera una respuesta,
la guarda como `last_report`, y marca su estado como `Complete`. Luego el
bucle `while let Some(msg) = rx.recv().await` recibe el siguiente mensaje.

**El problema:** después de `Complete`, el supervisor no deja de escuchar.
Si el padre le manda otro `Report` (de un hijo que llegó tarde), vuelve a
evaluar `all_done` — que sigue siendo true porque los hijos ya completaron —
y se manda otro `Dispatch` de síntesis. Y otro. Y otro.

Esto no es un loop infinito real (requiere que el padre siga enviando reportes),
pero es un bug de correctness: el supervisor puede sintetizar múltiples veces
y sobreescribir su `last_report` con síntesis redundantes.

---

## Cambios requeridos

### 1. Agregar flag `synthesis_done` al contexto del loop

El loop necesita saber que ya sintetizó para no hacerlo de nuevo:

```rust
async fn run_agent_loop(...) {
    // ... código existente de setup ...
    
    let mut synthesis_done = false;  // CORE-288: flag anti-loop

    loop {  // cambiar while let por loop con select (CORE-286 ya lo hace)
        let msg = /* tokio::select! con timeout (CORE-286) */;

        match msg {
            AgentMessage::Report { from, result, status } => {
                // ... código existente de actualización del árbol ...

                let all_done = { /* código existente */ };

                // CORE-288: solo sintetizar UNA vez
                if all_done && !synthesis_done {
                    synthesis_done = true;  // marcar antes de enviar para evitar re-entrada
                    
                    let child_results = { /* código existente */ };
                    let synth = AgentMessage::Dispatch {
                        task_description: "Synthesize all child reports.".to_string(),
                        context: AgentContext {
                            child_reports: child_results,
                            ..Default::default()
                        },
                        reply_to: agent_id,
                        deadline_ms: None,
                    };
                    let self_tx = { channels.read().await.get(&agent_id).cloned() };
                    if let Some(stx) = self_tx {
                        let _ = stx.send(synth).await;
                    }
                }
                // Si all_done && synthesis_done → ignorar silenciosamente
            }

            AgentMessage::Dispatch { .. } => {
                // ... código existente de LLM call ...
                // Después de Complete, si llega otro Dispatch ignorarlo:
                {
                    let state = tree.read().await.get(&agent_id).map(|n| n.state.clone());
                    if matches!(state, Some(AgentState::Complete)) && synthesis_done {
                        // Ya completamos — ignorar dispatch tardío
                        continue;
                    }
                }
                // ... resto del código existente ...
            }

            // ... otros arms sin cambios ...
        }
    }
}
```

### 2. Salir del loop después de `Complete`

Después de que el agente marca su estado como `Complete` en el arm `Dispatch`,
verificar si debe terminar:

```rust
// Al final del arm Dispatch, después de set_state(Complete):
{
    let state = tree.read().await.get(&agent_id).map(|n| n.state.clone());
    if matches!(state, Some(AgentState::Complete)) {
        // Si no tiene padre que espere reporte, el loop puede terminar
        if parent_tx.is_none() {
            // ProjectSupervisor — terminar después de Complete
            channels.write().await.remove(&agent_id);
            break;
        }
        // Supervisores con padre: seguir escuchando por si el padre
        // necesita una Query. Pero con synthesis_done = true no van a
        // sintetizar de nuevo.
    }
}
```

---

## Criterios de aceptación

- [x] Un supervisor con todos los hijos completados sintetiza exactamente UNA vez
- [x] Si llegan reportes tardíos de hijos después de la síntesis, se ignoran
- [x] Si llegan Dispatches tardíos después de Complete, se ignoran
- [x] Un `ProjectSupervisor` (sin padre) termina su loop después de `Complete`
- [x] Supervisores intermedios (con padre) siguen escuchando después de `Complete`
  para posibles Queries del padre
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-286 (timeout en run_agent_loop — este ticket modifica el mismo loop,
  coordinar para que no generen conflictos en el PR)
