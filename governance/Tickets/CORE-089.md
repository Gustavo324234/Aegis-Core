# CORE-089 — Fix: `providers.rs` — endpoint `/api/providers/models` sin autenticación

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA
**Estado:** TODO

---

## Contexto

El endpoint `POST /api/providers/models` en `providers.rs` recibe una
API key de terceros y la usa para consultar el catálogo de modelos del
provider. No tiene ningún mecanismo de autenticación:

```rust
async fn list_provider_models(
    State(_state): State<AppState>,  // ← _state: no se verifica nada
    Json(req): Json<ProviderModelsRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    // Recibe provider + api_key + api_url sin verificar quién llama
```

**Impacto:**
1. Cualquier request sin autenticar puede usar el servidor Aegis como **proxy
   para consultar APIs externas** con keys ajenas.
2. Si un atacante conoce la URL del servidor, puede hacer que Aegis consuma
   cuota de keys de terceros.
3. El endpoint hace requests HTTP salientes desde el servidor — vector de
   SSRF si `api_url` no está validada.

**Nota sobre SSRF:** El campo `api_url` en `ProviderModelsRequest` se usa
para construir la URL de `GET /v1/models`:

```rust
let models_url = format!("{}/v1/models", base_url);
client.get(&models_url).send().await
```

No hay validación de que `api_url` apunte a un host legítimo.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/providers.rs`

### 1. Agregar autenticación Citadel

```rust
async fn list_provider_models(
    State(_state): State<AppState>,
    _auth: CitadelAuthenticated,  // ← cualquier tenant autenticado puede llamarlo
    Json(req): Json<ProviderModelsRequest>,
) -> Result<Json<Value>, AegisHttpError> {
    // ...
}
```

### 2. Validar `api_url` contra lista de dominios permitidos

```rust
const ALLOWED_API_HOSTS: &[&str] = &[
    "api.openai.com",
    "api.anthropic.com",
    "api.groq.com",
    "openrouter.ai",
    "generativelanguage.googleapis.com",
    "api.together.xyz",
    "localhost",
    "127.0.0.1",
];

fn validate_api_url(url: &str) -> Result<(), AegisHttpError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| AegisHttpError::BadRequest("Invalid api_url".into()))?;
    let host = parsed.host_str().unwrap_or("");
    if ALLOWED_API_HOSTS.iter().any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed))) {
        Ok(())
    } else {
        Err(AegisHttpError::BadRequest(format!("api_url host '{}' is not in the allowlist", host)))
    }
}
```

Llamar a `validate_api_url(&req.api_url)?` antes de hacer cualquier request
HTTP saliente.

### 3. Agregar `url` a las dependencias si no está ya

```toml
# En ank-http/Cargo.toml si no existe:
url = "2.5"
```

---

## Criterios de aceptación

- [ ] `list_provider_models` requiere `CitadelAuthenticated`
- [ ] `api_url` se valida contra una allowlist de hosts antes de hacer requests
- [ ] Un `api_url` con host arbitrario (`http://internal-server/`) retorna 400
- [ ] `cargo build` pasa sin errores

---

## Dependencias

Ninguna bloqueante. Puede implementarse en paralelo con otros tickets.
