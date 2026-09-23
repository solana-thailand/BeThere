//! Cache-Control middleware — adds caching headers to public endpoints.

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::sync::LazyLock;

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
    with_public_cache(next.run(req).await, &CACHE_PUBLIC_60)
}

/// Adds `Cache-Control: public, max-age=120` — for individual public event details.
pub async fn cache_public_120_layer(req: Request, next: Next) -> Response {
    with_public_cache(next.run(req).await, &CACHE_PUBLIC_120)
}

/// `Cache-Control` that a handler sets on a per-viewer response (a private
/// event served to an authorised member) so the public layers leave it alone.
pub static CACHE_PRIVATE_NO_STORE: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::from_static("private, no-store"));

/// Mark only successful responses as publicly cacheable, and never override a
/// `Cache-Control` the handler already chose. Errors (401/403 on a private
/// event, 404 before publish) must not be pinned in shared caches either.
/// A 304 repeats the policy of the 200 it revalidates (RFC 9110 §15.4.5).
pub fn with_public_cache(mut response: Response, value: &HeaderValue) -> Response {
    let status = response.status();
    match (
        status.is_success() || status == StatusCode::NOT_MODIFIED,
        response.headers().contains_key(header::CACHE_CONTROL),
    ) {
        (true, false) => {
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, value.clone());
        }
        (false, false) => {
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, CACHE_NO_STORE.clone());
        }
        (_, true) => {}
    }
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
