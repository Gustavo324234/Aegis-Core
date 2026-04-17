# CORE-114 — Retry con exponential backoff en `CloudProxyDriver`

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🟠 MEDIA — Resiliencia
**Estado:** TODO
**Origen:** REC-007 / Auditoría multi-modelo 2026-04-16

---

## Contexto

`CloudProxyDriver::generate_stream` hace una única petición HTTP al proveedor
LLM. Si el proveedor retorna un error transitorio (HTTP 429 rate limit, 502 Bad
Gateway, 503 Service Unavailable), el driver falla inmediatamente y la tarea
del usuario queda en estado de error.

Los errores 429 y 5xx son comunes en APIs de LLMs bajo carga. La mayoría son
transitorios y se resuelven con un simple retry después de unos segundos.

**Archivo afectado:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

### Implementación de retry con exponential backoff

```rust
const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 500;

async fn send_with_retry(
    &self,
    request_builder: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    let mut attempt = 0;

    loop {
        // reqwest::RequestBuilder no es Clone, necesitamos reconstruir o clonar antes del loop
        let response = self.client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .timeout(Duration::from_secs(30))
            .json(&payload)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) if resp.status() == 429 || resp.status().is_server_error() => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    return Err(anyhow::anyhow!(
                        "API returned {} after {} retries",
                        resp.status(),
                        MAX_RETRIES
                    ));
                }
                let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1); // 500ms, 1s, 2s
                warn!(
                    "Provider returned {}, retry {}/{} in {}ms",
                    resp.status(), attempt, MAX_RETRIES, delay
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
                // Continuar loop
            }
            Ok(resp) => {
                // Error no retriable (400, 401, 403, etc.)
                return Err(anyhow::anyhow!("API returned non-retriable status: {}", resp.status()));
            }
            Err(e) if e.is_timeout() || e.is_connect() => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    return Err(e.into());
                }
                let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                warn!("Network error ({}), retry {}/{} in {}ms", e, attempt, MAX_RETRIES, delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### Consideración: `RequestBuilder` no es `Clone`

Para poder reenviar el request en cada intento, las opciones son:
1. Construir el `RequestBody` (struct serializable) antes del loop y pasarlo al builder en cada iteración.
2. Usar `.try_clone()` en el `Request` construido (disponible si el body no es streaming).

**Preferir opción 1** — construir el payload una vez y usarlo en cada intento.

### No aplicar retry a errores de autenticación

Los errores 401/403 no son transitorios y no deben reintentarse. El código
anterior ya los maneja como "non-retriable".

## Criterios de aceptación

- [ ] Un error HTTP 429 o 5xx produce hasta 3 reintentos con backoff de 500ms/1s/2s
- [ ] Errores 400/401/403 fallan inmediatamente sin reintentos
- [ ] Errores de red (timeout, connection refused) también se reintentan
- [ ] El número de reintentos y el delay base son constantes configurables (no magic numbers dispersos)
- [ ] Los reintentos quedan registrados en el log con nivel `warn` con número de intento
- [ ] `cargo build` pasa sin errores ni warnings de clippy
- [ ] No regresión en tests del driver

## Dependencias

Se puede implementar junto con CORE-112 (ambos tocan `cloud.rs`). No depende de CORE-113.
