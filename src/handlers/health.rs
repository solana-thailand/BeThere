use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};

use crate::config::AppState;

/// Health check endpoint.
/// Returns basic server status information.
pub async fn health_check(State(_state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "event-checkin",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
