//! Axum response integration for `AppError`.
//!
//! Converts typed errors into JSON responses with appropriate HTTP status codes.
//! Uses a newtype wrapper to satisfy Rust's orphan rule for trait implementations.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

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
        let message = self.0.to_string();

        let body = json!({
            "success": false,
            "error": message,
        });

        (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            axum::Json(body),
        )
            .into_response()
    }
}
