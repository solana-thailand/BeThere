//! Typed application error enum for consistent error handling across handlers.
//! Replaces `Result<T, String>` with `Result<T, AppError>` for proper error
//! classification and HTTP status code mapping.

use std::fmt;

/// Application-level error with HTTP status code mapping.
#[derive(Debug)]
pub enum AppError {
    /// Resource not found (404)
    NotFound(String),
    /// Authentication required or failed (401)
    Unauthorized(String),
    /// Authenticated but lacks permission (403)
    Forbidden(String),
    /// Input validation failed (400)
    Validation(String),
    /// External service error (502)
    External {
        service: String,
        status: u16,
        body: String,
    },
    /// Rate limit exceeded (429)
    RateLimited(String),
    /// Internal server error (500)
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Unauthorized(msg) => write!(f, "unauthorized: {msg}"),
            Self::Forbidden(msg) => write!(f, "forbidden: {msg}"),
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::External {
                service,
                status,
                body,
            } => {
                write!(
                    f,
                    "external service error: {service} returned {status}: {body}"
                )
            }
            Self::RateLimited(msg) => write!(f, "rate limited: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        Self::Internal(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        Self::Internal(s.to_string())
    }
}

impl AppError {
    /// Map this error to an HTTP status code.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::Validation(_) => 400,
            Self::External { .. } => 502,
            Self::RateLimited(_) => 429,
            Self::Internal(_) => 500,
        }
    }

    /// Check if this is a not-found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    /// Check if this is an auth error (unauthorized or forbidden).
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Self::Unauthorized(_) | Self::Forbidden(_))
    }
}

/// Helper trait to convert `Result<T, String>` to `Result<T, AppError>`.
pub trait IntoAppError<T> {
    fn map_to_app(self) -> Result<T, AppError>;
    fn not_found(self) -> Result<T, AppError>;
    fn unauthorized(self) -> Result<T, AppError>;
    fn forbidden(self) -> Result<T, AppError>;
    fn validation(self) -> Result<T, AppError>;
}

impl<T> IntoAppError<T> for Result<T, String> {
    fn map_to_app(self) -> Result<T, AppError> {
        self.map_err(AppError::Internal)
    }
    fn not_found(self) -> Result<T, AppError> {
        self.map_err(AppError::NotFound)
    }
    fn unauthorized(self) -> Result<T, AppError> {
        self.map_err(AppError::Unauthorized)
    }
    fn forbidden(self) -> Result<T, AppError> {
        self.map_err(AppError::Forbidden)
    }
    fn validation(self) -> Result<T, AppError> {
        self.map_err(AppError::Validation)
    }
}
