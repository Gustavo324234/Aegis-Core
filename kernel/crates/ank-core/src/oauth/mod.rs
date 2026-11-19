use crate::enclave::TenantDB;
use anyhow::Result;
use std::sync::Arc;

pub mod google_calendar;

#[derive(Debug, Clone, Default)]
pub struct OAuthClientCredentials {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
}

/// Resolves per-tenant OAuth client credentials from `TenantDB`, falling back
/// to process environment variables (`GOOGLE_OAUTH_CLIENT_ID` / `GOOGLE_CLIENT_ID`, etc.).
pub fn resolve_client_credentials(db: &TenantDB, provider: &str) -> OAuthClientCredentials {
    let (db_id, db_secret, db_redirect) = db.get_oauth_client_config(provider).unwrap_or_default();

    let (env_id, env_secret, env_redirect) = match provider {
        "google" => (
            std::env::var("GOOGLE_OAUTH_CLIENT_ID")
                .or_else(|_| std::env::var("GOOGLE_CLIENT_ID"))
                .ok(),
            std::env::var("GOOGLE_OAUTH_CLIENT_SECRET")
                .or_else(|_| std::env::var("GOOGLE_CLIENT_SECRET"))
                .ok(),
            std::env::var("GOOGLE_OAUTH_REDIRECT_URI")
                .or_else(|_| std::env::var("GOOGLE_REDIRECT_URI"))
                .ok(),
        ),
        "spotify" => (
            std::env::var("SPOTIFY_CLIENT_ID").ok(),
            std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
            std::env::var("SPOTIFY_REDIRECT_URI").ok(),
        ),
        _ => (None, None, None),
    };

    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    OAuthClientCredentials {
        client_id: clean(db_id).or_else(|| clean(env_id)),
        client_secret: clean(db_secret).or_else(|| clean(env_secret)),
        redirect_uri: clean(db_redirect).or_else(|| clean(env_redirect)),
    }
}

fn token_endpoint_for(provider: &str) -> Result<&'static str> {
    match provider {
        "google" => Ok("https://oauth2.googleapis.com/token"),
        "spotify" => Ok("https://accounts.spotify.com/api/token"),
        other => anyhow::bail!("Unknown OAuth provider: {}", other),
    }
}

async fn exchange_refresh_token_http(
    http_client: &Arc<reqwest::Client>,
    provider: &str,
    refresh_token: &str,
    creds: &OAuthClientCredentials,
    existing_scope: &str,
) -> Result<(String, Option<String>, u64, String)> {
    let token_url = token_endpoint_for(provider)?;

    let mut params: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.to_string()),
    ];
    if let Some(ref cid) = creds.client_id {
        params.push(("client_id", cid.clone()));
    }
    if let Some(ref csec) = creds.client_secret {
        params.push(("client_secret", csec.clone()));
    }

    let resp_raw = http_client
        .post(token_url)
        .form(&params)
        .timeout(std::time::Duration::from_secs(12))
        .send()
        .await?;

    let status = resp_raw.status();
    let resp: serde_json::Value = resp_raw.json().await?;

    if !status.is_success() || resp.get("access_token").and_then(|v| v.as_str()).is_none() {
        let err_code = resp["error"].as_str().unwrap_or("refresh_failed");
        let err_desc = resp["error_description"]
            .as_str()
            .unwrap_or("No se pudo renovar el token OAuth");
        anyhow::bail!(
            "OAuth refresh error for '{}' (HTTP {}): {} - {}",
            provider,
            status,
            err_code,
            err_desc
        );
    }

    let new_token = resp["access_token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let rotated_rt = resp["refresh_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from);
    let expires_in = resp["expires_in"].as_u64().unwrap_or(3600);
    let scope = resp["scope"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(existing_scope)
        .to_string();

    Ok((new_token, rotated_rt, expires_in, scope))
}

/// Refreshes an OAuth token using a `&TenantDB` reference (single-thread contexts).
pub async fn get_or_refresh_token(
    http_client: &Arc<reqwest::Client>,
    db: &TenantDB,
    provider: &str,
) -> Result<String> {
    if let Some(token) = db.get_valid_access_token(provider)? {
        return Ok(token);
    }

    let refresh_token = db.get_refresh_token(provider)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Provider '{}' not connected or token expired without refresh_token. \
             Reconnect in Settings → Cuentas Conectadas.",
            provider
        )
    })?;

    let creds = resolve_client_credentials(db, provider);
    let existing_scope = db.get_oauth_scope(provider)?.unwrap_or_default();

    let (new_token, rotated_rt, expires_in, scope) = exchange_refresh_token_http(
        http_client,
        provider,
        &refresh_token,
        &creds,
        &existing_scope,
    )
    .await?;

    db.set_oauth_token(
        provider,
        &new_token,
        rotated_rt.as_deref(),
        expires_in,
        &scope,
    )?;

    Ok(new_token)
}

/// `Send`-safe OAuth token retrieval + refresh for multi-threaded async tasks
/// (`ank-http` and `chal`): opens and drops `TenantDB` strictly before any `.await`.
pub async fn get_or_refresh_token_for_tenant(
    http_client: &Arc<reqwest::Client>,
    tenant_id: &str,
    session_key: &str,
    provider: &str,
    force_refresh: bool,
) -> Result<String> {
    let (cached_token, refresh_token, creds, existing_scope) = {
        let db = TenantDB::open(tenant_id, session_key)?;
        let cached = if force_refresh {
            None
        } else {
            db.get_valid_access_token(provider)?
        };
        if let Some(tok) = cached {
            return Ok(tok);
        }
        let rt = db.get_refresh_token(provider)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Provider '{}' not connected (missing refresh_token). Connect in Settings → Cuentas Conectadas.",
                provider
            )
        })?;
        let creds = resolve_client_credentials(&db, provider);
        let scope = db.get_oauth_scope(provider)?.unwrap_or_default();
        (None::<String>, rt, creds, scope)
    };
    let _ = cached_token;

    let (new_token, rotated_rt, expires_in, scope) = exchange_refresh_token_http(
        http_client,
        provider,
        &refresh_token,
        &creds,
        &existing_scope,
    )
    .await?;

    {
        let db = TenantDB::open(tenant_id, session_key)?;
        db.set_oauth_token(
            provider,
            &new_token,
            rotated_rt.as_deref(),
            expires_in,
            &scope,
        )?;
    }

    Ok(new_token)
}
