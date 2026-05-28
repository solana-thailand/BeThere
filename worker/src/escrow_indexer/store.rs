//! KV storage operations for on-chain events.

use worker::KvStore;

use super::{MAX_ONCHAIN_EVENTS, OnChainEvent};

/// Read on-chain events for an event.
pub async fn read_onchain_events(kv: &KvStore, event_id: &str) -> Vec<OnChainEvent> {
    let key = format!("event:{event_id}:onchain");
    let raw: Option<String> = match kv.get(&key).text().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(key, "onchain events KV read failed: {e:?}");
            return Vec::new();
        }
    };

    match raw {
        None => Vec::new(),
        Some(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            tracing::warn!(key, "onchain events parse failed: {e:?}");
            Vec::new()
        }),
    }
}

/// Read on-chain events for an event, newest first (up to `limit`).
pub async fn get_onchain_events(kv: &KvStore, event_id: &str, limit: usize) -> Vec<OnChainEvent> {
    let mut events = read_onchain_events(kv, event_id).await;
    events.reverse();
    events.into_iter().take(limit).collect()
}

/// Save an on-chain event, deduplicating by signature.
pub async fn save_onchain_event(
    kv: &KvStore,
    event_id: &str,
    event: OnChainEvent,
) -> Result<bool, String> {
    // Check dedup
    let dedup_key = format!("onchain:sig:{}", event.signature);
    if let Ok(Some(_)) = kv.get(&dedup_key).text().await {
        tracing::debug!(sig = %event.signature, "on-chain event already indexed, skipping");
        return Ok(false);
    }

    // Append to per-event list
    let key = format!("event:{event_id}:onchain");
    let mut events = read_onchain_events(kv, event_id).await;
    events.push(event);

    // FIFO trim
    let start = events.len().saturating_sub(MAX_ONCHAIN_EVENTS);
    let trimmed = &events[start..];
    let trimmed_vec: Vec<_> = trimmed.to_vec();

    let json = serde_json::to_string(&trimmed_vec)
        .map_err(|e| format!("onchain events serialize failed: {e:?}"))?;

    // Write events
    kv.put(&key, &json)
        .map_err(|e| format!("onchain events put failed: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("onchain events write failed: {e:?}"))?;

    // Mark dedup (TTL handled by cleanup cron)
    let _ = kv
        .put(&dedup_key, "1")
        .map_err(|e| format!("dedup put failed: {e:?}"))?
        .execute()
        .await;

    Ok(true)
}

/// Save the polling cursor (last processed signature) for an escrow address.
pub async fn save_cursor(
    kv: &KvStore,
    escrow_address: &str,
    signature: &str,
) -> Result<(), String> {
    let key = format!("onchain:cursor:{escrow_address}");
    kv.put(&key, signature)
        .map_err(|e| format!("cursor put failed: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("cursor write failed: {e:?}"))
}

/// Read the polling cursor for an escrow address.
#[allow(dead_code)]
pub async fn read_cursor(kv: &KvStore, escrow_address: &str) -> Option<String> {
    let key = format!("onchain:cursor:{escrow_address}");
    kv.get(&key).text().await.ok().flatten()
}
