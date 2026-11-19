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

pub struct CitadelAuthenticated {
    pub tenant_id: String,
    pub session_key_hash: String,
    pub is_master: bool,
}

#[axum::async_trait]
impl FromRequestParts<crate::state::AppState> for CitadelAuthenticated {
    type Rejection = CitadelError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::state::AppState,
    ) -> Result<Self, Self::Rejection> {
        let creds = CitadelCredentials::from_request_parts(parts, state).await?;
        let is_master = {
            let citadel = state.citadel.lock().await;
            let master_ok = citadel
                .enclave
                .authenticate_master(&creds.tenant_id, &creds.session_key_hash)
                .await
                .unwrap_or(false);
            if master_ok {
                true
            } else {
                let tenant_ok = citadel
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
                if !tenant_ok {
                    return Err(CitadelError::Unauthorized);
                }
                false
            }
        };
        Ok(CitadelAuthenticated {
            tenant_id: creds.tenant_id,
            session_key_hash: creds.session_key_hash,
            is_master,
        })
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

/// CORE-368: Extracts `session-key.<key>` from `Sec-WebSocket-Protocol` (returning both the
/// plaintext session key and the exact subprotocol token to echo in `101 Switching Protocols`),
/// falling back to `x-citadel-key` for non-browser WebSocket clients.
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

    let fallback_key = headers
        .get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    (fallback_key, None)
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
    if enclave
        .authenticate_master(tenant_id, &key_hash)
        .await
        .unwrap_or(false)
    {
        return Ok(true);
    }

    match enclave.authenticate_tenant(tenant_id, &key_hash).await {
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

/// CORE-368: Validates a plaintext session key for `tenant_id` against the Citadel Enclave
/// before upgrading a WebSocket connection. Returns `Ok(is_master)` when valid.
pub async fn verify_ws_tenant(
    state: &crate::state::AppState,
    tenant_id: &str,
    raw_session_key: &str,
) -> Result<bool, CitadelError> {
    let citadel = state.citadel.lock().await;
    verify_ws_enclave(&citadel.enclave, tenant_id, raw_session_key).await
}

/// CORE-398: Authenticates a WebSocket upgrade request against `MasterEnclave` BEFORE calling `on_upgrade`.
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

    Ok((session_key, protocol_header))
}

/// CORE-398: Authenticates a WebSocket upgrade request against `AppState` BEFORE calling `on_upgrade`.
pub async fn authenticate_ws_upgrade(
    state: &crate::state::AppState,
    tenant_id: &str,
    headers: &axum::http::HeaderMap,
    ws_label: &str,
) -> Result<(String, Option<String>), axum::response::Response> {
    let citadel = state.citadel.lock().await;
    authenticate_ws_upgrade_with_enclave(&citadel.enclave, tenant_id, headers, ws_label).await
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
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut buf))
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
}
