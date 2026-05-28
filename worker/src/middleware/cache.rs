//! Cache-Control middleware — adds caching headers to public endpoints.

use axum::{
    extract::Request,
    http::{HeaderValue, header},
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
