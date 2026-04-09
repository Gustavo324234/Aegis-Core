# CORE-011 — ank-http: CitadelLayer + hash_passphrase

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/citadel.rs`
**Asignado a:** Kernel Engineer
**Prioridad:** 🔴 Crítica — bloquea todos los endpoints autenticados
**Estado:** DONE
**Completado el:** 2026-04-09
**Depende de:** CORE-010

---

## Contexto

El Protocolo Citadel usa `tenant_id` + `session_key` (SHA-256 del passphrase).
En gRPC se aplica vía interceptor. En HTTP se aplica vía Axum extractors.

El BFF Python hace `hashlib.sha256(text.encode()).hexdigest()` en cada endpoint
antes de pasar la key al kernel. Este ticket reimplementa eso en Rust.

---

## Trabajo requerido

### `src/citadel.rs` completo

```rust
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};

// ── Hash ─────────────────────────────────────────────────────────────────────

/// SHA-256 hex digest — idéntico a Python's hashlib.sha256(text.encode()).hexdigest()
pub fn hash_passphrase(plaintext: &str) -> String {
    let mut h = Sha256::new();
    h.update(plaintext.as_bytes());
    hex::encode(h.finalize())
}

// ── Extractor: solo headers, sin validar contra enclave ──────────────────────

#[derive(Debug, Clone)]
pub struct CitadelCredentials {
    pub tenant_id: String,
    pub session_key_hash: String,
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for CitadelCredentials {
    type Rejection = CitadelError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let tenant_id = header_str(parts, "x-citadel-tenant")?;
        let raw_key   = header_str(parts, "x-citadel-key")?;
        // La UI manda la key en texto plano; el BFF Python la hashea antes de
        // pasarla al kernel. Hacemos lo mismo aquí.
        Ok(CitadelCredentials {
            tenant_id,
            session_key_hash: hash_passphrase(&raw_key),
        })
    }
}

// ── Extractor: valida contra el enclave Citadel ───────────────────────────────

pub struct CitadelAuthenticated {
    pub tenant_id: String,
    pub session_key_hash: String,
}

#[axum::async_trait]
impl FromRequestParts<crate::state::AppState> for CitadelAuthenticated {
    type Rejection = CitadelError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::state::AppState,
    ) -> Result<Self, Self::Rejection> {
        let creds = CitadelCredentials::from_request_parts(parts, state).await?;
        {
            let citadel = state.citadel.lock().await;
            citadel
                .enclave
                .authenticate_tenant(&creds.tenant_id, &creds.session_key_hash)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("PASSWORD_MUST_CHANGE") {
                        CitadelError::PasswordMustChange
                    } else {
                        CitadelError::Unauthorized
                    }
                })?;
        }
        Ok(CitadelAuthenticated {
            tenant_id: creds.tenant_id,
            session_key_hash: creds.session_key_hash,
        })
    }
}

// ── Errores ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum CitadelError {
    #[error("Missing x-citadel-tenant header")]
    MissingTenant,
    #[error("Missing x-citadel-key header")]
    MissingKey,
    #[error("Citadel Protocol: Access Denied")]
    Unauthorized,
    #[error("Password rotation required")]
    PasswordMustChange,
}

impl IntoResponse for CitadelError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            CitadelError::PasswordMustChange => StatusCode::FORBIDDEN,
            _ => StatusCode::UNAUTHORIZED,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn header_str(parts: &Parts, name: &str) -> Result<String, CitadelError> {
    parts
        .headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or(if name.contains("tenant") {
            CitadelError::MissingTenant
        } else {
            CitadelError::MissingKey
        })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_python_bff() {
        // Python: hashlib.sha256("test_password".encode()).hexdigest()
        assert_eq!(
            hash_passphrase("test_password"),
            "0b14d501a594442a01c6859541b2d233faf05d43d09dce5629ad1976f5dc4af4"
        );
    }

    #[test]
    fn hash_empty_string() {
        // Python: hashlib.sha256("".encode()).hexdigest()
        assert_eq!(
            hash_passphrase(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
```

---

## Criterios de aceptación

- [ ] `CitadelCredentials` extrae `x-citadel-tenant` y hashea `x-citadel-key`
- [ ] `CitadelAuthenticated` llama a `authenticate_tenant()` y rechaza con 401/403
- [ ] `hash_passphrase("test_password")` produce el hash correcto (test incluido)
- [ ] `hash_passphrase("")` produce el hash correcto (test incluido)
- [ ] `CitadelError` retorna JSON `{ "error": "..." }` con el status HTTP correcto
- [ ] `cargo test -p ank-http -- citadel` → pasa
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings
