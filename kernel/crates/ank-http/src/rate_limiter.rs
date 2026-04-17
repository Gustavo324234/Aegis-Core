use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimitConfig {
    pub max_attempts: u32,
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            window_secs: 60,
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        let default = Self::default();
        let env_val = std::env::var("AEGIS_AUTH_RATE_LIMIT").ok();

        let (max_attempts, window_secs) = match env_val.as_deref() {
            Some(v) => {
                parse_rate_limit_env(v).unwrap_or((default.max_attempts, default.window_secs))
            }
            None => (default.max_attempts, default.window_secs),
        };

        Self {
            max_attempts,
            window_secs,
        }
    }
}

fn parse_rate_limit_env(s: &str) -> Option<(u32, u64)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let max_attempts: u32 = parts[0].parse().ok()?;
    let window_secs: u64 = parts[1].parse().ok()?;
    Some((max_attempts, window_secs))
}

struct Entry {
    attempts: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct AuthRateLimiter {
    config: RateLimitConfig,
    store: Arc<DashMap<String, Entry>>,
}

#[derive(Debug)]
pub enum RateLimitOutcome {
    Allowed { remaining: u32, reset_in_secs: u64 },
    Blocked { retry_after_secs: u64 },
}

impl AuthRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            store: Arc::new(DashMap::new()),
        }
    }

    pub fn check_and_record_failed(&self, ip: IpAddr, tenant_id: &str) -> RateLimitOutcome {
        let key = format!("{}_{}", ip, tenant_id);
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_secs);

        let mut entry = self.store.entry(key).or_insert_with(|| Entry {
            attempts: 0,
            window_start: now,
        });

        let e = entry.value_mut();
        if now.duration_since(e.window_start) >= window {
            e.attempts = 1;
            e.window_start = now;
            return RateLimitOutcome::Allowed {
                remaining: self.config.max_attempts - 1,
                reset_in_secs: self.config.window_secs,
            };
        }

        if e.attempts >= self.config.max_attempts {
            let elapsed = now.duration_since(e.window_start);
            let retry_after_secs = window.saturating_sub(elapsed).as_secs().max(1);
            return RateLimitOutcome::Blocked { retry_after_secs };
        }

        e.attempts += 1;
        let remaining = self.config.max_attempts - e.attempts;
        let elapsed = now.duration_since(e.window_start);
        let reset_in_secs = window.saturating_sub(elapsed).as_secs();
        RateLimitOutcome::Allowed {
            remaining,
            reset_in_secs,
        }
    }

    pub fn reset(&self, ip: IpAddr, tenant_id: &str) {
        let key = format!("{}_{}", ip, tenant_id);
        self.store.remove(&key);
    }

    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_config_default() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.max_attempts, 5);
        assert_eq!(cfg.window_secs, 60);
    }

    #[test]
    fn rate_limit_config_parse_env() {
        let cfg = RateLimitConfig::from_env();
        assert!(cfg.max_attempts >= 1);
        assert!(cfg.window_secs >= 1);
    }

    #[test]
    fn parse_rate_limit_env_valid() {
        assert_eq!(parse_rate_limit_env("10/120"), Some((10, 120)));
        assert_eq!(parse_rate_limit_env("3/30"), Some((3, 30)));
    }

    #[test]
    fn parse_rate_limit_env_invalid() {
        assert_eq!(parse_rate_limit_env("abc"), None);
        assert_eq!(parse_rate_limit_env("5"), None);
        assert_eq!(parse_rate_limit_env("5/abc"), None);
    }

    #[test]
    fn check_and_record_first_attempt() {
        let limiter = AuthRateLimiter::new(RateLimitConfig {
            max_attempts: 5,
            window_secs: 60,
        });
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let outcome = limiter.check_and_record_failed(ip, "tenant1");
        match outcome {
            RateLimitOutcome::Allowed {
                remaining,
                reset_in_secs,
            } => {
                assert_eq!(remaining, 4);
                assert_eq!(reset_in_secs, 60);
            }
            RateLimitOutcome::Blocked { .. } => panic!("expected Allowed"),
        }
    }

    #[test]
    fn check_and_record_max_attempts() {
        let limiter = AuthRateLimiter::new(RateLimitConfig {
            max_attempts: 3,
            window_secs: 60,
        });
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        for i in 0..2 {
            let outcome = limiter.check_and_record_failed(ip, "tenant1");
            match outcome {
                RateLimitOutcome::Allowed { remaining, .. } => {
                    assert_eq!(remaining, 2 - i);
                }
                RateLimitOutcome::Blocked { .. } => panic!("unexpected Blocked at attempt {}", i),
            }
        }

        let outcome = limiter.check_and_record_failed(ip, "tenant1");
        match outcome {
            RateLimitOutcome::Blocked { retry_after_secs } => {
                assert!(retry_after_secs >= 1 && retry_after_secs <= 60);
            }
            RateLimitOutcome::Allowed { .. } => panic!("expected Blocked at max attempts"),
        }
    }

    #[test]
    fn successful_reset_clears_entry() {
        let limiter = AuthRateLimiter::new(RateLimitConfig {
            max_attempts: 3,
            window_secs: 60,
        });
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        for _ in 0..3 {
            limiter.check_and_record_failed(ip, "tenant1");
        }

        let outcome = limiter.check_and_record_failed(ip, "tenant1");
        assert!(matches!(outcome, RateLimitOutcome::Blocked { .. }));

        limiter.reset(ip, "tenant1");

        let outcome = limiter.check_and_record_failed(ip, "tenant1");
        match outcome {
            RateLimitOutcome::Allowed { remaining, .. } => {
                assert_eq!(remaining, 2);
            }
            RateLimitOutcome::Blocked { .. } => panic!("expected Allowed after reset"),
        }
    }
}
