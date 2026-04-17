# CORE-112 — Reuse de `reqwest::Client` en `CloudProxyDriver`

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🟠 MEDIA-ALTA — Performance
**Estado:** TODO
**Origen:** REC-005 / Auditoría multi-modelo 2026-04-16

---

## Contexto

`CognitiveHAL::execute_with_decision` (o el código que instancia el driver)
crea un nuevo `CloudProxyDriver` por cada request de inferencia. Internamente,
`CloudProxyDriver` crea un `reqwest::Client` nuevo en cada instancia.

`reqwest::Client` mantiene un connection pool interno (TCP keep-alive, TLS
sessions). Al crear una instancia nueva por request, ese pool se destruye
inmediatamente después de cada inferencia, obligando a negociar una nueva
conexión TCP + TLS handshake en cada llamada a la API del proveedor LLM.

En una sesión de chat con múltiples mensajes, esto produce overhead acumulado
medible, especialmente en la primera inferencia de cada mensaje.

**Archivo afectado:** `kernel/crates/ank-core/src/chal/mod.rs` (línea ~239 según auditoría)

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`
**Archivo:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

### Cambio 1 — `CloudProxyDriver` recibe `Arc<reqwest::Client>`

```rust
// Antes
pub struct CloudProxyDriver {
    client: reqwest::Client,  // Nuevo en cada instancia
    api_url: String,
    api_key: String,
}

// Después
pub struct CloudProxyDriver {
    client: Arc<reqwest::Client>,  // Compartido
    api_url: String,
    api_key: String,
}
```

### Cambio 2 — `CognitiveHAL` mantiene el cliente compartido

```rust
pub struct CognitiveHAL {
    // ... campos existentes ...
    http_client: Arc<reqwest::Client>,
}

impl CognitiveHAL {
    pub fn new(/* ... */) -> Self {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .pool_max_idle_per_host(10)
                .build()
                .expect("reqwest::Client construction is infallible with valid config")
        );
        Self { /* ... */ http_client }
    }
}
```

### Cambio 3 — Pasar `Arc<reqwest::Client>` al construir el driver

```rust
// En execute_with_decision o donde se instancie el driver
let driver = CloudProxyDriver::new(
    Arc::clone(&self.http_client),
    api_url,
    api_key,
);
```

## Criterios de aceptación

- [ ] `CloudProxyDriver` usa `Arc<reqwest::Client>` en lugar de instancia propia
- [ ] `CognitiveHAL` inicializa el cliente HTTP una sola vez en `new()`
- [ ] El cliente se comparte entre requests concurrentes sin mutex (Arc es suficiente
  ya que `reqwest::Client` es `Clone + Send + Sync`)
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] No regresión en los tests del HAL/drivers

## Dependencias

Ninguna. Refactor interno del crate `ank-core`.
