//! Security headers middleware for the Cloudflare Worker.
//!
//! Adds security-related HTTP headers to every response:
//! - `Strict-Transport-Security` (HSTS) — enforce HTTPS for 2 years
//! - `X-Content-Type-Options` — prevent MIME type sniffing
//! - `X-Frame-Options` — prevent clickjacking (DENY)
//! - `X-XSS-Protection` — disabled (modern browsers use CSP instead)
//! - `Referrer-Policy` — limit referrer info to origin only
//! - `Content-Security-Policy` — restrict resource loading
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
         img-src 'self' data: blob:; \
         media-src 'self' blob:; \
         connect-src 'self' https://cloudflareinsights.com; \
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
