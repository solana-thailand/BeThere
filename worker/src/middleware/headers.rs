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
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// The security headers, as `(lowercase name, value)` pairs.
///
/// The single source of truth: `frontend-leptos/_headers` mirrors this table
/// under `/*` for the responses Cloudflare serves asset-first (every HTML
/// navigation once `run_worker_first` became an array, `7ed1c07`), and
/// `tests/security_headers_parity.rs` fails when the two drift.
pub const SECURITY_HEADERS: [(&str, &str); 9] = [
    (
        "strict-transport-security",
        "max-age=63072000; includeSubDomains; preload",
    ),
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "DENY"),
    ("x-xss-protection", "0"),
    ("referrer-policy", "strict-origin-when-cross-origin"),
    (
        "content-security-policy",
        "default-src 'self'; \
         script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://unpkg.com https://cdn.jsdelivr.net https://static.cloudflareinsights.com https://telegram.org; \
         style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
         img-src 'self' data: blob: https:; \
         media-src 'self' blob:; \
         object-src 'none'; \
         frame-src https://www.youtube.com https://www.youtube-nocookie.com https://oauth.telegram.org; \
         connect-src 'self' https: wss:; \
         font-src 'self' https://fonts.gstatic.com; \
         frame-ancestors 'none'; \
         base-uri 'self'; \
         form-action 'self'",
    ),
    (
        "permissions-policy",
        "camera=(self), \
         microphone=(), \
         geolocation=(), \
         payment=()",
    ),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
];

/// Axum middleware that adds security headers to every response.
///
/// Applied as a layer around the entire router so every Worker-served
/// response (API + the paths in `run_worker_first`) includes these headers.
pub async fn security_headers_layer(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    add_security_headers(response)
}

pub fn add_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
