//! Read operations: getters, listers, resolvers, and access-check helpers.

use worker::KvStore;

use event_checkin_domain::models::event::{
    EventConfig, EventIndex, EventMeta, EventStatus,
};

use super::schema::{event_config_key, escrow_index_key, deposit_status_key, thb_deposit_key, thb_deposit_list_key};

// ---------------------------------------------------------------------------
// Event index
// ---------------------------------------------------------------------------

/// Read the event index from KV.
/// Returns an empty index if the key doesn't exist yet (first run).
pub async fn get_event_index(kv: &KvStore) -> Result<EventIndex, String> {
    let raw: Option<String> = kv
        .get("events")
        .text()
        .await
        .map_err(|e| format!("failed to read event index from KV: {e:?}"))?;

    match raw {
        None => Ok(EventIndex::default()),
        Some(json_str) => {
            serde_json::from_str(&json_str).map_err(|e| format!("failed to parse event index: {e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Per-event config reads
// ---------------------------------------------------------------------------

/// Read a single event's full configuration.
/// Returns `None` if the event ID doesn't exist.
pub async fn get_event_config(kv: &KvStore, id: &str) -> Result<Option<EventConfig>, String> {
    let key = event_config_key(id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read event config '{id}' from KV: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json_str) => serde_json::from_str(&json_str)
            .map(Some)
            .map_err(|e| format!("failed to parse event config '{id}': {e}")),
    }
}

// ---------------------------------------------------------------------------
// Escrow index reads
// ---------------------------------------------------------------------------

/// Look up an event ID by its escrow PDA address via the reverse index.
/// Returns `None` if the escrow address is not indexed.
pub async fn get_event_id_by_escrow(kv: &KvStore, escrow_address: &str) -> Option<String> {
    let key = escrow_index_key(escrow_address);
    kv.get(&key).text().await.ok().flatten()
}

// ---------------------------------------------------------------------------
// CRUD reads
// ---------------------------------------------------------------------------

/// List all events (metadata only, no full config).
/// Returns events sorted by creation date (newest first).
pub async fn list_events(kv: &KvStore) -> Result<Vec<EventMeta>, String> {
    let index = get_event_index(kv).await?;
    Ok(index.events)
}

/// Get a single event's full configuration by ID.
pub async fn get_event(kv: &KvStore, id: &str) -> Result<Option<EventConfig>, String> {
    get_event_config(kv, id).await
}

// ---------------------------------------------------------------------------
// Event resolution
// ---------------------------------------------------------------------------

/// Find the first active event.
///
/// Used for backward compatibility: legacy API routes that don't specify
/// an event_id resolve to the first active event.
pub async fn get_active_event(kv: &KvStore) -> Result<Option<EventConfig>, String> {
    let index = get_event_index(kv).await?;
    for meta in &index.events {
        if meta.status == EventStatus::Active
            && let Some(config) = get_event_config(kv, &meta.id).await?
        {
            return Ok(Some(config));
        }
    }
    Ok(None)
}

/// Resolve an event ID to its full configuration.
///
/// Falls back to the first active event if `event_id` is empty or "default".
/// Returns an error if no matching event is found.
pub async fn resolve_event(kv: &KvStore, event_id: Option<&str>) -> Result<EventConfig, String> {
    match event_id {
        Some(id) if !id.is_empty() => get_event_config(kv, id)
            .await?
            .ok_or_else(|| format!("event '{id}' not found")),
        _ => {
            // Fall back to first active event
            get_active_event(kv)
                .await?
                .ok_or_else(|| "no active event found — create an event first".to_string())
        }
    }
}

/// Resolve an event, falling back to global config if EVENTS KV is not available.
///
/// This is the main entry point for handlers:
/// - If `events_kv` is `Some` → resolve event from KV (by ID or first active)
/// - If `events_kv` is `None` → build synthetic EventConfig from global env vars
pub async fn resolve_event_or_fallback(
    events_kv: Option<&KvStore>,
    event_id: Option<&str>,
    global: &event_checkin_domain::config::AppConfig,
) -> Result<EventConfig, String> {
    match events_kv {
        Some(kv) => resolve_event(kv, event_id).await,
        None => {
            let d = &global.event_defaults;
            Ok(EventConfig::from_global_config(
                &d.name,
                &d.tagline,
                &d.link,
                d.start_ms,
                d.end_ms,
                &global.sheets.sheet_id,
                &global.sheets.sheet_name,
                &global.sheets.staff_sheet_name,
                &global.nft.collection_mint,
                &global.nft.metadata_uri,
                &global.nft.image_url,
                "", // nft_symbol — not in global config
                global.staff_emails.iter().cloned().collect::<Vec<String>>(), // organizer_emails — use staff_emails for legacy
                Vec::new(),                                                   // staff_emails
                &global.server.claim_base_url,
                "", // merkle_tree — not in global config
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Access checks (pure functions, no I/O)
// ---------------------------------------------------------------------------

/// Check if a user is an organizer for a specific event.
pub fn is_event_organizer(config: &EventConfig, email: &str) -> bool {
    config
        .organizer_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
}

/// Check if a user is staff for a specific event.
pub fn is_event_staff(config: &EventConfig, email: &str) -> bool {
    config
        .staff_emails
        .iter()
        .any(|e| e.eq_ignore_ascii_case(email))
}

/// Check if a user has any access (organizer or staff) to a specific event.
pub fn has_event_access(config: &EventConfig, email: &str) -> bool {
    is_event_organizer(config, email) || is_event_staff(config, email)
}

// ---------------------------------------------------------------------------
// Deposit reads
// ---------------------------------------------------------------------------

/// Get deposit status for an attendee.
pub async fn get_deposit_status(
    kv: &KvStore,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<event_checkin_domain::models::deposit::DepositStatus>, String> {
    let key = deposit_status_key(event_id, attendee_id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read deposit status: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| format!("failed to parse deposit status: {e}"))
            .map(Some),
    }
}

/// List all deposit statuses for an event using KV list with prefix.
/// Returns all DepositStatus records found under `event:{id}:deposit:status:*`.
pub async fn list_deposit_statuses(
    kv: &KvStore,
    event_id: &str,
) -> Result<Vec<event_checkin_domain::models::deposit::DepositStatus>, String> {
    let prefix = format!("event:{event_id}:deposit:status:");
    let mut deposits = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut builder = kv.list().prefix(prefix.clone());
        if let Some(c) = cursor.take() {
            builder = builder.cursor(c);
        }

        let resp = builder.execute().await.map_err(|e| {
            format!("failed to list deposit statuses for event '{event_id}': {e:?}")
        })?;

        for key in &resp.keys {
            let raw: Option<String> = kv
                .get(&key.name)
                .text()
                .await
                .map_err(|e| format!("failed to read deposit status '{}': {e:?}", key.name))?;

            if let Some(json) = raw {
                match serde_json::from_str(&json) {
                    Ok(deposit) => deposits.push(deposit),
                    Err(e) => {
                        tracing::warn!(key = %key.name, error = %e, "skipping malformed deposit status");
                    }
                }
            }
        }

        if resp.list_complete {
            break;
        }
        cursor = resp.cursor;
    }

    Ok(deposits)
}

/// Get THB deposit record for an attendee.
pub async fn get_thb_deposit(
    kv: &KvStore,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<event_checkin_domain::models::deposit::ThbDeposit>, String> {
    let key = thb_deposit_key(event_id, attendee_id);
    let raw: Option<String> = kv
        .get(&key)
        .text()
        .await
        .map_err(|e| format!("failed to read THB deposit: {e:?}"))?;

    match raw {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| format!("failed to parse THB deposit: {e}"))
            .map(Some),
    }
}

/// List all THB deposits for an event.
pub async fn list_thb_deposits(
    kv: &KvStore,
    event_id: &str,
) -> Result<Vec<event_checkin_domain::models::deposit::ThbDeposit>, String> {
    let list_key = thb_deposit_list_key(event_id);
    let raw: Option<String> = kv
        .get(&list_key)
        .text()
        .await
        .map_err(|e| format!("failed to read THB deposit list: {e:?}"))?;

    let ids: Vec<String> = match raw {
        None => return Ok(vec![]),
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| format!("failed to parse THB deposit list: {e}"))?,
    };

    let mut deposits = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(deposit) = get_thb_deposit(kv, event_id, &id).await? {
            deposits.push(deposit);
        }
    }

    Ok(deposits)
}

/// Find attendee API ID by wallet address within a specific event's deposit records.
///
/// Scans `DepositStatus` entries for the event and matches on `wallet_address`.
/// Returns the first matching `attendee_id` (API ID from Sheets).
pub async fn find_attendee_by_wallet(
    kv: &KvStore,
    event_id: &str,
    wallet_address: &str,
) -> Result<Option<String>, String> {
    if wallet_address.is_empty() {
        return Ok(None);
    }
    let deposits = list_deposit_statuses(kv, event_id).await?;
    for d in &deposits {
        if d.wallet_address.as_deref() == Some(wallet_address) {
            return Ok(Some(d.attendee_id.clone()));
        }
    }
    Ok(None)
}
