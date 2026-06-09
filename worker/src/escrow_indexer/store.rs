//! D1 storage operations for on-chain events (Phase 3b — Issue #053).
//!
//! On-chain events, dedup markers, and polling cursors live in D1 only.
//! KV fallback and dual-write have been removed.

use worker::D1Database;

use super::OnChainEvent;
use crate::db::onchain_events as d1;

/// Read on-chain events for an event, newest first (up to `limit`).
pub async fn get_onchain_events(
    db: &D1Database,
    event_id: &str,
    limit: usize,
) -> Vec<OnChainEvent> {
    match d1::get_onchain_events_from_d1(db, event_id, limit).await {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(event_id, "onchain events D1 read failed: {e:?}");
            Vec::new()
        }
    }
}

/// Save an on-chain event, deduplicating by signature.
///
/// Returns `Ok(true)` if the event was new, `Ok(false)` if it was a duplicate.
pub async fn save_onchain_event(
    db: &D1Database,
    event_id: &str,
    event: OnChainEvent,
) -> Result<bool, String> {
    d1::insert_onchain_event_to_d1(db, event_id, &event).await
}

/// Save the polling cursor (last processed signature) for an escrow address.
pub async fn save_cursor(
    db: &D1Database,
    escrow_address: &str,
    signature: &str,
) -> Result<(), String> {
    d1::save_cursor_to_d1(db, escrow_address, signature).await
}

/// Read the polling cursor for an escrow address.
#[allow(dead_code)]
pub async fn read_cursor(db: &D1Database, escrow_address: &str) -> Option<String> {
    d1::read_cursor_from_d1(db, escrow_address)
        .await
        .ok()
        .flatten()
}
