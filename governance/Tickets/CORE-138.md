# CORE-138 — Feature: OAuth Token Receiver — almacena tokens de la app en TenantDB

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-core`, `ank-http`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer
**Depende de:** CORE-142 (SystemConfig en MasterEnclave)

---

## Arquitectura — servidor como receptor, no como OAuth client

El servidor **no inicia flujos OAuth**. La app mobile es el único cliente OAuth.
El servidor recibe tokens ya obtenidos por la app y los almacena cifrados.

```
App mobile → POST /api/oauth/tokens → Servidor
    body: { provider, access_token, refresh_token, expires_in, scope }

Servidor → TenantDB::set_oauth_token() → SQLCipher del tenant
```

---

## ADR-041 (revisado)

> OAuth tokens en `kv_store` del enclave SQLCipher del tenant.
> Claves: `oauth_{provider}_{access_token|refresh_token|expiry|scope}`.
> El servidor refresca access tokens expirados automáticamente usando el refresh_token.
> Para refrescar: Google y Spotify aceptan refresh sin Client Secret cuando se usó PKCE.
> El servidor no tiene Client IDs propios — el refresh solo necesita el refresh_token.

---

## Cambios requeridos

### 1. `ank-core` — Métodos OAuth en `TenantDB`

Agregar en `kernel/crates/ank-core/src/enclave/mod.rs`:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

impl TenantDB {
    pub fn set_oauth_token(
        &self,
        provider: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in_secs: u64,
        scope: &str,
    ) -> Result<()> {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .saturating_add(expires_in_secs);

        self.set_kv(&format!("oauth_{}_access_token", provider), access_token)?;
        self.set_kv(&format!("oauth_{}_expiry", provider), &expiry.to_string())?;
        self.set_kv(&format!("oauth_{}_scope", provider), scope)?;
        if let Some(rt) = refresh_token {
            self.set_kv(&format!("oauth_{}_refresh_token", provider), rt)?;
        }
        Ok(())
    }

    /// Retorna el access token si está vigente (con buffer de 60s).
    pub fn get_valid_access_token(&self, provider: &str) -> Result<Option<String>> {
        let token  = self.get_kv(&format!("oauth_{}_access_token", provider))?;
        let expiry = self.get_kv(&format!("oauth_{}_expiry", provider))?;
        match (token, expiry) {
            (Some(t), Some(exp)) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)?
                    .as_secs();
                let exp_secs: u64 = exp.parse().unwrap_or(0);
                if now + 60 < exp_secs { Ok(Some(t)) } else { Ok(None) }
            }
            _ => Ok(None),
        }
    }

    pub fn get_refresh_token(&self, provider: &str) -> Result<Option<String>> {
        self.get_kv(&format!("oauth_{}_refresh_token", provider))
    }

    pub fn get_oauth_scope(&self, provider: &str) -> Result<Option<String>> {
        self.get_kv(&format!("oauth_{}_scope", provider))
    }

    pub fn is_oauth_connected(&self, provider: &str) -> Result<bool> {
        Ok(self.get_refresh_token(provider)?.is_some())
    }

    pub fn revoke_oauth(&self, provider: &str) -> Result<()> {
        for suffix in &["access_token","refresh_token","expiry","scope","email"] {
            let _ = self.connection.execute(
                "DELETE FROM kv_store WHERE key = ?1",
                [&format!("oauth_{}_{}", provider, suffix)],
            );
        }
        Ok(())
    }
}
```

### 2. `ank-core` — `oauth/mod.rs` — refresh automático

Crear `kernel/crates/ank-core/src/oauth/mod.rs`:

```rust
use anyhow::Result;
use std::sync::Arc;

/// Obtiene un access token válido: del caché o refrescado.
/// El refresh con PKCE no necesita Client Secret ni Client ID del servidor.
pub async fn get_or_refresh_token(
    http_client: &Arc<reqwest::Client>,
    db: &crate::enclave::TenantDB,
    provider: &str,
) -> Result<String> {
    // Token vigente en caché
    if let Some(token) = db.get_valid_access_token(provider)? {
        return Ok(token);
    }

    // Refrescar con refresh_token
    let refresh_token = db.get_refresh_token(provider)?
        .ok_or_else(|| anyhow::anyhow!(
            "Provider '{}' not connected. Tell the user to connect their account \
             from the Aegis app (Settings → Cuentas).", provider
        ))?;

    let token_url = match provider {
        "google"  => "https://oauth2.googleapis.com/token",
        "spotify" => "https://accounts.spotify.com/api/token",
        other => anyhow::bail!("Unknown OAuth provider: {}", other),
    };

    // PKCE refresh — solo necesita grant_type y refresh_token
    // (no requiere client_secret cuando la autorización original usó PKCE)
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.as_str()),
    ];

    let resp: serde_json::Value = http_client
        .post(token_url)
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    let new_token = resp["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response: {}", resp))?
        .to_string();
    let expires_in = resp["expires_in"].as_u64().unwrap_or(3600);

    db.set_oauth_token(provider, &new_token, None, expires_in, "")?;

    Ok(new_token)
}
```

Exponer en `ank-core/src/lib.rs`: `pub mod oauth;`

### 3. `ank-http` — Endpoints OAuth

Crear `kernel/crates/ank-http/src/routes/oauth_api.rs`:

#### `POST /api/oauth/tokens` — recibir tokens de la app
Auth: `CitadelAuthenticated` (tenant)

```rust
#[derive(Deserialize)]
struct OAuthTokensBody {
    provider: String,       // "google" | "spotify"
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: String,
}

async fn receive_tokens(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<OAuthTokensBody>,
) -> Result<Json<Value>, AegisHttpError> {
    // Validar provider
    if !["google", "spotify"].contains(&body.provider.as_str()) {
        return Err(AegisHttpError::BadRequest("Unknown provider".into()));
    }

    // Abrir enclave del tenant con su session key hasheada
    let db = ank_core::enclave::TenantDB::open(
        &auth.tenant_id,
        &auth.session_key_hash,
    ).map_err(|e| AegisHttpError::Internal(e))?;

    db.set_oauth_token(
        &body.provider,
        &body.access_token,
        body.refresh_token.as_deref(),
        body.expires_in,
        &body.scope,
    ).map_err(|e| AegisHttpError::Internal(e))?;

    tracing::info!(
        tenant = %auth.tenant_id,
        provider = %body.provider,
        "OAuth tokens stored"
    );

    Ok(Json(json!({ "success": true })))
}
```

#### `GET /api/oauth/status` — estado de conexiones
Auth: `CitadelAuthenticated`

```rust
// Retorna:
// { "google": { "connected": true, "scope": "..." }, "spotify": { "connected": false } }
```

#### `DELETE /api/oauth/{provider}` — desconectar
Auth: `CitadelAuthenticated`

```rust
// db.revoke_oauth(provider)?;
// Ok(Json(json!({ "success": true })))
```

Registrar en `routes/mod.rs`:
```rust
pub mod oauth_api;
.nest("/api/oauth", oauth_api::router())
```

---

## Nota sobre el email del usuario

Para mostrar el email de la cuenta conectada en la UI, la app puede obtenerlo
del token de Google antes de enviarlo al servidor:

```typescript
// En oauthService.ts, después de obtener el access_token de Google:
const userInfo = await fetch('https://www.googleapis.com/oauth2/v3/userinfo', {
  headers: { Authorization: `Bearer ${tokens.accessToken}` }
}).then(r => r.json());
// Incluir email en el POST /api/oauth/tokens:
body: { ...tokens, email: userInfo.email }
```

El servidor lo guarda como `oauth_google_email` en el kv_store.

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] `TenantDB::set_oauth_token` guarda tokens y `get_valid_access_token` los retorna
- [ ] Token expirado: `get_valid_access_token` retorna `None`, `get_or_refresh_token` lo refresca
- [ ] `POST /api/oauth/tokens` almacena en el enclave del tenant autenticado
- [ ] `GET /api/oauth/status` refleja el estado real del enclave
- [ ] `DELETE /api/oauth/google` elimina todos los tokens de Google
- [ ] Sin refresh_token: `get_or_refresh_token` retorna error con mensaje al usuario

---

## Dependencias

- CORE-142 (SystemConfig en MasterEnclave — para que `TenantDB` tenga path correcto)

## Tickets que desbloquea

- CORE-140 (Spotify music), CORE-141 (Google integrations), CORE-143 (App OAuth)

---

## Commit message

```
feat(ank-core,ank-http): CORE-138 OAuth token receiver — store tokens from mobile app in SQLCipher
```
