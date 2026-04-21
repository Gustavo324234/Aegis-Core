use crate::enclave::TenantDB;
use anyhow::Result;

pub const PROVIDER_GOOGLE: &str = "google";

pub fn get_google_token(
    db: &TenantDB,
    http_client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<Option<String>> {
    let is_connected = db.is_oauth_connected(PROVIDER_GOOGLE)?;
    if !is_connected {
        return Ok(None);
    }

    if let Some(token) = db.get_valid_access_token(PROVIDER_GOOGLE)? {
        return Ok(Some(token));
    }

    if let Some(refresh_token) = db.get_refresh_token(PROVIDER_GOOGLE)? {
        let params = [
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", &refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let response = http_client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()?;

        if response.status().is_success() {
            let data: serde_json::Value = response.json()?;
            let new_access_token = data["access_token"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No access_token in refresh response"))?;
            let expires_in = data["expires_in"].as_u64().unwrap_or(3600);

            db.set_oauth_token(
                PROVIDER_GOOGLE,
                new_access_token,
                None,
                expires_in,
                "https://www.googleapis.com/auth/calendar.readonly \
                 https://www.googleapis.com/auth/drive.readonly \
                 https://www.googleapis.com/auth/gmail.readonly",
            )?;

            return Ok(Some(new_access_token.to_string()));
        }
    }

    Ok(None)
}
