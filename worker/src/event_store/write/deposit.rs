//! Deposit writes: status, counter, and THB deposit records.

use worker::{D1Database, KvStore};

use crate::event_store::schema::{
    deposit_counter_key, deposit_status_key, thb_deposit_key, thb_deposit_list_key,
};

/// Save deposit status (D1-first, KV fallback for D1-absent deployments).
///
/// When D1 is available (production), writes D1 only and returns — skipping
/// the KV write + `add_to_deposit_list` KV maintenance. The unconditional KV
/// write was burning the free-tier write quota (1,000/day) on every check-in,
/// and all read paths (`get_deposit_status_with_fallback`,
/// `list_deposit_statuses`) are D1-first with KV fallback anyway. Mirrors the
/// `save_thb_deposit` pattern. See issue #053 Phase 3e.
pub async fn save_deposit_status(
    kv: &KvStore,
    status: &event_checkin_domain::models::deposit::DepositStatus,
    d1: Option<&D1Database>,
) -> Result<(), String> {
    if let Some(db) = d1 {
        crate::db::deposit_statuses::save_deposit_status(db, status).await?;
        return Ok(());
    }

    // Legacy KV-only fallback (used only when D1 is not bound).
    let key = deposit_status_key(&status.event_id, &status.attendee_id);
    let json = serde_json::to_string(status)
        .map_err(|e| format!("failed to serialize deposit status: {e}"))?;
    kv.put(&key, &json)
        .map_err(|e| format!("failed to build deposit status put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write deposit status to KV: {e:?}"))?;
    // Add to attendee list for event
    add_to_deposit_list(kv, &status.event_id, &status.attendee_id).await?;
    Ok(())
}

/// Atomically increment and return the new deposit counter value.
/// If the counter doesn't exist yet, creates it starting at 1.
pub async fn increment_deposit_counter(kv: &KvStore, event_id: &str) -> Result<u32, String> {
    let key = deposit_counter_key(event_id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read deposit counter: {e:?}"))?;

    let current: u32 = match raw {
        None => 0,
        Some(s) => s
            .parse::<u32>()
            .map_err(|e| format!("failed to parse deposit counter '{s}': {e}"))?,
    };

    let next = current + 1;
    let val = next.to_string();
    kv.put(&key, &val)
        .map_err(|e| format!("failed to build deposit counter put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write deposit counter: {e:?}"))?;

    Ok(next)
}

/// Save THB deposit record for an attendee (D1-first, KV fallback).
pub async fn save_thb_deposit(
    kv: &KvStore,
    deposit: &event_checkin_domain::models::deposit::ThbDeposit,
    d1: Option<&D1Database>,
) -> Result<(), String> {
    if let Some(db) = d1 {
        // Check if deposit exists → insert or update
        let existing =
            crate::db::thb_deposits::get_thb_deposit(db, &deposit.event_id, &deposit.attendee_id)
                .await?;

        if existing.is_some() {
            crate::db::thb_deposits::update_thb_deposit(db, deposit).await?;
        } else {
            crate::db::thb_deposits::insert_thb_deposit(db, deposit).await?;
        }
        return Ok(());
    }

    let key = thb_deposit_key(&deposit.event_id, &deposit.attendee_id);
    let json = serde_json::to_string(deposit)
        .map_err(|e| format!("failed to serialize THB deposit: {e}"))?;
    kv.put(&key, &json)
        .map_err(|e| format!("failed to build THB deposit put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write THB deposit to KV: {e:?}"))
}

/// Add an attendee_id to the THB deposit list for an event.
async fn add_to_deposit_list(
    kv: &KvStore,
    event_id: &str,
    attendee_id: &str,
) -> Result<(), String> {
    let list_key = thb_deposit_list_key(event_id);
    let raw: Option<String> = kv
        .get(&list_key)
        .text()
        .await
        .map_err(|e| format!("failed to read deposit list: {e:?}"))?;

    let mut ids: Vec<String> = match raw {
        None => vec![],
        Some(json) => {
            serde_json::from_str(&json).map_err(|e| format!("failed to parse deposit list: {e}"))?
        }
    };

    if !ids.iter().any(|id| id == attendee_id) {
        ids.push(attendee_id.to_string());
        let json = serde_json::to_string(&ids)
            .map_err(|e| format!("failed to serialize deposit list: {e}"))?;
        kv.put(&list_key, &json)
            .map_err(|e| format!("failed to build deposit list put: {e:?}"))?
            .execute()
            .await
            .map_err(|e| format!("failed to write deposit list to KV: {e:?}"))?;
    }

    Ok(())
}
