//! Event index and per-event config writes, plus D1 dual-write sync.

use worker::KvStore;

use event_checkin_domain::models::event::{EventConfig, EventIndex};

use crate::event_store::schema::event_config_key;

// ---------------------------------------------------------------------------
// Event index writes
// ---------------------------------------------------------------------------

/// Write the event index to KV.
pub async fn save_event_index(kv: &KvStore, index: &EventIndex) -> Result<(), String> {
    let json_str = serde_json::to_string(index)
        .map_err(|e| format!("failed to serialize event index: {e:?}"))?;
    kv.put("events", &json_str)
        .map_err(|e| format!("failed to build event index put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event index to KV: {e:?}"))
}

// ---------------------------------------------------------------------------
// Per-event config writes
// ---------------------------------------------------------------------------

/// Write a single event's full configuration.
pub async fn save_event_config(kv: &KvStore, config: &EventConfig) -> Result<(), String> {
    let key = event_config_key(&config.id);
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize event config: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build event config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event config to KV: {e:?}"))
}

/// Dual-write: persist event config to D1 alongside KV.
/// Non-blocking — errors are logged, not propagated, so KV remains the source of truth.
pub async fn sync_event_to_d1(d1: Option<&worker::D1Database>, config: &EventConfig) {
    if let Some(db) = d1
        && let Err(e) = crate::db::events::upsert_event(db, config).await
    {
        tracing::warn!(event_id = %config.id, error = %e, "D1 event dual-write failed");
    }
}

/// Dual-write: delete event from D1 alongside KV.
pub async fn sync_delete_event_from_d1(d1: Option<&worker::D1Database>, event_id: &str) {
    if let Some(db) = d1
        && let Err(e) = crate::db::events::delete_event(db, event_id).await
    {
        tracing::warn!(event_id = %event_id, error = %e, "D1 event delete failed");
    }
}
