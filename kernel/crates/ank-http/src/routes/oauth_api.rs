use crate::citadel::CitadelAuthenticated;
use crate::error::AegisHttpError;
use ank_core::enclave::TenantDB;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Html,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

const OAUTH_FLOW_TTL_SECS: u64 = 600;

#[derive(Debug, Clone)]
pub struct PendingOAuthFlow {
    pub tenant_id: String,
    pub session_key_hash: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub created_at: u64,
}

pub type PendingOAuthStore = std::sync::Arc<dashmap::DashMap<String, PendingOAuthFlow>>;

pub fn new_pending_oauth_store() -> PendingOAuthStore {
    std::sync::Arc::new(dashmap::DashMap::new())
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Deserialize)]
pub struct OAuthTokensBody {
    provider: String,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
}

fn default_expires_in() -> u64 {
    3600
}

#[derive(Serialize)]
pub struct ProviderStatus {
    connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    client_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    has_client_secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_uri: Option<String>,
}

#[derive(Serialize)]
pub struct OAuthStatusResponse {
    google: ProviderStatus,
    spotify: ProviderStatus,
}

#[derive(Deserialize)]
pub struct GoogleClientConfigBody {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    clear: bool,
}

#[derive(Deserialize, Default)]
pub struct GoogleStartBody {
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    include_extra_scopes: bool,
}

#[derive(Deserialize)]
pub struct DisconnectQuery {
    #[serde(default)]
    clear_client: bool,
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub async fn receive_tokens(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<OAuthTokensBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    if !["google", "spotify"].contains(&body.provider.as_str()) {
        return Err(AegisHttpError::BadRequest("Unknown provider".into()));
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(AegisHttpError::Internal)?;

    if let Some(cid) = body.client_id.as_deref().filter(|s| !s.trim().is_empty()) {
        db.set_oauth_client_config(
            &body.provider,
            cid,
            body.client_secret.as_deref(),
            body.redirect_uri.as_deref(),
        )
        .map_err(AegisHttpError::Internal)?;
    }

    db.set_oauth_token(
        &body.provider,
        &body.access_token,
        body.refresh_token.as_deref(),
        body.expires_in,
        &body.scope,
    )
    .map_err(AegisHttpError::Internal)?;

    if let Some(email) = body.email.as_deref().filter(|s| !s.trim().is_empty()) {
        db.set_oauth_email(&body.provider, email.trim())
            .map_err(AegisHttpError::Internal)?;
    }

    tracing::info!(
        tenant = %auth.tenant_id,
        provider = %body.provider,
        "OAuth tokens stored"
    );

    Ok(Json(json!({ "success": true })))
}

pub async fn get_status(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
) -> Result<Json<OAuthStatusResponse>, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(AegisHttpError::Internal)?;

    let google_connected = db
        .is_oauth_connected("google")
        .map_err(AegisHttpError::Internal)?;
    let google_scope = db
        .get_oauth_scope("google")
        .map_err(AegisHttpError::Internal)?;
    let google_email = db
        .get_oauth_email("google")
        .map_err(AegisHttpError::Internal)?;
    let google_creds = ank_core::oauth::resolve_client_credentials(&db, "google");

    let spotify_connected = db
        .is_oauth_connected("spotify")
        .map_err(AegisHttpError::Internal)?;
    let spotify_scope = db
        .get_oauth_scope("spotify")
        .map_err(AegisHttpError::Internal)?;
    let spotify_email = db
        .get_oauth_email("spotify")
        .map_err(AegisHttpError::Internal)?;
    let spotify_creds = ank_core::oauth::resolve_client_credentials(&db, "spotify");

    Ok(Json(OAuthStatusResponse {
        google: ProviderStatus {
            connected: google_connected,
            scope: google_scope,
            email: google_email,
            client_configured: google_creds.client_id.is_some(),
            client_id: google_creds.client_id,
            has_client_secret: google_creds.client_secret.is_some(),
            redirect_uri: google_creds.redirect_uri,
        },
        spotify: ProviderStatus {
            connected: spotify_connected,
            scope: spotify_scope,
            email: spotify_email,
            client_configured: spotify_creds.client_id.is_some(),
            client_id: spotify_creds.client_id,
            has_client_secret: spotify_creds.client_secret.is_some(),
            redirect_uri: spotify_creds.redirect_uri,
        },
    }))
}

pub async fn save_google_config(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    Json(body): Json<GoogleClientConfigBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(AegisHttpError::Internal)?;

    if body.clear {
        db.clear_oauth_client_config("google")
            .map_err(AegisHttpError::Internal)?;
        return Ok(Json(json!({ "success": true, "cleared": true })));
    }

    let trimmed_id = body.client_id.trim();
    if trimmed_id.is_empty() {
        return Err(AegisHttpError::BadRequest(
            "El Client ID de Google es obligatorio".into(),
        ));
    }

    if let Some(ref uri) = body.redirect_uri {
        let u = uri.trim();
        if !u.is_empty() && !u.starts_with("http://") && !u.starts_with("https://") {
            return Err(AegisHttpError::BadRequest(
                "La Redirect URI debe comenzar con http:// o https://".into(),
            ));
        }
    }

    db.set_oauth_client_config(
        "google",
        trimmed_id,
        body.client_secret.as_deref(),
        body.redirect_uri.as_deref(),
    )
    .map_err(AegisHttpError::Internal)?;

    Ok(Json(json!({ "success": true })))
}

fn infer_redirect_uri(
    headers: &HeaderMap,
    saved: Option<String>,
    body_uri: Option<String>,
) -> String {
    if let Some(u) = saved.filter(|s| !s.trim().is_empty()) {
        return u;
    }
    if let Some(u) = body_uri
        .map(|s| s.trim().to_string())
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
    {
        return u;
    }
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8090");
    format!("{}://{}/api/oauth/google/callback", proto, host)
}

pub async fn start_google_oauth(
    State(state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    headers: HeaderMap,
    Json(body): Json<GoogleStartBody>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    let creds = {
        let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
            .map_err(AegisHttpError::Internal)?;
        ank_core::oauth::resolve_client_credentials(&db, "google")
    };

    let client_id = creds.client_id.ok_or_else(|| {
        AegisHttpError::BadRequest(
            "Falta configurar el Client ID de Google (podés usar el Client ID de Orion Creative \
             y pegarlo en Cuentas Conectadas o en GOOGLE_OAUTH_CLIENT_ID)."
                .into(),
        )
    })?;

    let redirect_uri = infer_redirect_uri(&headers, creds.redirect_uri, body.redirect_uri);
    let mut scopes = vec![
        "https://www.googleapis.com/auth/calendar",
        "https://www.googleapis.com/auth/userinfo.email",
        "https://www.googleapis.com/auth/userinfo.profile",
    ];
    if body.include_extra_scopes {
        scopes.extend([
            "https://www.googleapis.com/auth/youtube.readonly",
            "https://www.googleapis.com/auth/drive.readonly",
            "https://www.googleapis.com/auth/gmail.readonly",
        ]);
    }
    let scope_str = scopes.join(" ");

    let code_verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge_hash = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(challenge_hash);
    let oauth_state = uuid::Uuid::new_v4().simple().to_string();

    let now = now_epoch();
    state
        .pending_oauth_flows
        .retain(|_, v| now.saturating_sub(v.created_at) <= OAUTH_FLOW_TTL_SECS);
    state.pending_oauth_flows.insert(
        oauth_state.clone(),
        PendingOAuthFlow {
            tenant_id: auth.tenant_id,
            session_key_hash: auth.session_key_hash,
            code_verifier,
            redirect_uri: redirect_uri.clone(),
            client_id: client_id.clone(),
            client_secret: creds.client_secret,
            created_at: now,
        },
    );

    let auth_url = reqwest::Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", scope_str.as_str()),
            ("state", oauth_state.as_str()),
            ("code_challenge", code_challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("include_granted_scopes", "true"),
        ],
    )
    .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!("URL build error: {}", e)))?;

    Ok(Json(json!({
        "auth_url": auth_url.as_str(),
        "redirect_uri": redirect_uri,
        "scopes": scopes,
    })))
}

fn render_callback_html(
    success: bool,
    title: &str,
    detail: &str,
    email: Option<&str>,
) -> Html<String> {
    let payload_json = serde_json::json!({
        "type": "aegis-oauth-complete",
        "provider": "google",
        "success": success,
        "email": email,
        "message": detail,
    })
    .to_string()
    .replace('<', "\\u003c")
    .replace('>', "\\u003e");

    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };

    let color = if success { "#10b981" } else { "#f87171" };
    let icon = if success { "✓" } else { "✕" };

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8" />
  <title>Aegis OS — Google OAuth</title>
  <style>
    body {{
      margin: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      background: #07090f; color: #e2e8f0; display: flex; align-items: center;
      justify-content: center; min-height: 100vh; padding: 24px;
    }}
    .card {{
      max-width: 460px; width: 100%; background: #0f172a; border: 1px solid #1e293b;
      border-radius: 16px; padding: 28px; text-align: center;
      box-shadow: 0 20px 50px rgba(0,0,0,0.5);
    }}
    .badge {{
      width: 48px; height: 48px; border-radius: 999px; margin: 0 auto 16px;
      display: flex; align-items: center; justify-content: center; font-size: 22px;
      background: rgba(255,255,255,0.06); color: {color}; border: 1px solid {color};
    }}
    h1 {{ font-size: 16px; margin: 0 0 10px; letter-spacing: 0.05em; }}
    p {{ font-size: 12px; color: #94a3b8; line-height: 1.6; margin: 0 0 18px; }}
    button {{
      background: #06b6d4; color: #020617; border: 0; border-radius: 10px;
      padding: 10px 18px; font-weight: 700; font-family: inherit; font-size: 12px; cursor: pointer;
    }}
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">{icon}</div>
    <h1>{title}</h1>
    <p>{detail}</p>
    <button onclick="window.close()">Cerrar ventana</button>
  </div>
  <script>
    try {{
      if (window.opener) {{
        window.opener.postMessage({payload_json}, "*");
      }}
    }} catch (e) {{}}
    if ({auto_close}) {{
      setTimeout(() => {{ try {{ window.close(); }} catch (e) {{}} }}, 1400);
    }}
  </script>
</body>
</html>"#,
        color = color,
        icon = icon,
        title = esc(title),
        detail = esc(detail),
        payload_json = payload_json,
        auto_close = if success { "true" } else { "false" }
    ))
}

pub async fn google_oauth_callback(
    State(state): State<crate::AppState>,
    Query(query): Query<GoogleCallbackQuery>,
) -> (StatusCode, Html<String>) {
    if let Some(err) = query.error {
        let desc = query
            .error_description
            .unwrap_or_else(|| "Autorización rechazada o cancelada.".to_string());
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(
                false,
                "Autorización de Google cancelada",
                &format!("Google devolvió '{}': {}", err, desc),
                None,
            ),
        );
    }

    let Some(oauth_state) = query.state.as_deref().filter(|s| !s.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(
                false,
                "Falta parámetro state",
                "Solicitud OAuth inválida.",
                None,
            ),
        );
    };

    let Some((_, flow)) = state.pending_oauth_flows.remove(oauth_state) else {
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(
                false,
                "Sesión OAuth expirada",
                "El enlace de autorización expiró o ya fue usado. Volvé a Settings y tocá Conectar Google.",
                None,
            ),
        );
    };

    if now_epoch().saturating_sub(flow.created_at) > OAUTH_FLOW_TTL_SECS {
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(
                false,
                "Sesión OAuth expirada",
                "Pasaron más de 10 minutos. Reintentá desde Settings → Cuentas Conectadas.",
                None,
            ),
        );
    }

    let Some(code) = query.code.as_deref().filter(|s| !s.is_empty()) else {
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(
                false,
                "Falta código OAuth",
                "Google no envió el parámetro `code`.",
                None,
            ),
        );
    };

    let mut form_params: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", &flow.client_id),
        ("redirect_uri", &flow.redirect_uri),
        ("code_verifier", &flow.code_verifier),
    ];
    if let Some(ref secret) = flow.client_secret {
        form_params.push(("client_secret", secret));
    }

    let client = reqwest::Client::new();
    let token_res = match client
        .post("https://oauth2.googleapis.com/token")
        .form(&form_params)
        .timeout(std::time::Duration::from_secs(12))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                render_callback_html(
                    false,
                    "Error al contactar Google",
                    &format!("No se pudo conectar con oauth2.googleapis.com: {}", e),
                    None,
                ),
            );
        }
    };

    let status_code = token_res.status();
    let token_json: serde_json::Value = match token_res.json().await {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                render_callback_html(
                    false,
                    "Respuesta inválida de Google",
                    &format!("No se pudo leer el JSON del token: {}", e),
                    None,
                ),
            );
        }
    };

    let Some(access_token) = token_json["access_token"].as_str() else {
        let err = token_json["error_description"]
            .as_str()
            .or_else(|| token_json["error"].as_str())
            .unwrap_or("Google no devolvió access_token (revisá Client Secret y Redirect URI).");
        return (
            StatusCode::BAD_REQUEST,
            render_callback_html(false, &format!("Error OAuth ({})", status_code), err, None),
        );
    };

    let refresh_token = token_json["refresh_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from);
    let expires_in = token_json["expires_in"].as_u64().unwrap_or(3600);
    let scope = token_json["scope"].as_str().unwrap_or("").to_string();

    // Fetch user email best-effort so the UI clearly shows which Google account is connected.
    let email = match client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .timeout(std::time::Duration::from_secs(6))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["email"].as_str().map(String::from)),
        _ => None,
    };

    let save_res = TenantDB::open(&flow.tenant_id, &flow.session_key_hash).and_then(|db| {
        let existing_rt = db.get_refresh_token("google").ok().flatten();
        let effective_rt = refresh_token.as_deref().or(existing_rt.as_deref());
        db.set_oauth_token("google", access_token, effective_rt, expires_in, &scope)?;
        if let Some(ref addr) = email {
            db.set_oauth_email("google", addr)?;
        }
        Ok(())
    });

    if let Err(e) = save_res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            render_callback_html(
                false,
                "Error al guardar tokens en Citadel",
                &e.to_string(),
                None,
            ),
        );
    }

    tracing::info!(
        tenant = %flow.tenant_id,
        email = ?email,
        "Google Web OAuth completed cleanly"
    );

    let msg = match email.as_deref() {
        Some(addr) => format!(
            "Cuenta {} vinculada correctamente. Ya podés consultar y crear eventos en Google Calendar desde Aegis.",
            addr
        ),
        None => "Cuenta de Google vinculada correctamente. Ya podés usar Google Calendar desde Aegis."
            .to_string(),
    };

    (
        StatusCode::OK,
        render_callback_html(true, "Google Calendar conectado", &msg, email.as_deref()),
    )
}

pub async fn disconnect_provider(
    State(_state): State<crate::AppState>,
    auth: CitadelAuthenticated,
    Path(provider): Path<String>,
    Query(query): Query<DisconnectQuery>,
) -> Result<Json<serde_json::Value>, AegisHttpError> {
    if !["google", "spotify"].contains(&provider.as_str()) {
        return Err(AegisHttpError::BadRequest("Unknown provider".into()));
    }

    let db = TenantDB::open(&auth.tenant_id, &auth.session_key_hash)
        .map_err(AegisHttpError::Internal)?;

    db.revoke_oauth(&provider)
        .map_err(AegisHttpError::Internal)?;

    if query.clear_client {
        db.clear_oauth_client_config(&provider)
            .map_err(AegisHttpError::Internal)?;
    }

    tracing::info!(
        tenant = %auth.tenant_id,
        provider = %provider,
        clear_client = query.clear_client,
        "OAuth disconnected"
    );

    Ok(Json(json!({ "success": true })))
}

pub fn router() -> axum::Router<crate::AppState> {
    axum::Router::new()
        .route("/tokens", axum::routing::post(receive_tokens))
        .route("/status", axum::routing::get(get_status))
        .route("/google/config", axum::routing::post(save_google_config))
        .route("/google/start", axum::routing::post(start_google_oauth))
        .route(
            "/google/callback",
            axum::routing::get(google_oauth_callback),
        )
        .route("/:provider", axum::routing::delete(disconnect_provider))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_infer_redirect_uri_precedence() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("aegistest.orioncrea.com"),
        );

        assert_eq!(
            infer_redirect_uri(
                &headers,
                Some("https://saved.example.com/cb".into()),
                Some("https://body.example.com/cb".into()),
            ),
            "https://saved.example.com/cb"
        );

        assert_eq!(
            infer_redirect_uri(
                &headers,
                Some("   ".into()),
                Some("https://aegistest.orioncrea.com/u/alice/api/oauth/google/callback".into()),
            ),
            "https://aegistest.orioncrea.com/u/alice/api/oauth/google/callback"
        );

        assert_eq!(
            infer_redirect_uri(&headers, None, Some("ftp://invalid".into())),
            "https://aegistest.orioncrea.com/api/oauth/google/callback"
        );

        let empty = HeaderMap::new();
        assert_eq!(
            infer_redirect_uri(&empty, None, None),
            "http://localhost:8090/api/oauth/google/callback"
        );
    }

    #[test]
    fn test_render_callback_html_escapes_and_emits_postmessage() {
        let ok_html = render_callback_html(
            true,
            "<script>x</script>",
            "Conectado",
            Some("user@example.com"),
        )
        .0;
        assert!(ok_html.contains("&lt;script&gt;x&lt;/script&gt;"));
        assert!(ok_html.contains("\"type\":\"aegis-oauth-complete\""));
        assert!(ok_html.contains("\"success\":true"));
        assert!(ok_html.contains("\"email\":\"user@example.com\""));
        assert!(ok_html.contains("if (true)"));

        let xss_html =
            render_callback_html(false, "Error", "</script><script>alert(1)</script>", None).0;
        assert!(!xss_html.contains("</script><script>alert(1)</script>"));
        assert!(xss_html
            .contains("\\u003c/script\\u003e\\u003cscript\\u003ealert(1)\\u003c/script\\u003e"));

        let err_html = render_callback_html(false, "Error", "access_denied", None).0;
        assert!(err_html.contains("\"success\":false"));
        assert!(err_html.contains("if (false)"));
    }

    #[test]
    fn test_oauth_request_body_deserialization() {
        let tok: OAuthTokensBody =
            serde_json::from_str(r#"{"provider":"google","access_token":"abc"}"#).unwrap();
        assert_eq!(tok.expires_in, 3600);
        assert!(tok.email.is_none());
        assert!(tok.client_id.is_none());

        let start: GoogleStartBody = serde_json::from_str("{}").unwrap();
        assert!(!start.include_extra_scopes);
        assert!(start.redirect_uri.is_none());

        let cfg: GoogleClientConfigBody = serde_json::from_str(r#"{"clear":true}"#).unwrap();
        assert!(cfg.clear);
        assert_eq!(cfg.client_id, "");
    }
}
