use axum::extract::State;
use axum::response::Json;
use serde_json::{Value, json};

use crate::state::AppState;

/// Health check endpoint.
/// Returns service status, Solana cluster, and D1 database connectivity.
#[worker::send]
pub async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let cluster = if state.config.solana.rpc_url.contains("mainnet") {
        "mainnet-beta"
    } else {
        "devnet"
    };

    // D1 connectivity check — runs a lightweight COUNT query.
    // Wrapped in `worker::send` compatible future.
    let d1_status = match state.d1 {
        Some(db) => {
            let db = Arc::clone(&db);
            let result = check_d1_health(&db).await;
            json!({
                "connected": result.is_ok(),
                "counts": result.unwrap_or_default(),
            })
        }
        None => json!({
            "connected": false,
            "error": "D1 binding not configured",
        }),
    };

    Json(json!({
        "status": "ok",
        "service": "event-checkin",
        "runtime": "cloudflare-workers",
        "version": env!("CARGO_PKG_VERSION"),
        "cluster": cluster,
        "dev_mode": state.config.dev_mode,
        "d1": d1_status,
    }))
}

use std::sync::Arc;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct D1Counts {
    attendees: i64,
    contacts: i64,
    events: i64,
    staff: i64,
    claim_locks: i64,
    audit_log: i64,
}

async fn check_d1_health(db: &worker::D1Database) -> Result<D1Counts, String> {
    let stmt = db.prepare(
        "SELECT \
         (SELECT COUNT(*) FROM attendees) as attendees, \
         (SELECT COUNT(*) FROM contacts) as contacts, \
         (SELECT COUNT(*) FROM events) as events, \
         (SELECT COUNT(*) FROM staff) as staff, \
         (SELECT COUNT(*) FROM claim_locks) as claim_locks, \
         (SELECT COUNT(*) FROM audit_log) as audit_log",
    );
    let row = stmt
        .first::<D1Counts>(None)
        .await
        .map_err(|e| format!("D1 health query: {e:?}"))?;

    Ok(row.unwrap_or_default())
}
