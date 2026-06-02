//! Rate limiting middleware (Issue #039).

use axum::{extract::Request, middleware::Next, response::Response};
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

const RATE_LIMIT_AUTH: RateLimitConfig = RateLimitConfig {
    group: "auth",
    max_requests: 20,
    window_secs: 60,
};

const RATE_LIMIT_CLAIM: RateLimitConfig = RateLimitConfig {
    group: "claim",
    max_requests: 10,
    window_secs: 60,
};

const RATE_LIMIT_DEPOSIT: RateLimitConfig = RateLimitConfig {
    group: "deposit",
    max_requests: 10,
    window_secs: 60,
};

const RATE_LIMIT_WEBHOOK: RateLimitConfig = RateLimitConfig {
    group: "webhook",
    max_requests: 30,
    window_secs: 60,
};

/// Extract client IP from Cloudflare headers.
///
/// Cloudflare sets `cf-connecting-ip` on every request.
/// Falls back to `x-forwarded-for` first entry, then `unknown`.
fn extract_client_ip(req: &Request) -> String {
    req.headers()
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        })
}

/// Check rate limit for a given IP + route group. Returns `true` if allowed.
fn check_rate_limit(ip: &str, config: &RateLimitConfig) -> bool {
    let now = chrono::Utc::now().timestamp();
    let key = (ip.to_string(), config.group);

    let mut state = RATE_LIMIT_STATE.lock().unwrap_or_else(|e| e.into_inner());

    let bucket = state.entry(key).or_insert(RateBucket {
        window_start: now,
        count: 0,
    });

    // Reset window if expired
    if now - bucket.window_start >= config.window_secs {
        bucket.window_start = now;
        bucket.count = 0;
    }

    bucket.count += 1;

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
    *count += 1;
    if (*count).is_multiple_of(100) {
        evict_expired_buckets();
    }
}

/// Determine which rate limit config applies to a path.
/// Returns `None` for paths that should not be rate-limited.
fn rate_limit_for_path(path: &str) -> Option<&'static RateLimitConfig> {
    // Auth endpoints
    if path.starts_with("/api/auth/") {
        return Some(&RATE_LIMIT_AUTH);
    }
    // Claim endpoints
    if path.starts_with("/api/claim/") {
        return Some(&RATE_LIMIT_CLAIM);
    }
    // Deposit endpoints
    if path.starts_with("/api/deposit/") {
        return Some(&RATE_LIMIT_DEPOSIT);
    }
    // Escrow webhooks (both on-chain and deposit)
    if path == "/api/escrow/onchain-webhook" {
        return Some(&RATE_LIMIT_WEBHOOK);
    }
    None
}

/// Axum middleware that applies per-IP rate limiting to sensitive endpoints.
///
/// Routes not in the rate limit config pass through without any overhead.
/// Uses `cf-connecting-ip` header set by Cloudflare on every request.
pub async fn rate_limit_layer(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();

    if let Some(config) = rate_limit_for_path(&path) {
        let ip = extract_client_ip(&req);

        if !check_rate_limit(&ip, config) {
            let body = format!(
                r#"{{"error":"rate_limit_exceeded","retry_after_secs":{}}}"#,
                config.window_secs
            );
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
                .header("Content-Type", "application/json")
                .header("Retry-After", config.window_secs.to_string())
                .body(axum::body::Body::from(body))
                .unwrap();
        }

        maybe_evict();
    }

    next.run(req).await
}
