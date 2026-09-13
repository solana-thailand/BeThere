//! Rate limiting middleware (Issue #039).

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Per-IP rate limit bucket.
struct RateBucket {
    /// Unix timestamp (seconds) of the current window start.
    window_start: i64,
    /// Number of requests in the current window.
    count: u32,
}

/// Global rate limit state — per isolate.
///
/// Keyed by `(IP, route_group)` so different endpoint groups have
/// independent limits.
static RATE_LIMIT_STATE: LazyLock<Mutex<HashMap<(String, &'static str), RateBucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Rate limit configuration per route group.
struct RateLimitConfig {
    /// Route group identifier (for logging).
    group: &'static str,
    /// Maximum requests per IP per window.
    max_requests: u32,
    /// Window duration in seconds.
    window_secs: i64,
}

#[derive(Clone, Copy)]
enum LimiterKind {
    Auth,
    Claim,
    Deposit,
    Webhook,
}

const RATE_LIMIT_AUTH: RateLimitConfig = RateLimitConfig {
    group: "auth",
    max_requests: 20,
    window_secs: 60,
};

const RATE_LIMIT_CLAIM: RateLimitConfig = RateLimitConfig {
    group: "claim",
    max_requests: 120,
    window_secs: 60,
};

const RATE_LIMIT_DEPOSIT: RateLimitConfig = RateLimitConfig {
    group: "deposit",
    max_requests: 60,
    window_secs: 60,
};

const RATE_LIMIT_WEBHOOK: RateLimitConfig = RateLimitConfig {
    group: "webhook",
    max_requests: 120,
    window_secs: 60,
};

/// Extract client IP from Cloudflare headers.
///
/// Cloudflare sets `cf-connecting-ip` on every request.
/// Do not trust caller-controlled forwarding headers when the edge header is absent.
fn extract_client_ip(req: &Request) -> String {
    req.headers()
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Check rate limit for a given IP + route group. Returns `true` if allowed.
fn check_rate_limit(ip: &str, config: &RateLimitConfig) -> bool {
    let now = chrono::Utc::now().timestamp();
    let key = (ip.to_string(), config.group);

    let mut state = RATE_LIMIT_STATE.lock().unwrap_or_else(|e| e.into_inner());

    // Bound isolate memory even under a stream of distinct source addresses.
    if state.len() >= 4096 && !state.contains_key(&key) {
        state.retain(|_, bucket| now - bucket.window_start < 60);
        if state.len() >= 4096 {
            return false;
        }
    }

    let bucket = state.entry(key).or_insert(RateBucket {
        window_start: now,
        count: 0,
    });

    // Reset window if expired
    if now - bucket.window_start >= config.window_secs {
        bucket.window_start = now;
        bucket.count = 0;
    }

    bucket.count = bucket.count.saturating_add(1);

    if bucket.count > config.max_requests {
        tracing::warn!(
            ip = %ip,
            group = %config.group,
            count = bucket.count,
            max = config.max_requests,
            window_secs = config.window_secs,
            "rate limit exceeded"
        );
        return false;
    }

    true
}

/// Evict expired buckets to prevent memory growth.
/// Called periodically (every 100th rate-limited request via lazy counter).
fn evict_expired_buckets() {
    let now = chrono::Utc::now().timestamp();
    let mut state = RATE_LIMIT_STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.retain(|_, bucket| now - bucket.window_start < 3600); // evict after 1 hour idle
}

static EVICT_COUNTER: LazyLock<Mutex<u64>> = LazyLock::new(|| Mutex::new(0));

fn maybe_evict() {
    let mut count = EVICT_COUNTER.lock().unwrap_or_else(|e| e.into_inner());
    *count = count.wrapping_add(1);
    if (*count).is_multiple_of(100) {
        evict_expired_buckets();
    }
}

/// Determine which rate limit config applies to a path.
/// Returns `None` for paths that should not be rate-limited.
fn rate_limit_for_path(path: &str) -> Option<(&'static RateLimitConfig, LimiterKind)> {
    if matches!(
        path,
        "/api/escrow/onchain-webhook" | "/api/deposit/usdc/webhook"
    ) {
        return Some((&RATE_LIMIT_WEBHOOK, LimiterKind::Webhook));
    }
    // Auth endpoints
    if path.starts_with("/api/auth/") {
        return Some((&RATE_LIMIT_AUTH, LimiterKind::Auth));
    }
    // Claim endpoints
    if path.starts_with("/api/claim/") {
        return Some((&RATE_LIMIT_CLAIM, LimiterKind::Claim));
    }
    // Deposit endpoints
    if path.starts_with("/api/deposit/") {
        return Some((&RATE_LIMIT_DEPOSIT, LimiterKind::Deposit));
    }
    None
}

/// Axum middleware that applies native per-location limits to sensitive endpoints.
///
/// Routes not in the rate limit config pass through without any overhead.
/// Falls back to the isolate-local IP limiter when the binding is unavailable.
pub async fn rate_limit_layer(
    State(state): State<crate::state::AppState>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    if let Some((config, kind)) = rate_limit_for_path(&path) {
        let ip = extract_client_ip(&req);
        let limiter = match kind {
            LimiterKind::Auth => state.auth_rate_limiter.as_deref(),
            LimiterKind::Claim => state.claim_rate_limiter.as_deref(),
            LimiterKind::Deposit => state.deposit_rate_limiter.as_deref(),
            LimiterKind::Webhook => state.webhook_rate_limiter.as_deref(),
        };
        let key = binding_key(config, &ip);
        let allowed = if let Some(limiter) = limiter {
            match limiter.limit(key).await {
                Ok(outcome) => outcome.success,
                Err(error) => {
                    tracing::warn!(group = config.group, error = ?error, "native rate limiter unavailable; using isolate fallback");
                    check_rate_limit(&ip, config)
                }
            }
        } else {
            check_rate_limit(&ip, config)
        };

        if !allowed {
            let body = format!(
                r#"{{"error":"rate_limit_exceeded","retry_after_secs":{}}}"#,
                config.window_secs
            );
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
                .header("Content-Type", "application/json")
                .header("Cache-Control", "no-store")
                .header("Retry-After", config.window_secs.to_string())
                .body(axum::body::Body::from(body))
                .unwrap();
        }

        maybe_evict();
    }

    next.run(req).await
}

fn binding_key(config: &RateLimitConfig, ip: &str) -> String {
    // This middleware precedes authentication. Neither credentials nor paths
    // establish an identity, and capability tokens must not enter counter keys.
    format!("{}:{ip}", config.group)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn request(path: &str, authorization: Option<&str>) -> Request {
        let mut builder = Request::builder().uri(path);
        if let Some(value) = authorization {
            builder = builder.header("authorization", value);
        }
        builder.body(Body::empty()).expect("valid test request")
    }

    #[test]
    fn rotating_credentials_and_paths_does_not_rotate_the_bucket() {
        let key = |path, credential| {
            let mut req = request(path, Some(credential));
            req.headers_mut()
                .insert("cf-connecting-ip", "203.0.113.1".parse().unwrap());
            req.headers_mut()
                .insert("cookie", credential.parse().unwrap());
            let (config, _) = rate_limit_for_path(req.uri().path()).unwrap();
            binding_key(config, &extract_client_ip(&req))
        };
        assert_eq!(
            key("/api/deposit/usdc", "a"),
            key("/api/deposit/status", "b")
        );
        assert_eq!(
            key("/api/claim/token-a", "a"),
            key("/api/claim/token-b", "b")
        );
    }

    #[test]
    fn webhook_sources_do_not_share_a_global_budget() {
        for path in ["/api/deposit/usdc/webhook", "/api/escrow/onchain-webhook"] {
            let (config, _) = rate_limit_for_path(path).unwrap();
            assert_eq!(config.group, "webhook");
            assert_ne!(
                binding_key(config, "203.0.113.1"),
                binding_key(config, "198.51.100.2")
            );
        }
    }

    #[test]
    fn untrusted_forwarding_header_cannot_select_an_ip() {
        let mut req = request("/api/auth/me", None);
        req.headers_mut()
            .insert("x-forwarded-for", "203.0.113.1".parse().unwrap());
        assert_eq!(extract_client_ip(&req), "unknown");
    }
}
