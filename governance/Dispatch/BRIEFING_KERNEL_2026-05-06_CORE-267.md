# BRIEFING — Kernel Engineer
**Fecha:** 2026-05-06  
**Ticket:** CORE-267  
**Prioridad:** Alta  
**Branch:** `fix/core-267-rate-limit-feedback`

---

## Problema

Cuando `openrouter/free` devuelve 429, el driver reintenta con backoff pero nunca notifica al `KeyPool`. El siguiente request vuelve a usar la misma key saturada y recibe otro 429. Esto explica los timeouts en producción: Chat Agent y supervisor se pisan mutuamente en el rate limit.

El `KeyPool` ya tiene `mark_rate_limited(key_id, until)` y el router ya excluye keys marcadas. Solo hay que conectar los dos.

---

## Tres archivos, cambios quirúrgicos

### 1. `kernel/crates/ank-core/src/router/mod.rs`

**a) Agregar `key_id` a `RoutingDecision`:**
```rust
pub struct RoutingDecision {
    pub model_id: String,
    pub provider: String,
    pub api_url: String,
    pub api_key: String,
    pub key_id: Option<String>,   // ← nuevo
    pub fallback_chain: Vec<FallbackDecision>,
}
```

**b) Poblar `key_id` en `decide()` — donde se construye el `Ok(RoutingDecision{...})`:**
```rust
Ok(RoutingDecision {
    model_id: api_model_id,
    provider: primary.provider.clone(),
    api_url: ...,
    api_key: primary_key.api_key.clone(),
    key_id: Some(primary_key.key_id.clone()),   // ← nuevo
    fallback_chain,
})
```

**c) Agregar método helper:**
```rust
pub async fn mark_key_rate_limited(&self, key_id: &str, until: chrono::DateTime<chrono::Utc>) {
    self.key_pool.mark_rate_limited(key_id, until).await;
}
```

---

### 2. `kernel/crates/ank-core/src/chal/drivers/cloud.rs`

**a) Agregar campos al struct:**
```rust
pub struct CloudProxyDriver {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    pub key_id: Option<String>,
    client: Arc<Client>,
    /// CORE-267: callback invocado cuando el provider devuelve 429.
    on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>,
}
```

**b) Agregar constructor con callback:**
```rust
pub fn new_with_callback(
    client: Arc<Client>,
    api_url: String,
    api_key: String,
    model_id: String,
    key_id: Option<String>,
    on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>,
) -> Self {
    Self { client, api_url, api_key, model_id, key_id, on_rate_limited }
}
```

El constructor `new()` existente puede delegar a este con `key_id: None, on_rate_limited: None`.

**c) En `send_with_retry`, cuando `status == 429` después de agotar reintentos:**
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

---

### 3. `kernel/crates/ank-core/src/chal/mod.rs`

El `CognitiveHAL` necesita acceso al `CognitiveRouter` para el callback. Ya tiene `agent_orchestrator` — agregar `router` como campo opcional del mismo estilo:

**a) Agregar campo al struct `CognitiveHAL`:**
```rust
pub router_ref: RwLock<Option<Arc<RwLock<crate::router::CognitiveRouter>>>>,
```

**b) Agregar setter (igual que `set_orchestrator`):**
```rust
pub async fn set_router_ref(&self, router: Arc<RwLock<crate::router::CognitiveRouter>>) {
    let mut r = self.router_ref.write().await;
    *r = Some(router);
}
```

Llamar `set_router_ref` desde `ank-server/src/main.rs` donde ya se llama `set_router`.

**c) En `execute_with_decision`, construir el driver con callback:**
```rust
let on_rate_limited = {
    let router_opt = self.router_ref.read().await.clone();
    let key_id = decision.key_id.clone();
    match (router_opt, key_id) {
        (Some(router), Some(kid)) => {
            Some(Arc::new(move |until: chrono::DateTime<chrono::Utc>| {
                let router = Arc::clone(&router);
                let kid = kid.clone();
                tokio::spawn(async move {
                    router.read().await.mark_key_rate_limited(&kid, until).await;
                });
            }) as Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>)
        }
        _ => None,
    }
};

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

---

## Verificación

```bash
cargo build --workspace
```

Buscar en logs tras el fix:
```
CORE-267: 429 recibido — key marcada como rate-limited por 60s
```

---

## Commit

```
fix(ank-core): CORE-267 mark_rate_limited al recibir 429 en CloudProxyDriver
```

No correr `cargo test`, no pushear. Tavo maneja git.
