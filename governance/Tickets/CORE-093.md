# CORE-093 — Reuse de CloudProxyDriver (Arc<reqwest::Client>)

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/chal/mod.rs` + `kernel/crates/ank-core/src/chal/drivers/cloud.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Performance  
**Estado:** TODO  
**Origen:** REC-005 / big-pickle DEBT-003

---

## Contexto

`execute_with_decision` instancia un nuevo `CloudProxyDriver` por cada request de
inferencia. `CloudProxyDriver` contiene internamente un `reqwest::Client`, que a su
vez mantiene un connection pool TCP. Al crear una instancia por request, el pool
se descarta al final de cada inferencia y cada llamada paga el costo de un nuevo
handshake TCP (y TLS si aplica). Bajo carga esto produce overhead acumulado
innecesario.

---

## Cambios requeridos

1. Modificar `CloudProxyDriver` para que el `reqwest::Client` sea un `Arc<reqwest::Client>`
   recibido en el constructor en lugar de creado internamente:

   ```rust
   pub struct CloudProxyDriver {
       client: Arc<reqwest::Client>,
       api_url: String,
       api_key: String,
   }

   impl CloudProxyDriver {
       pub fn new(client: Arc<reqwest::Client>, api_url: String, api_key: String) -> Self {
           Self { client, api_url, api_key }
       }
   }
   ```

2. En `CognitiveHAL` (o en `AppState`), crear el `reqwest::Client` una sola vez
   durante la inicialización y envolverlo en `Arc`. Pasarlo a los drivers al momento
   de construcción.

3. Asegurarse de que el `reqwest::Client` compartido tenga configurado el timeout
   actual (30s) y que los headers por defecto no filtren credenciales entre tenants
   (las API keys deben ir por request, no como default headers del cliente compartido).

---

## Criterios de aceptación

- [ ] `CloudProxyDriver` recibe `Arc<reqwest::Client>` en su constructor
- [ ] El cliente HTTP se inicializa una sola vez en el ciclo de vida del servidor
- [ ] Las API keys se envían por request (header `Authorization` por llamada), no como
      default del cliente compartido
- [ ] `cargo build -p ank-core` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

Ninguna.
