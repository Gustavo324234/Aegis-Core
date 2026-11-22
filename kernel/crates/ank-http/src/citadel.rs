use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub const CITADEL_TOKEN_PREFIX: &str = "cst_";
pub const DEFAULT_SESSION_TTL_SECS: u64 = 12 * 3600;

fn configured_session_ttl_secs() -> u64 {
    std::env::var("AEGIS_SESSION_TTL_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|s| s.clamp(60, 7 * 86_400))
        .unwrap_or(DEFAULT_SESSION_TTL_SECS)
}

#[derive(Debug, Clone)]
struct CitadelSession {
    tenant_id: String,
    session_key_hash: String,
    is_master: bool,
    expires_at: Instant,
}

/// CORE-402: Store en memoria de tokens efímeros de sesión Citadel (`cst_*`) con
/// TTL y revocación explícita para evitar re-ejecutar Argon2id en cada request.
#[derive(Debug, Clone, Default)]
pub struct CitadelSessionStore {
    sessions: Arc<RwLock<HashMap<String, CitadelSession>>>,
}

impl CitadelSessionStore {
    pub fn issue(&self, tenant_id: &str, session_key_hash: &str, is_master: bool) -> (String, u64) {
        let ttl_secs = configured_session_ttl_secs();
        self.issue_with_ttl(tenant_id, session_key_hash, is_master, ttl_secs)
    }

    pub fn issue_with_ttl(
        &self,
        tenant_id: &str,
        session_key_hash: &str,
        is_master: bool,
        ttl_secs: u64,
    ) -> (String, u64) {
        let token = format!(
            "{CITADEL_TOKEN_PREFIX}{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let ttl = ttl_secs.max(1);
        let session = CitadelSession {
            tenant_id: tenant_id.to_string(),
            session_key_hash: session_key_hash.to_string(),
            is_master,
            expires_at: Instant::now() + Duration::from_secs(ttl),
        };
        if let Ok(mut guard) = self.sessions.write() {
            let now = Instant::now();
            guard.retain(|_, s| s.expires_at > now);
            guard.insert(token.clone(), session);
        }
        (token, ttl)
    }

    pub fn resolve(
        &self,
        token: &str,
        expected_tenant: Option<&str>,
    ) -> Result<CitadelAuthenticated, CitadelError> {
        let trimmed = token.trim();
        if !trimmed.starts_with(CITADEL_TOKEN_PREFIX) {
            return Err(CitadelError::Unauthorized);
        }

        let now = Instant::now();
        let mut expired = false;
        let resolved = if let Ok(guard) = self.sessions.read() {
            match guard.get(trimmed) {
                Some(sess) if sess.expires_at > now => {
                    if let Some(expected) = expected_tenant.map(str::trim).filter(|s| !s.is_empty())
                    {
                        if sess.tenant_id != expected {
                            return Err(CitadelError::Unauthorized);
                        }
                    }
                    Some(CitadelAuthenticated {
                        tenant_id: sess.tenant_id.clone(),
                        session_key_hash: sess.session_key_hash.clone(),
                        is_master: sess.is_master,
                    })
                }
                Some(_) => {
                    expired = true;
                    None
                }
                None => None,
            }
        } else {
            None
        };

        if expired {
            if let Ok(mut guard) = self.sessions.write() {
                if let Some(sess) = guard.get(trimmed) {
                    if sess.expires_at <= now {
                        guard.remove(trimmed);
                    }
                }
            }
        }

        resolved.ok_or(CitadelError::Unauthorized)
    }

    pub fn revoke_token(&self, token: &str) -> bool {
        let trimmed = token.trim();
        if let Ok(mut guard) = self.sessions.write() {
            guard.remove(trimmed).is_some()
        } else {
            false
        }
    }

    pub fn revoke_tenant(&self, tenant_id: &str) -> usize {
        if let Ok(mut guard) = self.sessions.write() {
            let before = guard.len();
            guard.retain(|_, sess| sess.tenant_id != tenant_id);
            before.saturating_sub(guard.len())
        } else {
            0
        }
    }
}

/// Extrae un token efímero de sesión (`cst_*`) desde `Authorization: Bearer`,
/// `x-citadel-token` o `x-citadel-key`.
pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
    {
        if let Some(bearer) = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
        {
            let tok = bearer.trim();
            if tok.starts_with(CITADEL_TOKEN_PREFIX) {
                return Some(tok.to_string());
            }
        }
    }

    for hdr in ["x-citadel-token", "x-citadel-key"] {
        if let Some(val) = headers.get(hdr).and_then(|v| v.to_str().ok()) {
            let trimmed = val.trim();
            if trimmed.starts_with(CITADEL_TOKEN_PREFIX) {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

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
        let raw_key = header_str(parts, "x-citadel-key")?;
        // La UI manda la key en texto plano; el BFF Python la hashea antes de
        // pasarla al kernel. Hacemos lo mismo aquí.
        Ok(CitadelCredentials {
            tenant_id,
            session_key_hash: hash_passphrase(&raw_key),
        })
    }
}

// ── Extractor: valida contra el enclave Citadel ───────────────────────────────

#[derive(Debug, Clone)]
pub struct CitadelAuthenticated {
    pub tenant_id: String,
    pub session_key_hash: String,
    pub is_master: bool,
}

/// Autentica cabeceras HTTP usando primero un token efímero `cst_*` (O(1), sin Argon2id
/// ni lock de `citadel`) y cayendo en la passphrase legacy clonando el `MasterEnclave`.
pub async fn authenticate_headers(
    state: &crate::state::AppState,
    headers: &HeaderMap,
) -> Result<CitadelAuthenticated, CitadelError> {
    let optional_tenant = headers
        .get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if let Some(token) = extract_session_token(headers) {
        return state.session_store.resolve(&token, optional_tenant);
    }

    let tenant_id = optional_tenant
        .map(ToString::to_string)
        .ok_or(CitadelError::MissingTenant)?;
    let raw_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .ok_or(CitadelError::MissingKey)?;

    let session_key_hash = hash_passphrase(&raw_key);
    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };

    let (master_res, tenant_res) = tokio::join!(
        enclave.authenticate_master(&tenant_id, &session_key_hash),
        enclave.authenticate_tenant(&tenant_id, &session_key_hash),
    );

    let is_master = if master_res.unwrap_or(false) {
        true
    } else {
        let tenant_ok = tenant_res.map_err(|e| {
            let msg = e.to_string();
            if msg.contains("PASSWORD_MUST_CHANGE") {
                CitadelError::PasswordMustChange
            } else {
                CitadelError::Unauthorized
            }
        })?;
        if !tenant_ok {
            return Err(CitadelError::Unauthorized);
        }
        false
    };

    Ok(CitadelAuthenticated {
        tenant_id,
        session_key_hash,
        is_master,
    })
}

/// Autentica y verifica rol Master Admin a partir de cabeceras HTTP (`cst_*` o key legacy).
pub async fn authenticate_master_headers(
    state: &crate::state::AppState,
    headers: &HeaderMap,
) -> Result<String, CitadelError> {
    let auth = authenticate_headers(state, headers).await?;
    if auth.is_master {
        Ok(auth.tenant_id)
    } else {
        Err(CitadelError::Unauthorized)
    }
}

#[axum::async_trait]
impl FromRequestParts<crate::state::AppState> for CitadelAuthenticated {
    type Rejection = CitadelError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::state::AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate_headers(state, &parts.headers).await
    }
}

// ── Errores ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, Clone)]
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

/// CORE-368 / CORE-402: Extracts `session-key.<key_or_token>` from `Sec-WebSocket-Protocol`
/// (returning the credential and exact subprotocol token to echo in `101 Switching Protocols`),
/// falling back to `x-citadel-token`, `x-citadel-key`, or `Authorization: Bearer`.
pub fn extract_ws_session_key_and_protocol(
    headers: &axum::http::HeaderMap,
) -> (Option<String>, Option<String>) {
    if let Some(val) = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(proto) = val
            .split(',')
            .map(str::trim)
            .find(|p| p.starts_with("session-key."))
        {
            let key = proto
                .strip_prefix("session-key.")
                .unwrap_or_default()
                .to_string();
            if !key.is_empty() {
                return (Some(key), Some(proto.to_string()));
            }
        }
    }

    for hdr in ["x-citadel-token", "x-citadel-key"] {
        if let Some(val) = headers.get(hdr).and_then(|v| v.to_str().ok()) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return (Some(trimmed.to_string()), None);
            }
        }
    }

    if let Some(auth) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
    {
        if let Some(tok) = auth
            .strip_prefix("Bearer ")
            .or_else(|| auth.strip_prefix("bearer "))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return (Some(tok.to_string()), None);
        }
    }

    (None, None)
}

/// CORE-368: Validates a plaintext session key for `tenant_id` directly against a `MasterEnclave`.
/// Returns `Ok(is_master)` when valid, or `CitadelError::PasswordMustChange` when rotation is required.
pub async fn verify_ws_enclave(
    enclave: &ank_core::enclave::MasterEnclave,
    tenant_id: &str,
    raw_session_key: &str,
) -> Result<bool, CitadelError> {
    if tenant_id.trim().is_empty() {
        return Err(CitadelError::MissingTenant);
    }
    if raw_session_key.trim().is_empty() {
        return Err(CitadelError::MissingKey);
    }

    let key_hash = hash_passphrase(raw_session_key);
    let (master_res, tenant_res) = tokio::join!(
        enclave.authenticate_master(tenant_id, &key_hash),
        enclave.authenticate_tenant(tenant_id, &key_hash),
    );
    if master_res.unwrap_or(false) {
        return Ok(true);
    }

    match tenant_res {
        Ok(true) => Ok(false),
        Ok(false) => Err(CitadelError::Unauthorized),
        Err(e) => {
            if e.to_string().contains("PASSWORD_MUST_CHANGE") {
                Err(CitadelError::PasswordMustChange)
            } else {
                Err(CitadelError::Unauthorized)
            }
        }
    }
}

/// CORE-368 / CORE-402: Validates a plaintext key or `cst_*` session token for `tenant_id`.
/// Returns `Ok(is_master)` when valid.
pub async fn verify_ws_tenant(
    state: &crate::state::AppState,
    tenant_id: &str,
    raw_or_token: &str,
) -> Result<bool, CitadelError> {
    if raw_or_token.trim().starts_with(CITADEL_TOKEN_PREFIX) {
        return state
            .session_store
            .resolve(raw_or_token, Some(tenant_id))
            .map(|a| a.is_master);
    }
    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    verify_ws_enclave(&enclave, tenant_id, raw_or_token).await
}

/// CORE-398: Authenticates a WebSocket upgrade request against `MasterEnclave` BEFORE calling `on_upgrade`.
/// Returns `(session_key_hash, protocol_header)` on success.
pub async fn authenticate_ws_upgrade_with_enclave(
    enclave: &ank_core::enclave::MasterEnclave,
    tenant_id: &str,
    headers: &axum::http::HeaderMap,
    ws_label: &str,
) -> Result<(String, Option<String>), axum::response::Response> {
    let (Some(session_key), protocol_header) = extract_ws_session_key_and_protocol(headers) else {
        tracing::warn!(
            tenant = %tenant_id,
            channel = %ws_label,
            "Rejected unauthenticated WebSocket upgrade (missing key)"
        );
        return Err(CitadelError::MissingKey.into_response());
    };

    if let Err(err) = verify_ws_enclave(enclave, tenant_id, &session_key).await {
        tracing::warn!(
            tenant = %tenant_id,
            channel = %ws_label,
            error = %err,
            "Rejected unauthorized WebSocket upgrade"
        );
        return Err(err.into_response());
    }

    Ok((hash_passphrase(&session_key), protocol_header))
}

/// CORE-398 / CORE-402: Authenticates a WebSocket upgrade request against `AppState` BEFORE calling
/// `on_upgrade`. Uses `CitadelSessionStore` in O(1) when given a `cst_*` token, otherwise falls
/// back to validating against a cloned `MasterEnclave` without holding `state.citadel`.
/// Returns `(session_key_hash, protocol_header)` on success.
pub async fn authenticate_ws_upgrade(
    state: &crate::state::AppState,
    tenant_id: &str,
    headers: &axum::http::HeaderMap,
    ws_label: &str,
) -> Result<(String, Option<String>), axum::response::Response> {
    let (Some(cred), protocol_header) = extract_ws_session_key_and_protocol(headers) else {
        tracing::warn!(
            tenant = %tenant_id,
            channel = %ws_label,
            "Rejected unauthenticated WebSocket upgrade (missing key)"
        );
        return Err(CitadelError::MissingKey.into_response());
    };

    if cred.trim().starts_with(CITADEL_TOKEN_PREFIX) {
        return match state.session_store.resolve(&cred, Some(tenant_id)) {
            Ok(auth) => Ok((auth.session_key_hash, protocol_header)),
            Err(err) => {
                tracing::warn!(
                    tenant = %tenant_id,
                    channel = %ws_label,
                    error = %err,
                    "Rejected unauthorized WebSocket upgrade (session token)"
                );
                Err(err.into_response())
            }
        };
    }

    let enclave = {
        let citadel = state.citadel.lock().await;
        citadel.enclave.clone()
    };
    if let Err(err) = verify_ws_enclave(&enclave, tenant_id, &cred).await {
        tracing::warn!(
            tenant = %tenant_id,
            channel = %ws_label,
            error = %err,
            "Rejected unauthorized WebSocket upgrade"
        );
        return Err(err.into_response());
    }

    Ok((hash_passphrase(&cred), protocol_header))
}

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
            "10a6e6cc8311a3e2bcc09bf6c199adecd5dd59408c343e926b129c4914f3cb01"
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

    #[test]
    fn extract_ws_subprotocol_and_header_fallback() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "sec-websocket-protocol",
            axum::http::HeaderValue::from_static("chat, session-key.my_secret_123"),
        );
        let (key, proto) = extract_ws_session_key_and_protocol(&headers);
        assert_eq!(key.as_deref(), Some("my_secret_123"));
        assert_eq!(proto.as_deref(), Some("session-key.my_secret_123"));

        let mut header_only = axum::http::HeaderMap::new();
        header_only.insert(
            "x-citadel-key",
            axum::http::HeaderValue::from_static("cli_key_456"),
        );
        let (key2, proto2) = extract_ws_session_key_and_protocol(&header_only);
        assert_eq!(key2.as_deref(), Some("cli_key_456"));
        assert!(proto2.is_none());
    }

    #[test]
    fn extract_ws_preserves_embedded_prefix() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "sec-websocket-protocol",
            axum::http::HeaderValue::from_static("session-key.abc.session-key.xyz"),
        );
        let (key, proto) = extract_ws_session_key_and_protocol(&headers);
        assert_eq!(key.as_deref(), Some("abc.session-key.xyz"));
        assert_eq!(proto.as_deref(), Some("session-key.abc.session-key.xyz"));
    }

    async fn raw_ws_upgrade_response(
        addr: std::net::SocketAddr,
        path: &str,
        proto_header: Option<&str>,
    ) -> anyhow::Result<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        let mut req = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
        );
        if let Some(proto) = proto_header {
            req.push_str(&format!("Sec-WebSocket-Protocol: {proto}\r\n"));
        }
        req.push_str("\r\n");
        stream.write_all(req.as_bytes()).await?;
        let mut buf = vec![0u8; 2048];
        let n = tokio::time::timeout(std::time::Duration::from_secs(10), stream.read(&mut buf))
            .await??;
        Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
    }

    #[tokio::test]
    async fn authenticate_ws_upgrade_rejects_missing_invalid_and_password_must_change_before_upgrade(
    ) -> anyhow::Result<()> {
        use ank_core::enclave::MasterEnclave;
        use axum::{
            extract::{ws::WebSocketUpgrade, Path, State},
            http::HeaderMap,
            routing::get,
            Router,
        };
        use std::sync::Arc;

        let enclave = Arc::new(MasterEnclave::new_in_memory().await?);
        let valid_secret = "abc.session-key.xyz";
        enclave
            .create_personal_tenant("alice", &hash_passphrase(valid_secret))
            .await?;
        let (_port, temp_pass) = enclave.create_tenant("bob").await?;

        async fn test_ws_handler(
            ws: WebSocketUpgrade,
            Path(tenant_id): Path<String>,
            State(enclave): State<Arc<MasterEnclave>>,
            headers: HeaderMap,
        ) -> axum::response::Response {
            let (_key, proto) = match authenticate_ws_upgrade_with_enclave(
                &enclave, &tenant_id, &headers, "TestWS",
            )
            .await
            {
                Ok(res) => res,
                Err(resp) => return resp,
            };
            let ws = if let Some(p) = proto {
                ws.protocols([p])
            } else {
                ws
            };
            ws.on_upgrade(|socket| async move {
                let _ = socket.close().await;
            })
        }

        let app = Router::new()
            .route("/ws/:tenant_id", get(test_ws_handler))
            .with_state(enclave);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let missing = raw_ws_upgrade_response(addr, "/ws/alice", None).await?;
        assert!(
            missing.starts_with("HTTP/1.1 401 "),
            "Expected 401 before upgrade, got: {missing}"
        );

        let invalid = raw_ws_upgrade_response(addr, "/ws/alice", Some("session-key.wrong")).await?;
        assert!(
            invalid.starts_with("HTTP/1.1 401 "),
            "Expected 401 on wrong key, got: {invalid}"
        );

        let must_change_proto = format!("session-key.{temp_pass}");
        let must_change =
            raw_ws_upgrade_response(addr, "/ws/bob", Some(&must_change_proto)).await?;
        assert!(
            must_change.starts_with("HTTP/1.1 403 "),
            "Expected 403 PASSWORD_MUST_CHANGE before upgrade, got: {must_change}"
        );

        let ok_proto = format!("session-key.{valid_secret}");
        let upgraded = raw_ws_upgrade_response(addr, "/ws/alice", Some(&ok_proto)).await?;
        assert!(
            upgraded.starts_with("HTTP/1.1 101 "),
            "Expected 101 Switching Protocols on valid embedded prefix key, got: {upgraded}"
        );
        assert!(
            upgraded
                .to_ascii_lowercase()
                .contains("sec-websocket-protocol: session-key.abc.session-key.xyz"),
            "Expected echoed subprotocol in response headers: {upgraded}"
        );

        server.abort();
        Ok(())
    }

    #[tokio::test]
    async fn test_citadel_session_token_lifecycle_ttl_and_concurrency() -> anyhow::Result<()> {
        let store = CitadelSessionStore::default();
        let (tok_alice, ttl) = store.issue("alice", "hash_alice", false);
        assert!(tok_alice.starts_with(CITADEL_TOKEN_PREFIX));
        assert!(ttl >= 60);

        let auth = store.resolve(&tok_alice, Some("alice"))?;
        assert_eq!(auth.tenant_id, "alice");
        assert_eq!(auth.session_key_hash, "hash_alice");
        assert!(!auth.is_master);

        // Tenant mismatch must be rejected even with a valid token.
        assert!(matches!(
            store.resolve(&tok_alice, Some("bob")),
            Err(CitadelError::Unauthorized)
        ));

        // Concurrent O(1) resolution works across many tasks.
        let mut handles = Vec::new();
        for _ in 0..16 {
            let s = store.clone();
            let t = tok_alice.clone();
            handles.push(tokio::spawn(async move { s.resolve(&t, Some("alice")) }));
        }
        for h in handles {
            let resolved = h.await??;
            assert_eq!(resolved.tenant_id, "alice");
        }

        // Single-token revocation.
        assert!(store.revoke_token(&tok_alice));
        assert!(matches!(
            store.resolve(&tok_alice, Some("alice")),
            Err(CitadelError::Unauthorized)
        ));

        // Tenant-wide revocation invalidates multiple active tokens.
        let (t1, _) = store.issue("alice", "hash_alice", false);
        let (t2, _) = store.issue("alice", "hash_alice", false);
        let (tbob, _) = store.issue("bob", "hash_bob", true);
        assert_eq!(store.revoke_tenant("alice"), 2);
        assert!(store.resolve(&t1, Some("alice")).is_err());
        assert!(store.resolve(&t2, Some("alice")).is_err());
        assert!(store.resolve(&tbob, Some("bob"))?.is_master);

        // Header & WS token extraction order.
        let mut hdrs = HeaderMap::new();
        hdrs.insert(
            "authorization",
            axum::http::HeaderValue::from_str(&format!("Bearer {tbob}"))?,
        );
        assert_eq!(extract_session_token(&hdrs).as_deref(), Some(tbob.as_str()));

        let mut ws_hdrs = HeaderMap::new();
        ws_hdrs.insert(
            "sec-websocket-protocol",
            axum::http::HeaderValue::from_str(&format!("session-key.{tbob}"))?,
        );
        let (ws_cred, ws_proto) = extract_ws_session_key_and_protocol(&ws_hdrs);
        assert_eq!(ws_cred.as_deref(), Some(tbob.as_str()));
        assert_eq!(ws_proto, Some(format!("session-key.{tbob}")));

        // TTL expiry evicts session automatically.
        let (short_tok, _) = store.issue_with_ttl("bob", "hash_bob", false, 1);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(matches!(
            store.resolve(&short_tok, Some("bob")),
            Err(CitadelError::Unauthorized)
        ));

        Ok(())
    }
}
