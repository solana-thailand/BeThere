//! Correlation ID middleware — request-scoped tracing identifier.

use axum::{extract::Request, http::HeaderName, middleware::Next, response::Response};
use std::ops::Deref;
use uuid::Uuid;

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
