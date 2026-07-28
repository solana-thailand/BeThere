//! Event lifecycle: archive, restore, hard-delete.

use worker::KvStore;

use event_checkin_domain::models::event::EventStatus;

use crate::event_store::read as read_mod;
use crate::event_store::read::get_event_index;

use super::escrow::delete_escrow_index;
use super::index::{save_event_config, save_event_index};

/// Archive (soft-delete) an event by setting its status to Archived.
///
/// SEC-004: Rejects archive if the event has an active on-chain escrow.
/// Archive the event first on-chain (close escrow) before archiving here.
pub async fn archive_event(kv: &KvStore, id: &str) -> Result<(), String> {
    let mut config = read_mod::get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    // SEC-004: Block archive if escrow is active on-chain
    if config.escrow_status.is_active() {
        return Err(format!(
            "cannot archive event with active on-chain escrow (status: {}) — close escrow first",
            config.escrow_status
        ));
    }

    config.status = EventStatus::Archived;
    config.updated_at = chrono::Utc::now().to_rfc3339();

    save_event_config(kv, &config).await?;

    // Update index
    let mut index = get_event_index(kv).await?;
    if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
        entry.status = EventStatus::Archived;
    }
    save_event_index(kv, &index).await?;

    tracing::info!(event_id = %id, "event archived");

    Ok(())
}

/// Restore (unarchive) an event by setting its status back to Draft.
///
/// Only works on Archived events. This is the reverse of `archive_event`.
pub async fn restore_event(kv: &KvStore, id: &str) -> Result<(), String> {
    let mut config = read_mod::get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    if config.status != EventStatus::Archived {
        return Err(format!(
            "event '{id}' is not archived (current status: {}) — only archived events can be restored",
            config.status.as_str()
        ));
    }

    config.status = EventStatus::Draft;
    config.updated_at = chrono::Utc::now().to_rfc3339();

    save_event_config(kv, &config).await?;

    // Update index
    let mut index = get_event_index(kv).await?;
    if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
        entry.status = EventStatus::Draft;
    }
    save_event_index(kv, &index).await?;

    tracing::info!(event_id = %id, "event restored from archive");

    Ok(())
}

/// Hard-delete an event: remove config from KV and remove from index.
/// This frees up the slug for reuse.
///
/// When `force` is true, allows deleting Draft events and bypasses the escrow guard.
/// Intended for devnet cleanup of test events. SuperAdmin-gated at the handler level.
pub async fn hard_delete_event(kv: &KvStore, id: &str, force: bool) -> Result<(), String> {
    let config = read_mod::get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    if force {
        // Force mode: allow Draft + Archived, skip escrow guard
        if !matches!(config.status, EventStatus::Archived | EventStatus::Draft) {
            return Err(format!(
                "event '{id}' must be Draft or Archived to force-delete (current status: {}) — deactivate/close event first",
                config.status.as_str()
            ));
        }
        if !config.escrow_address.is_empty() {
            tracing::warn!(
                event_id = %id,
                escrow = %config.escrow_address,
                "force-deleting event with active escrow — on-chain account will be orphaned"
            );
        }
    } else {
        // Normal mode: Archived only, escrow guard enforced
        if config.status != EventStatus::Archived {
            return Err(format!(
                "event '{id}' must be archived before deletion (current status: {}) — archive it first",
                config.status.as_str()
            ));
        }

        // SEC-004: Block delete if escrow is active on-chain
        if config.escrow_status.is_active() {
            return Err(format!(
                "cannot delete event with active on-chain escrow (status: {}) — close escrow first",
                config.escrow_status
            ));
        }
    }

    // Remove config from KV
    let config_key = format!("event:{id}");
    kv.delete(&config_key)
        .await
        .map_err(|e| format!("failed to delete event config: {e:?}"))?;

    // Clean up escrow reverse index (H7)
    if !config.escrow_address.is_empty() {
        let _ = delete_escrow_index(None, Some(kv), &config.escrow_address).await;
    }

    // Remove from index
    let mut index = get_event_index(kv).await?;
    let before = index.events.len();
    index.events.retain(|e| e.id != id);
    if index.events.len() == before {
        tracing::warn!(event_id = %id, "event was in KV but not in index");
    }
    save_event_index(kv, &index).await?;

    tracing::info!(event_id = %id, "event hard-deleted");

    Ok(())
}
