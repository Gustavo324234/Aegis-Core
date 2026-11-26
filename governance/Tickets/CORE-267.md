# CORE-267 — mark_rate_limited al recibir 429 en CloudProxyDriver

**Tipo:** fix  
**Prioridad:** Alta  
**Épica:** EPIC 49 — Cognitive Loop  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/chal/drivers/cloud.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
**Estado:✅ Done

---

## Contexto

Cuando `openrouter/free` devuelve 429, `CloudProxyDriver::send_with_retry` reintenta con backoff pero **nunca notifica al KeyPool**. En el próximo request (del Chat Agent o del supervisor), el router elige la misma key saturada y vuelve a recibir 429.

Evidencia en logs de producción (2026-05-06):
```
WARN Retryable error received attempt=1 status=429 Too Many Requests
WARN Retrying request after error attempt=1 delay_ms=1000 model=openrouter/free
-- segundo request inmediato --
WARN Retryable error received attempt=1 status=429 Too Many Requests
```

El `KeyPool` ya tiene `mark_rate_limited(key_id, until: DateTime<Utc>)` y el router ya excluye keys marcadas en `get_available_key`. Solo falta conectar el 429 del driver con el KeyPool.

---

## Cambios requeridos

### 1. `cloud.rs` — propagar key_id y notificar 429 al KeyPool

El `CloudProxyDriver` necesita acceso al `key_id` de la key que está usando y a un callback para notificar rate limits. La forma más limpia es pasar un `Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>` como campo `on_rate_limited`.

**Agregar campo al struct:**
```rust
pub struct CloudProxyDriver {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    pub key_id: Option<String>,
    client: Arc<Client>,
    /// CORE-267: callback invocado cuando el provider devuelve 429.
    /// Recibe el `until` timestamp para marcar la key como rate-limited en el KeyPool.
    on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>,
}
```

**En `send_with_retry`, cuando status == 429 y es el último intento:**
```rust
if status.as_u16() == 429 {
    if let Some(cb) = &self.on_rate_limited {
        let until = chrono::Utc::now() + chrono::Duration::seconds(60);
        cb(until);
        tracing::warn!(
            model = %self.model_id,
            "CORE-267: 429 recibido — key marcada como rate-limited por 60s"
        );
    }
}
```

Aplicar tanto en el branch de reintentos agotados como cuando `attempt == MAX_RETRIES`.

### 2. `chal/mod.rs` — inyectar callback al construir el driver en `execute_with_decision`

En `execute_with_decision`, el driver se construye así actualmente:
```rust
let driver = CloudProxyDriver::new(
    Arc::clone(&self.http_client),
    decision.api_url.clone(),
    decision.api_key.clone(),
    decision.model_id.clone(),
);
```

Reemplazar por:
```rust
let key_id_for_cb = decision.key_id.clone(); // ver punto 3
let orchestrator_ref = self.agent_orchestrator.read().await.clone();
let on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>> =
    key_id_for_cb.map(|kid| {
        let router_ref = // obtener Arc<CognitiveRouter> — ver punto 3
        Arc::new(move |until: chrono::DateTime<chrono::Utc>| {
            let kid = kid.clone();
            let router = router_ref.clone();
            tokio::spawn(async move {
                router.read().await.mark_key_rate_limited(&kid, until).await;
            });
        }) as Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>
    });

let driver = CloudProxyDriver::new_with_callback(
    Arc::clone(&self.http_client),
    decision.api_url.clone(),
    decision.api_key.clone(),
    decision.model_id.clone(),
    decision.key_id.clone(),
    on_rate_limited,
);
```

Aplicar el mismo patrón en `execute_agent_loop`.

### 3. `router/mod.rs` — exponer `key_id` en `RoutingDecision` y `mark_key_rate_limited`

**Agregar `key_id` a `RoutingDecision`:**
```rust
pub struct RoutingDecision {
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
    pub key_id: Option<String>,          // ← nuevo
    pub fallback_chain: Vec<FallbackDecision>,
}
```

Poblarlo en `decide()` con `primary_key.key_id.clone()`.

**Agregar método al `CognitiveRouter`:**
```rust
pub async fn mark_key_rate_limited(&self, key_id: &str, until: chrono::DateTime<chrono::Utc>) {
    self.key_pool.mark_rate_limited(key_id, until).await;
}
```

---

## Criterios de aceptación

- [x] Al recibir 429, los logs muestran: `CORE-267: 429 recibido — key marcada como rate-limited por 60s`
- [x] El siguiente request del Chat Agent o supervisor elige una key diferente (si hay disponible) o espera hasta que expire el cooldown
- [x] Si solo hay una key free-tier y está rate-limited, el router retorna error descriptivo en lugar de reintentar indefinidamente contra la misma key
- [x] `cargo build --workspace` sin errores

---

## Dependencias

- CORE-261 ✅ (execute_with_decision)
- CORE-262 ✅ (execute_agent_loop)
- CORE-264 ✅ (dispatch post-spawn)
