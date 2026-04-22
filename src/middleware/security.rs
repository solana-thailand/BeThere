use axum::{
    extract::Request,
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};
use std::sync::LazyLock;

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
             script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' https://unpkg.com https://cdn.jsdelivr.net; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: blob:; \
             media-src 'self' blob:; \
             connect-src 'self'; \
             font-src 'self'; \
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
/// Headers added:
/// - `Strict-Transport-Security` (HSTS) — enforce HTTPS for 2 years
/// - `X-Content-Type-Options` — prevent MIME type sniffing
/// - `X-Frame-Options` — prevent clickjacking (DENY)
/// - `X-XSS-Protection` — disabled (modern browsers use CSP instead)
/// - `Referrer-Policy` — limit referrer info to origin only
/// - `Content-Security-Policy` — restrict resource loading
/// - `Permissions-Policy` — limit browser feature access
/// - `Cross-Origin-Opener-Policy` — isolate window origin
/// - `Cross-Origin-Resource-Policy` — prevent cross-origin resource leaks
pub async fn security_headers_layer(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    add_security_headers(response)
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

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request as HttpRequest, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    use super::security_headers_layer;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_security_headers_present() {
        let app = Router::new()
            .route("/test", get(ok_handler))
            .layer(axum::middleware::from_fn(security_headers_layer));

        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let headers = response.headers();

        assert_eq!(
            headers.get("strict-transport-security").unwrap(),
            "max-age=63072000; includeSubDomains; preload"
        );
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(headers.get("x-xss-protection").unwrap(), "0");
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(headers.get("content-security-policy").is_some());
        assert!(headers.get("permissions-policy").is_some());
        assert_eq!(
            headers.get("cross-origin-opener-policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            headers.get("cross-origin-resource-policy").unwrap(),
            "same-origin"
        );
    }
}
