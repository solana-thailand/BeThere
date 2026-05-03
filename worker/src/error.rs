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
        let message = self.0.to_string();

        let body = ApiResponse::<()> {
            success: false,
            data: None,
            error: Some(message),
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
