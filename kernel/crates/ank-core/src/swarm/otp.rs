use base64::{engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OtpError {
    #[error("Cryptographic error: {0}")]
    CryptoError(String),
}

/// Genera un OTP de un solo uso para teletransportación, firmado con la ROOT_KEY.
/// Válido por una ventana de ~5 minutos.
pub fn generate_teleport_otp(tenant_id: &str, root_key: &[u8]) -> Result<String, OtpError> {
    type HmacSha256 = Hmac<Sha256>;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| OtpError::CryptoError(e.to_string()))?
        .as_secs()
        / 300; // Ventana de 5 minutos

    let mut mac =
        HmacSha256::new_from_slice(root_key).map_err(|e| OtpError::CryptoError(e.to_string()))?;

    mac.update(tenant_id.as_bytes());
    mac.update(&timestamp.to_be_bytes());
    let bytes = mac.finalize().into_bytes();

    Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
}

/// Verifica que un OTP de teletransportación sea válido para el tenant dado.
pub fn verify_teleport_otp(
    token: &str,
    tenant_id: &str,
    root_key: &[u8],
) -> Result<bool, OtpError> {
    type HmacSha256 = Hmac<Sha256>;
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| OtpError::CryptoError(e.to_string()))?
        .as_secs()
        / 300;

    let token_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|e| OtpError::CryptoError(e.to_string()))?;

    // Verificar ventana actual y ventana anterior (por si el clock skew o retraso de red)
    for t in [current_timestamp, current_timestamp - 1] {
        let mut mac = HmacSha256::new_from_slice(root_key)
            .map_err(|e| OtpError::CryptoError(e.to_string()))?;
        mac.update(tenant_id.as_bytes());
        mac.update(&t.to_be_bytes());

        if mac.verify_slice(&token_bytes).is_ok() {
            return Ok(true);
        }
    }

    Ok(false)
}
