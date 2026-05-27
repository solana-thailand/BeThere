//! Security headers middleware for the Cloudflare Worker.
//!
//! Adds security-related HTTP headers to every response:
//! - `Strict-Transport-Security` (HSTS) — enforce HTTPS for 2 years
//! - `X-Content-Type-Options` — prevent MIME type sniffing
//! - `X-Frame-Options` — prevent clickjacking (DENY)
//! - `X-XSS-Protection` — disabled (modern browsers use CSP instead)
//! - `Referrer-Policy` — limit referrer info to origin only
//! - `Content-Security-Policy` — restrict resource loading
//!   - `connect-src 'self' https: wss:` — allows Solana RPC, wallet extensions (Phantom/Solflare/Backpack), and WebSocket connections
//!   - `frame-src https://www.youtube.com https://www.youtube-nocookie.com` — allows YouTube iframe embeds on ticket pages
//! - `Permissions-Policy` — limit browser feature access
//! - `Cross-Origin-Opener-Policy` — isolate window origin
//! - `Cross-Origin-Resource-Policy` — prevent cross-origin resource leaks

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, header},
    middleware::Next,
    response::Response,
};
use std::ops::Deref;
use std::sync::LazyLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// CorrelationId — request-scoped tracing identifier
// ---------------------------------------------------------------------------

/// Correlation ID extracted from `x-correlation-id` header or generated as UUID v7.
///
/// Inserted into request extensions by [`correlation_id_layer`] so downstream
/// handlers / layers can read it. Also added to every response header.
#[derive(Clone, Debug)]
pub struct CorrelationId(pub String);

impl Deref for CorrelationId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

static STRICT_TRANSPORT_SECURITY: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"));

static X_CONTENT_TYPE_OPTIONS: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("nosniff"));

static X_FRAME_OPTIONS: LazyLock<HeaderValue> = LazyLock::new(|| HeaderValue::from_static("DENY"));

static X_XSS_PROTECTION: LazyLock<HeaderValue> = LazyLock::new(|| HeaderValue::from_static("0"));

static REFERRER_POLICY: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("strict-origin-when-cross-origin"));

static CONTENT_SECURITY_POLICY: LazyLock<HeaderValue> = LazyLock::new(|| {
    HeaderValue::from_static(
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://unpkg.com https://cdn.jsdelivr.net https://static.cloudflareinsights.com; \
         style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
         img-src 'self' data: blob: https:; \
         media-src 'self' blob:; \
         frame-src https://www.youtube.com https://www.youtube-nocookie.com; \
         connect-src 'self' https: wss:; \
         font-src 'self' https://fonts.gstatic.com; \
         frame-ancestors 'none'; \
         base-uri 'self'; \
         form-action 'self'",
    )
});

static PERMISSIONS_POLICY: LazyLock<HeaderValue> = LazyLock::new(|| {
    HeaderValue::from_static(
        "camera=(self), \
         microphone=(), \
         geolocation=(), \
         payment=()",
    )
});

static CROSS_ORIGIN_OPENER_POLICY: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("same-origin"));

static CROSS_ORIGIN_RESOURCE_POLICY: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("same-origin"));

/// Axum middleware that adds security headers to every response.
///
/// Applied as a layer around the entire router so all responses
/// (API + static assets) include these headers.
pub async fn security_headers_layer(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    add_security_headers(response)
}

/// Axum middleware that assigns a correlation ID to every request/response cycle.
///
/// 1. Reads `x-correlation-id` from the incoming request header.
/// 2. Falls back to a new UUID v7 if the header is missing.
/// 3. Inserts a [`CorrelationId`] into request extensions.
/// 4. Logs request start and completion with the correlation ID.
/// 5. Adds `x-correlation-id` to the response headers.
pub async fn correlation_id_layer(mut req: Request, next: Next) -> Response {
    // 1. Check for incoming x-correlation-id header
    let correlation_id = req
        .headers()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::now_v7().to_string());

    // 2. Insert into request extensions
    req.extensions_mut()
        .insert(CorrelationId(correlation_id.clone()));

    // 3. Log request entry
    tracing::info!(
        correlation_id = %correlation_id,
        method = %req.method(),
        path = %req.uri().path(),
        "request started"
    );

    // 4. Run handler
    let mut response = next.run(req).await;

    // 5. Add correlation_id to response headers
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-correlation-id"),
        correlation_id.parse().unwrap(),
    );

    // 6. Log response
    let status = response.status();
    tracing::info!(
        correlation_id = %correlation_id,
        status = %status.as_u16(),
        "request completed"
    );

    response
}

// ---------------------------------------------------------------------------
// Cache-Control middleware — adds caching headers to public endpoints
// ---------------------------------------------------------------------------

static CACHE_PUBLIC_60: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("public, max-age=60"));

static CACHE_PUBLIC_120: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("public, max-age=120"));

static CACHE_NO_CACHE: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("no-cache"));

static CACHE_NO_STORE: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("no-store"));

/// Adds `Cache-Control: public, max-age=60` — for public event lists.
pub async fn cache_public_60_layer(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, CACHE_PUBLIC_60.clone());
    response
}

/// Adds `Cache-Control: public, max-age=120` — for individual public event details.
pub async fn cache_public_120_layer(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, CACHE_PUBLIC_120.clone());
    response
}

/// Adds `Cache-Control: no-cache` — for health check (must revalidate).
pub async fn cache_no_cache_layer(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, CACHE_NO_CACHE.clone());
    response
}

/// Adds `Cache-Control: no-store` — for auth endpoints (sensitive data).
pub async fn cache_no_store_layer(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, CACHE_NO_STORE.clone());
    response
}

fn add_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        STRICT_TRANSPORT_SECURITY.clone(),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        X_CONTENT_TYPE_OPTIONS.clone(),
    );
    headers.insert(header::X_FRAME_OPTIONS, X_FRAME_OPTIONS.clone());
    headers.insert("x-xss-protection", X_XSS_PROTECTION.clone());
    headers.insert(header::REFERRER_POLICY, REFERRER_POLICY.clone());
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        CONTENT_SECURITY_POLICY.clone(),
    );
    headers.insert("permissions-policy", PERMISSIONS_POLICY.clone());
    headers.insert(
        "cross-origin-opener-policy",
        CROSS_ORIGIN_OPENER_POLICY.clone(),
    );
    headers.insert(
        "cross-origin-resource-policy",
        CROSS_ORIGIN_RESOURCE_POLICY.clone(),
    );
    response
}

// ---------------------------------------------------------------------------
// Rate limiting middleware (Issue #039)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;

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
    // Escrow webhooks
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
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
                .body(axum::body::Body::empty())
                .unwrap();
        }

        maybe_evict();
    }

    next.run(req).await
}
