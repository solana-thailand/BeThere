use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};

use crate::state::AppState;

/// Health check endpoint.
/// Returns basic service status information.
pub async fn health_check(State(_state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "event-checkin",
        "runtime": "cloudflare-workers",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
