use anyhow::Result;
use std::sync::Arc;

pub async fn get_or_refresh_token(
    http_client: &Arc<reqwest::Client>,
    db: &crate::enclave::TenantDB,
    provider: &str,
) -> Result<String> {
    if let Some(token) = db.get_valid_access_token(provider)? {
        return Ok(token);
    }

    let refresh_token = db.get_refresh_token(provider)?.ok_or_else(|| {
        anyhow::anyhow!(
            "Provider '{}' not connected. Tell the user to connect their account \
             from the Aegis app (Settings → Cuentas).",
            provider
        )
    })?;

    let token_url = match provider {
        "google" => "https://oauth2.googleapis.com/token",
        "spotify" => "https://accounts.spotify.com/api/token",
        other => anyhow::bail!("Unknown OAuth provider: {}", other),
    };

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
