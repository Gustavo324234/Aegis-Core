use tracing::debug;
use std::time::Duration;

/// Verifica que el ANK responde HTTP en el puerto configurado
pub async fn check_health(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    debug!("Checking health on {}", url);

    // Damos un pequeño margen para el arranque
    for _ in 0..10 {
        match reqwest::get(&url).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return true;
                }
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    false
}
