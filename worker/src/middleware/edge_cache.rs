//! Edge cache for anonymous public GETs (`.plans/028` W4).
//!
//! Serves `/api/public/events` and `/api/public/event/{slug}` from the
//! Cloudflare Cache API for 30 s ([`EDGE_TTL`]), so a landing-page burst reads
//! D1/KV once per cache node instead of once per visitor. Probed on
//! `*.workers.dev` on 2026-09-29: it works, with about two misses per key per
//! colo per TTL (two cache nodes behind one colo).
//!
//! Rules, all of them required:
//! - Only anonymous `GET`s are looked up or stored. A request that carries a
//!   session token (bearer or cookie) always goes to the handler, so a member's
//!   view of a private event is never keyed, and an organizer sees an edit at
//!   once.
//! - Only a `200` that the handler left without `Cache-Control` and without
//!   `Set-Cookie` is stored: the same "public 2xx" rule as
//!   [`super::cache::with_public_cache`]. A 401/403/404 is never pinned.
//! - The key is scheme + host + path. The handlers ignore the query string, so
//!   dropping it keeps random `?x=` from busting the cache.
//! - There is no purge on event writes: `cache.delete` reaches one data centre
//!   only, and one colo has two nodes. The short TTL is the bound instead.
//!
//! The stored copy carries `public, max-age=30` for the Cache API. On a hit
//! that header is removed so the route's own layer stamps the client policy,
//! exactly as on a miss.

use axum::{
    body::Body,
    extract::{OriginalUri, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Response header naming the outcome, so a staging rehearsal can count hits.
pub const EDGE_CACHE_HEADER: &str = "x-edge-cache";

/// Lifetime of the stored copy: how long a public payload may be served from the edge.
const EDGE_TTL: HeaderValue = HeaderValue::from_static("public, max-age=30");
const HIT: HeaderValue = HeaderValue::from_static("HIT");
const MISS: HeaderValue = HeaderValue::from_static("MISS");

/// Cache key for a request, or `None` when the request must bypass the cache
/// (not a `GET`, carries a session token, or has no `Host`).
pub fn edge_cache_key(method: &Method, headers: &HeaderMap, path: &str) -> Option<String> {
    if method != Method::GET || crate::auth::extract_token_from_headers(headers).is_some() {
        return None;
    }
    let host = headers.get(header::HOST)?.to_str().ok()?;
    Some(format!("https://{host}{path}"))
}

/// Whether a handler response may be stored in the shared edge cache.
pub fn is_edge_storable(status: StatusCode, headers: &HeaderMap) -> bool {
    status == StatusCode::OK
        && !headers.contains_key(header::CACHE_CONTROL)
        && !headers.contains_key(header::SET_COOKIE)
}

/// Route layer: look up, else run the handler and store in `wait_until`.
#[worker::send]
pub async fn edge_cache_layer(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // No fetch context outside the Workers runtime (native tests): no cache.
    let Some(ctx) = state.worker_ctx.clone() else {
        return next.run(req).await;
    };
    let path = req
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| req.uri().path(), |uri| uri.0.path())
        .to_owned();
    let Some(key) = edge_cache_key(req.method(), req.headers(), &path) else {
        return next.run(req).await;
    };

    let cache = worker::Cache::default();
    match cache.get(key.as_str(), false).await {
        Ok(Some(hit)) => {
            let mut response = match worker::HttpResponse::try_from(hit) {
                Ok(hit) => hit.map(Body::new),
                Err(e) => {
                    tracing::warn!(error = %e, path = %path, "edge cache hit unreadable");
                    return next.run(req).await;
                }
            };
            let headers = response.headers_mut();
            headers.remove(header::CACHE_CONTROL);
            headers.insert(EDGE_CACHE_HEADER, HIT);
            tracing::info!(edge_cache = "hit", path = %path, "edge cache");
            return response;
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, path = %path, "edge cache lookup failed"),
    }

    let response = next.run(req).await;
    if !is_edge_storable(response.status(), response.headers()) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!(error = %e, path = %path, "edge cache could not buffer body");
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .expect("static 500 response is always valid");
        }
    };

    let mut stored = Response::new(Body::from(bytes.clone()));
    *stored.status_mut() = parts.status;
    *stored.headers_mut() = parts.headers.clone();
    stored.headers_mut().insert(header::CACHE_CONTROL, EDGE_TTL);
    match worker::Response::try_from(stored) {
        Ok(stored) => ctx.wait_until(async move {
            if let Err(e) = cache.put(key.as_str(), stored).await {
                tracing::warn!(error = %e, "edge cache put failed");
            }
        }),
        Err(e) => tracing::warn!(error = %e, "edge cache could not convert response"),
    }

    tracing::info!(edge_cache = "miss", path = %path, "edge cache");
    parts.headers.insert(EDGE_CACHE_HEADER, MISS);
    Response::from_parts(parts, Body::from(bytes))
}
