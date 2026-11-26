# CORE-279 — Kernel: WebSocket keepalive (ping cada 30s)

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-http/src/ws/chat.rs`
**Estado:** ✅ Completo

---

## Contexto

El WebSocket se reconecta cada ~2m20s porque Cloudflare tunnel cierra conexiones
idle sin tráfico. El kernel no envía keepalives — no hay ningún `Message::Ping`
en el código actual.

Cada reconexión re-inicializa el enclave del tenant y corta el contexto de la
conversación en curso.

---

## Cambios requeridos

En `handle_chat`, agregar un ticker de ping en el `tokio::select!` del loop principal:

```rust
async fn handle_chat(
    mut socket: WebSocket,
    tenant_id: String,
    raw_session_key: Option<String>,
    state: AppState,
) {
    // ... código existente de auth y welcome ...

    // CORE-279: Ticker de keepalive — ping cada 30s para mantener la conexión viva
    // a través de proxies (Cloudflare, nginx, etc.)
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // ... resto del setup existente ...

    loop {
        // Forward pending workspace events (código existente)
        loop {
            match workspace_rx.try_recv() {
                Ok(event) if event.tenant_id == tenant_id => {
                    let _ = socket.send(Message::Text(event.payload.to_string())).await;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        let msg = tokio::select! {
            // arm existente: mensajes del WebSocket
            ws_next = socket.recv() => {
                match ws_next {
                    Some(Ok(m)) => m,
                    _ => break,
                }
            }
            // arm existente: agent events
            event_result = agent_event_rx.recv() => {
                match event_result {
                    Ok(event) => {
                        if let Ok(data) = serde_json::to_value(&event) {
                            let frame = json!({ "event": "agent_event", "data": data });
                            let _ = socket.send(Message::Text(frame.to_string())).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[ChatWS] tenant={} agent_event_rx lagged by {} events", tenant_id, n);
                    }
                    Err(_) => {}
                }
                continue;
            }
            // CORE-279: arm nuevo — ping keepalive
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break; // conexión muerta
                }
                continue;
            }
        };

        // Ignorar Pong — el browser responde automáticamente, no necesitamos procesarlo
        let msg_text = match msg {
            Message::Text(t) => t,
            Message::Pong(_) => continue,  // CORE-279: ignorar pongs
            Message::Close(_) => break,
            _ => continue,
        };

        // ... resto del handler existente sin cambios ...
    }
}
```

El intervalo de 30 segundos está por debajo del timeout de Cloudflare (~100-140s)
y de la mayoría de proxies nginx (default 60s).

---

## Criterios de aceptación

- [x] El WebSocket envía un `Message::Ping` cada 30 segundos de inactividad
- [x] Los `Message::Pong` del browser son ignorados sin procesar
- [x] Si el send del Ping falla (conexión muerta), el loop termina limpiamente
- [x] La reconexión cada ~2m20s desaparece en producción con Cloudflare
- [x] `cargo build --workspace` sin errores

## Dependencias

- Ninguna — cambio aislado en `ws/chat.rs`
