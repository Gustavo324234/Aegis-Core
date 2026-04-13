# CORE-087 — Fix: `siren_api.rs` expone `session_key` en query params (GET endpoints)

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

Los endpoints GET de la API de Siren reciben credenciales en query params:

```rust
#[derive(Deserialize)]
pub struct SirenQuery {
    pub tenant_id: String,
    pub session_key: String,  // ← contraseña en la URL
}

// get_siren_config:
async fn get_siren_config(
    Query(query): Query<SirenQuery>,  // ← ?tenant_id=x&session_key=y
)

// list_siren_voices:
async fn list_siren_voices(
    Query(_query): Query<SirenQuery>,
)
```

Violación al Protocolo Citadel documentado en `CLAUDE.md`:
`Headers HTTP: x-citadel-tenant + x-citadel-key`

Adicionalmente, `set_siren_config` lee credenciales del body JSON:
```rust
pub struct SirenConfigRequest {
    pub tenant_id: String,
    pub session_key: String,  // ← en el body, no en headers
    ...
}
```

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/siren_api.rs`

### 1. Migrar `get_siren_config` a `CitadelAuthenticated`

```rust
async fn get_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,  // ← headers x-citadel-tenant / x-citadel-key
) -> Result<Json<Value>, AegisHttpError> {
    let profile = state.persistence
        .get_voice_profile(&auth.tenant_id)
        .await ...;
    // ...
}
```

### 2. Migrar `set_siren_config` a `CitadelAuthenticated`

```rust
#[derive(Deserialize)]
pub struct SirenConfigBody {
    pub provider: String,
    pub api_key: String,
    pub voice_id: String,
    // Sin tenant_id ni session_key en el body
}

async fn set_siren_config(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(req): Json<SirenConfigBody>,
) -> Result<Json<Value>, AegisHttpError> {
    let profile = VoiceProfile {
        tenant_id: auth.tenant_id.clone(),
        // ...
    };
    // ...
}
```

### 3. Migrar `list_siren_voices` — sin auth requerida o con CitadelAuthenticated

`list_siren_voices` actualmente ignora el query (`_query`) — la autenticación
es irrelevante. Simplificar a endpoint público sin auth:

```rust
async fn list_siren_voices() -> Json<Value> {
    Json(json!({ "voices": [...] }))
}
```

---

## Criterios de aceptación

- [ ] Ningún endpoint de `siren_api.rs` acepta `session_key` en query params o body
- [ ] `get_siren_config` y `set_siren_config` usan `CitadelAuthenticated`
- [ ] `list_siren_voices` es público (no requiere auth) o usa `CitadelAuthenticated`
- [ ] `cargo build` pasa sin errores

---

## Dependencias

`CitadelAuthenticated` ya está implementado en `citadel.rs` — listo para usar.
