//! Axum response integration for `AppError`.
//!
//! Converts typed errors into JSON responses with appropriate HTTP status codes.
//! Uses a newtype wrapper to satisfy Rust's orphan rule for trait implementations.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use event_checkin_domain::models::api::ApiResponse;
use event_checkin_domain::models::error::AppError;

/// Newtype wrapper around `AppError` that implements `IntoResponse`.
///
/// Use `.into()` or `?` operator (via `From`) to convert `AppError` into this type
/// at handler boundaries.
pub struct WorkerError(pub AppError);

impl From<AppError> for WorkerError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}

impl IntoResponse for WorkerError {
    fn into_response(self) -> Response {
        let status = self.0.status_code();
        // Two renderings, deliberately. `Display` is the operator-facing detail
        // and goes to the log only; `public_message` is the caller-facing body.
        // Issue 078: they used to be the same string, which handed the upstream
        // endpoint URL and its verbatim response to unauthenticated callers of
        // `GET /api/claim/{token}`. The `x-correlation-id` response header set by
        // `middleware::correlation` is what ties the opaque body back to the log
        // line that still carries the detail.
        let detail = self.0.to_string();
        let public = self.0.public_message();

        // Central failure capture: previously a 5xx was only visible in logs if
        // the handler happened to call tracing::error! itself, so server-side
        // failures could be entirely silent. Surface every 5xx at error level so
        // `wrangler tail` / Logpush can see and alert on them. (4xx are expected
        // client errors and stay quiet to avoid log noise.) The correlation id for
        // this request is on the response header + the middleware completion log.
        if status >= 500 {
            tracing::error!(status, error = %detail, "request failed (server error)");
        }

        let body = ApiResponse::<()> {
            success: false,
            data: None,
            error: Some(public.into_owned()),
            correlation_id: None,
        };

        (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            axum::Json(body),
        )
            .into_response()
    }
}

/// Newtype wrapper around `ApiResponse<T>` that implements `IntoResponse`.
///
/// Handlers return `ApiOk(ApiResponse::data(t))` or equivalently
/// `ApiOk::new(t)` which wraps the value in the standard envelope.
pub struct ApiOk<T: serde::Serialize>(pub ApiResponse<T>);

impl<T: serde::Serialize> ApiOk<T> {
    /// Convenience: wrap data in a success response.
    pub fn new(data: T) -> Self {
        Self(ApiResponse::data(data))
    }
}

impl<T: serde::Serialize> IntoResponse for ApiOk<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}
