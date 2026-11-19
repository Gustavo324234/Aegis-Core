# CORE-286 — Kernel: timeout en run_agent_loop + cleanup de tasks zombies

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs`
**Estado:** ✅ Completo

---

## Contexto

`run_agent_loop` espera mensajes con `rx.recv()` sin límite de tiempo.
Si un agente queda bloqueado esperando un Dispatch que nunca llega, o si el
padre muere sin enviar Cancel, el task de tokio vive indefinidamente consumiendo
recursos. Con múltiples tenants y proyectos activos esto acumula tasks zombies.

El segundo problema es que `spawn_loop` crea un nuevo par `(tx, rx)` y reemplaza
el `tx` en el mapa de canales, pero el `rx` del par anterior queda huérfano —
el `tx` viejo sigue en el mapa hasta que se reemplaza. Esto puede causar que
mensajes enviados al `tx` viejo se pierdan silenciosamente.

---

## Cambios requeridos

### 1. Timeout en el loop principal de `run_agent_loop`

Reemplazar el `while let Some(msg) = rx.recv().await` por un select con timeout:

```rust
const AGENT_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300); // 5 min

loop {
    let msg = tokio::select! {
        msg = rx.recv() => {
            match msg {
                Some(m) => m,
                None => {
                    // Canal cerrado — el orquestador terminó el agente
                    info!("[{}] Channel closed, loop exiting.", role_label);
                    break;
                }
            }
        }
        _ = tokio::time::sleep(AGENT_IDLE_TIMEOUT) => {
            // Timeout de inactividad — el agente lleva 5 min sin recibir nada
            warn!(
                agent = %agent_id,
                "[{}] Idle timeout after {}s — self-terminating.",
                role_label,
                AGENT_IDLE_TIMEOUT.as_secs()
            );
            // Marcar como failed en el árbol
            {
                let mut t = tree.write().await;
                if let Some(n) = t.get_mut(&agent_id) {
                    // Solo marcar failed si sigue Idle o Running — no tocar Complete
                    if matches!(n.state, AgentState::Idle | AgentState::Running | AgentState::WaitingReport) {
                        n.set_state(AgentState::Failed {
                            reason: "Idle timeout — no messages received in 5 minutes".to_string(),
                        });
                    }
                }
            }
            // Remover canal
            channels.write().await.remove(&agent_id);
            break;
        }
    };

    // ... procesamiento de msg igual que antes ...
}
```

El timeout de 5 minutos es conservador — suficientemente largo para que el agente
complete trabajo real, suficientemente corto para no acumular zombies indefinidamente.

Los agentes en estado `WaitingUser` (esperando respuesta del usuario via `ask_user`)
tienen su propio timeout de 10 minutos implementado en CORE-263 — este timeout
de `run_agent_loop` no interfiere porque el agente recibe el `SupervisorResumed`
o el `Cancel` dentro de ese tiempo.

### 2. Fix de canal en `spawn_loop`

El problema en `spawn_loop` es que crea un nuevo `(tx, rx)` pero el código de
`if tx_existing.is_some()` nunca actualiza el `rx` original. Reemplazar el
bloque de creación de canales por uno más simple:

```rust
fn spawn_loop(&self, agent_id: AgentId, parent_tx: Option<mpsc::Sender<AgentMessage>>) {
    let tree_ref = Arc::clone(&self.tree);
    let router_ref = Arc::clone(&self.router);
    let channels_ref = Arc::clone(&self.channels);
    let hal_ref = Arc::clone(&self.hal);

    // Crear siempre un par fresco — el tx viejo quedaba huérfano
    let (tx, rx) = mpsc::channel::<AgentMessage>(32);

    // Actualizar el canal antes de hacer spawn del task
    {
        if let Ok(mut ch) = channels_ref.try_write() {
            ch.insert(agent_id, tx);
        }
    }

    tokio::spawn(async move {
        Self::run_agent_loop(
            agent_id,
            tree_ref,
            router_ref,
            channels_ref,
            rx,
            parent_tx,
            hal_ref,
        )
        .await;
    });
}
```

El `tx` se inserta en el mapa **antes** de que el task arranque, eliminando
la race condition donde el padre podría intentar enviar un mensaje antes de
que el hijo esté escuchando.

### 3. Cleanup en `terminate`

Agregar al final de `terminate`, después de remover canales del mapa:

```rust
// Marcar todos los nodos terminados como Failed si no estaban Complete
{
    let mut tree = self.tree.write().await;
    let all_ids: Vec<AgentId> = std::iter::once(*agent_id)
        .chain(descendants.iter().copied())
        .collect();
    for id in all_ids {
        if let Some(n) = tree.get_mut(&id) {
            if !matches!(n.state, AgentState::Complete) {
                n.set_state(AgentState::Failed {
                    reason: "Terminated by orchestrator".to_string(),
                });
            }
        }
    }
}
```

---

## Criterios de aceptación

- [x] Un agente que no recibe mensajes durante 5 minutos se auto-termina limpiamente
- [x] El estado del agente en el árbol se marca como `Failed` con razón de timeout
- [x] El canal del agente se remueve del mapa al hacer timeout
- [x] `spawn_loop` inserta el `tx` en el mapa **antes** de hacer spawn del tokio task
- [x] `terminate` marca los nodos como Failed si no estaban Complete
- [x] Los agentes en `WaitingUser` no se ven afectados — tienen su propio timeout
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-263 ✅ (WaitingUser timeout implementado — no tocarlo)
