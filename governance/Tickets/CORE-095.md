# CORE-095 — Retry con Exponential Backoff en CloudProxyDriver

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Resiliencia  
**Estado:** TODO  
**Origen:** REC-007 / big-pickle DEBT-007

---

## Contexto

`CloudProxyDriver` falla inmediatamente ante cualquier error HTTP del proveedor LLM
(429 Rate Limit, 502 Bad Gateway, 503 Service Unavailable). Errores transitorios de
red o momentos de alta carga del proveedor resultan en error visible para el usuario
cuando un reintento habría tenido éxito.

---

## Cambios requeridos

1. Implementar retry con exponential backoff en `generate_stream()`:
   - Máximo 3 intentos totales (1 intento original + 2 reintentos)
   - Backoff: 1s, 2s (o leer el header `Retry-After` si está presente en el 429)
   - Solo reintentar en errores HTTP: 429, 502, 503, 504
   - No reintentar en: 400, 401, 403, 404 (errores de cliente — reintentar no ayuda)

2. Estructura sugerida:

   ```rust
   const MAX_RETRIES: u32 = 2;
   const RETRY_CODES: &[u16] = &[429, 502, 503, 504];

   let mut attempt = 0;
   loop {
       match self.try_generate_stream(&prompt).await {
           Ok(stream) => return Ok(stream),
           Err(e) if attempt < MAX_RETRIES && is_retryable(&e) => {
               let delay = Duration::from_secs(1 << attempt);
               tracing::warn!("CloudDriver: attempt {} failed, retrying in {:?}", attempt + 1, delay);
               tokio::time::sleep(delay).await;
               attempt += 1;
           }
           Err(e) => return Err(e),
       }
   }
   ```

3. Loguear cada reintento con `tracing::warn!` incluyendo intento número, código HTTP
   y delay. No loguear el prompt ni la API key.

4. Si los 3 intentos fallan, retornar el error original con contexto añadido via
   `anyhow::Context`.

---

## Criterios de aceptación

- [ ] `CloudProxyDriver` reintenta hasta 2 veces ante errores 429/502/503/504
- [ ] No reintenta ante errores 4xx que no sean 429
- [ ] Cada reintento tiene un delay de backoff (1s, 2s)
- [ ] Los reintentos se loguean con `tracing::warn!`
- [ ] `cargo build -p ank-core` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

- CORE-093 (reuse de client) — recomendado aplicar antes, pero no bloqueante.
