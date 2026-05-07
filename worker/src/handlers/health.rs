use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};

use crate::state::AppState;

/// Health check endpoint.
/// Returns basic service status information including Solana cluster.
pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let cluster = if state.config.solana.rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };
    Json(json!({
        "status": "ok",
        "service": "event-checkin",
        "runtime": "cloudflare-workers",
        "version": env!("CARGO_PKG_VERSION"),
        "cluster": cluster,
    }))
}
