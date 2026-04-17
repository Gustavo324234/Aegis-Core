# CORE-110 — Rate limiting en `/api/auth/login`

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🔴 ALTA — Seguridad
**Estado:** TODO
**Origen:** REC-003 / Auditoría multi-modelo 2026-04-16

---

## Contexto

El endpoint `POST /api/auth/login` no tiene ningún mecanismo de limitación de
intentos. Un atacante puede realizar fuerza bruta contra las credenciales de
cualquier tenant sin restricción. Lo mismo aplica al handshake WebSocket en
`/ws/chat/{tenant_id}` y `/ws/siren/{tenant_id}`.

El enclave usa Argon2id para verificar contraseñas (correcto), pero sin rate
limiting Argon2id solo ralentiza cada intento individual — no impide intentos
en volumen distribuido.

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/auth.rs`
**Archivo:** `kernel/crates/ank-http/src/state.rs` (o donde viva `AppState`)

### Opción A — `tower_governor` (recomendada)

Agregar la dependencia:
```toml
# kernel/Cargo.toml o ank-http/Cargo.toml
tower_governor = "0.4"
```

Configurar el layer en `ank-http`:
```rust
use tower_governor::{GovernorLayer, GovernorConfigBuilder};

let governor_conf = GovernorConfigBuilder::default()
    .per_second(1)          // 1 request por segundo
    .burst_size(5)          // burst de hasta 5 intentos
    .use_headers()
    .finish()
    .unwrap();

let governor_layer = GovernorLayer { config: Arc::new(governor_conf) };

// Aplicar solo a la ruta de login
let auth_router = Router::new()
    .route("/api/auth/login", post(login_handler))
    .layer(governor_layer);
```

### Opción B — Implementación manual con `DashMap`

Si se prefiere no agregar dependencia:

```rust
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

pub struct RateLimiter {
    // IP -> (intentos, timestamp_primer_intento)
    attempts: DashMap<IpAddr, (AtomicU32, Instant)>,
}

impl RateLimiter {
    pub fn check_and_increment(&self, ip: IpAddr) -> bool {
        // Retorna true si se permite el intento
        // Ventana de 60s, máximo 10 intentos
    }
}
```

Agregar `rate_limiter: Arc<RateLimiter>` al `AppState` e invocar al inicio del
handler de login.

**Usar Opción A si `tower_governor` ya está en el dependency tree, Opción B si se quiere evitar dependencias nuevas.**

## Criterios de aceptación

- [ ] Después de N intentos fallidos desde la misma IP en ventana de 60s, el
  endpoint retorna `429 Too Many Requests`
- [ ] El header `Retry-After` está presente en las respuestas 429
- [ ] Intentos desde IPs distintas no se bloquean mutuamente
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] El flujo de login normal (primer intento, credenciales correctas) no se ve afectado

## Dependencias

Ninguna bloqueante. Implementable de forma independiente.
