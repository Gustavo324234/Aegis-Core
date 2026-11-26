# CORE-257 — Kernel: Tunnel Manager no debe reintentar si cloudflared no está instalado

**Tipo:** fix  
**Prioridad:** Media  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Kernel Engineer  
**Componente:** `kernel/crates/ank-server/src/main.rs` o `tunnel_manager.rs`

**Estado:** ✅ Done
---

## Contexto

El Tunnel Manager intenta iniciar `cloudflared` cada 10 segundos indefinidamente. Si `cloudflared` no está instalado (como en Windows sin cloudflared), el log se llena con spam:

```
WARN Tunnel Manager: cloudflared error: Failed to spawn cloudflared.exe: program not found
INFO Tunnel Manager: Restarting in 10 seconds...
```

Esto ocurre una vez cada 10 segundos por toda la vida del proceso, haciendo el log ilegible para debug.

## Cambios requeridos

En el Tunnel Manager, distinguir entre dos tipos de error:

- **Error de "programa no encontrado"** (`ErrorKind::NotFound`) → loguear UNA sola vez como `WARN` y detener los reintentos permanentemente. Cloudflared no está instalado — reintentar no tiene sentido.
- **Error transitorio** (cloudflared instalado pero falló por red, config, etc.) → continuar reintentando con backoff.

```rust
match Command::new("cloudflared").spawn() {
    Ok(child) => { /* normal flow */ }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        warn!("Tunnel Manager: cloudflared not found. Tunnel disabled. \
               Install cloudflared to enable HTTPS access.");
        // Salir del loop — no reintentar
        return;
    }
    Err(e) => {
        warn!("Tunnel Manager: cloudflared error: {}. Restarting in 10s...", e);
        tokio::time::sleep(Duration::from_secs(10)).await;
        // Continuar el loop
    }
}
```

## Criterios de aceptación

- [x] Si cloudflared no está instalado, el log muestra el WARN exactamente UNA vez al arrancar y no vuelve a aparecer.
- [x] Si cloudflared está instalado pero falla transitoriamente, los reintentos continúan normalmente.
- [x] `cargo build --workspace` sin errores.

## Dependencias

Ninguna.
