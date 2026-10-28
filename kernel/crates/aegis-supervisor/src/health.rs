use std::time::Duration;
use tracing::debug;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

fn build_health_client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()
}

/// Verifica que el ANK responde HTTP en el puerto configurado con timeouts estrictos
/// para que un socket colgado nunca bloquee el bucle de supervisión.
pub async fn check_health(port: u16) -> bool {
    check_health_with_retries(port, 4, Duration::from_millis(500)).await
}

pub async fn check_health_with_retries(port: u16, attempts: usize, retry_delay: Duration) -> bool {
    let Some(client) = build_health_client() else {
        return false;
    };

    let url = format!("http://127.0.0.1:{}/health", port);
    debug!("Checking health on {}", url);

    let total_attempts = attempts.max(1);
    for idx in 0..total_attempts {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        if idx + 1 < total_attempts {
            tokio::time::sleep(retry_delay).await;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_returns_false_quickly_when_closed() {
        // Closed ephemeral port on loopback returns false without hanging
        let listener = std::net::TcpListener::bind("127.0.0.1:0");
        let Ok(listener) = listener else {
            return;
        };
        let Ok(addr) = listener.local_addr() else {
            return;
        };
        let port = addr.port();
        drop(listener);

        let ok = check_health_with_retries(port, 2, Duration::from_millis(20)).await;
        assert!(!ok);
    }
}
