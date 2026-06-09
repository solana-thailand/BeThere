//! Read operations: getters, listers, resolvers, and access-check helpers.

use worker::{D1Database, KvStore};

use event_checkin_domain::models::event::{EventConfig, EventIndex, EventMeta, EventStatus};

use super::schema::{
    deposit_status_key, escrow_index_key, event_config_key, thb_deposit_key, thb_deposit_list_key,
};

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

/// Look up an event ID by its escrow PDA address — D1 first, KV fallback.
/// Returns `None` if the escrow address is not indexed.
pub async fn get_event_id_by_escrow(
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
    escrow_address: &str,
) -> Option<String> {
    // D1 first
    if let Some(db) = d1 {
        match crate::db::escrow_index::get_event_id_by_escrow_from_d1(db, escrow_address).await {
            Ok(Some(event_id)) => return Some(event_id),
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(escrow_address, error = %e, "D1 escrow index lookup failed, falling back to KV");
            }
        }
    }

    // KV fallback
    if let Some(kv_ref) = kv {
        let key = escrow_index_key(escrow_address);
        return kv_ref.get(&key).text().await.ok().flatten();
    }

    None
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

/// Resolve an event, trying KV → D1 → global config fallback.
///
/// This is the main entry point for handlers:
/// 1. If `events_kv` is `Some` → resolve event from KV (by ID or first active)
/// 2. If KV miss → try D1 (by ID or first active)
/// 3. If `events_kv` is `None` and D1 miss → build synthetic EventConfig from global env vars
pub async fn resolve_event_or_fallback(
    events_kv: Option<&KvStore>,
    event_id: Option<&str>,
    global: &event_checkin_domain::config::AppConfig,
    d1: Option<&worker::D1Database>,
) -> Result<EventConfig, String> {
    match events_kv {
        Some(kv) => {
            let result = resolve_event(kv, event_id).await;
            match result {
                Ok(config) => Ok(config),
                Err(kv_err) => {
                    // KV miss — try D1 before giving up
                    if let Some(db) = d1 {
                        let d1_result = resolve_event_from_d1(db, event_id).await;
                        if let Some(config) = d1_result? {
                            tracing::info!(
                                event_id = %config.id,
                                "D1 fallback: recovered event not found in KV"
                            );
                            // Rebuild KV from D1 data
                            super::write::save_event_config(kv, &config).await.ok();
                            return Ok(config);
                        }
                    }
                    Err(kv_err)
                }
            }
        }
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

/// Resolve an event from D1 only.
async fn resolve_event_from_d1(
    db: &worker::D1Database,
    event_id: Option<&str>,
) -> Result<Option<EventConfig>, String> {
    match event_id {
        Some(id) if !id.is_empty() => crate::db::events::get_event(db, id)
            .await
            .map(|opt| opt.map(|row| row.to_event_config())),
        _ => crate::db::events::get_active_event(db)
            .await
            .map(|opt| opt.map(|row| row.to_event_config())),
    }
}

// ---------------------------------------------------------------------------
// Slug-based resolution (KV → D1 fallback)
// ---------------------------------------------------------------------------

/// Resolve an event by slug, trying KV index → D1 fallback.
///
/// 1. If `events_kv` is `Some` → scan KV index for slug → load full config
/// 2. If KV miss or unavailable → try D1 `get_event_by_slug`
pub async fn resolve_event_by_slug(
    events_kv: Option<&KvStore>,
    slug: &str,
    d1: Option<&worker::D1Database>,
) -> Result<EventConfig, String> {
    // Try KV first
    if let Some(kv) = events_kv {
        let index = get_event_index(kv).await?;
        if let Some(meta) = index.events.iter().find(|e| e.slug == slug)
            && let Ok(Some(config)) = get_event_config(kv, &meta.id).await
        {
            return Ok(config);
        }
    }

    // D1 fallback
    if let Some(db) = d1
        && let Some(row) = crate::db::events::get_event_by_slug(db, slug).await?
    {
        tracing::info!(%slug, event_id = %row.id.clone().unwrap_or_default(), "resolved event by slug from D1");
        return Ok(row.to_event_config());
    }

    Err(format!("event '{slug}' not found"))
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

/// Get deposit status for an attendee (D1-first, KV fallback).
pub async fn get_deposit_status(
    kv: &KvStore,
    event_id: &str,
    attendee_id: &str,
    d1: Option<&D1Database>,
) -> Result<Option<event_checkin_domain::models::deposit::DepositStatus>, String> {
    if let Some(db) = d1 {
        return crate::db::deposit_statuses::get_deposit_status(db, event_id, attendee_id).await;
    }

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

/// List all deposit statuses for an event (D1-first, KV fallback).
pub async fn list_deposit_statuses(
    kv: &KvStore,
    event_id: &str,
    d1: Option<&D1Database>,
) -> Result<Vec<event_checkin_domain::models::deposit::DepositStatus>, String> {
    if let Some(db) = d1 {
        return crate::db::deposit_statuses::list_deposit_statuses(db, event_id).await;
    }

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

/// Get THB deposit record for an attendee (D1-first, KV fallback).
pub async fn get_thb_deposit(
    kv: &KvStore,
    event_id: &str,
    attendee_id: &str,
    d1: Option<&D1Database>,
) -> Result<Option<event_checkin_domain::models::deposit::ThbDeposit>, String> {
    if let Some(db) = d1 {
        return crate::db::thb_deposits::get_thb_deposit(db, event_id, attendee_id).await;
    }

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

/// List all THB deposits for an event (D1-first, KV fallback).
pub async fn list_thb_deposits(
    kv: &KvStore,
    event_id: &str,
    d1: Option<&D1Database>,
) -> Result<Vec<event_checkin_domain::models::deposit::ThbDeposit>, String> {
    if let Some(db) = d1 {
        return crate::db::thb_deposits::list_thb_deposits(db, event_id).await;
    }

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
        if let Some(deposit) = get_thb_deposit(kv, event_id, &id, None).await? {
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
    d1: Option<&D1Database>,
) -> Result<Option<String>, String> {
    if wallet_address.is_empty() {
        return Ok(None);
    }
    if let Some(db) = d1 {
        return crate::db::deposit_statuses::find_attendee_by_wallet(db, event_id, wallet_address)
            .await;
    }
    let deposits = list_deposit_statuses(kv, event_id, None).await?;
    for d in &deposits {
        if d.wallet_address.as_deref() == Some(wallet_address) {
            return Ok(Some(d.attendee_id.clone()));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// KV-optional fallback reads (P3.2)
// ---------------------------------------------------------------------------

/// Get deposit status with KV → D1 fallback.
///
/// D1-first: queries the dedicated deposit_statuses table.
/// Falls back to KV only if D1 is unavailable.
pub async fn get_deposit_status_with_fallback(
    kv: Option<&KvStore>,
    d1: Option<&D1Database>,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<event_checkin_domain::models::deposit::DepositStatus>, String> {
    // D1 first (dedicated deposit_statuses table)
    if let Some(db) = d1
        && let Some(status) =
            crate::db::deposit_statuses::get_deposit_status(db, event_id, attendee_id).await?
    {
        return Ok(Some(status));
    }

    // KV fallback
    if let Some(kv) = kv {
        let result = get_deposit_status(kv, event_id, attendee_id, None).await?;
        if result.is_some() {
            return Ok(result);
        }
    }

    Ok(None)
}

/// Get event config with KV → D1 fallback.
///
/// When KV is available: try KV first, fall back to D1 on miss.
/// When KV is unavailable: read from D1 events table.
pub async fn get_event_config_with_fallback(
    kv: Option<&KvStore>,
    d1: Option<&D1Database>,
    event_id: &str,
) -> Result<Option<EventConfig>, String> {
    // Try KV first if available
    if let Some(kv) = kv
        && let Some(config) = get_event_config(kv, event_id).await?
    {
        return Ok(Some(config));
    }

    // D1 fallback
    if let Some(db) = d1
        && let Some(row) = crate::db::events::get_event(db, event_id).await?
    {
        return Ok(Some(row.to_event_config()));
    }

    Ok(None)
}

/// Increment deposit counter with KV → D1 fallback.
///
/// D1-first: counts existing deposits in the deposit_statuses table + 1.
pub async fn increment_deposit_counter_with_fallback(
    kv: Option<&KvStore>,
    d1: Option<&D1Database>,
    event_id: &str,
) -> Result<u32, String> {
    if let Some(db) = d1 {
        let count = crate::db::deposit_statuses::count_deposits_by_event(db, event_id).await?;
        return Ok(count + 1);
    }

    if let Some(kv) = kv {
        return super::write::increment_deposit_counter(kv, event_id).await;
    }

    // Neither available — fallback to 1
    Ok(1)
}

/// Save deposit status with D1 primary + KV best-effort.
///
/// Writes to D1 first (dedicated deposit_statuses table), then best-effort to KV.
pub async fn save_deposit_status_with_fallback(
    kv: Option<&KvStore>,
    d1: Option<&D1Database>,
    status: &event_checkin_domain::models::deposit::DepositStatus,
) -> Result<(), String> {
    // D1 write (primary)
    if let Some(db) = d1 {
        crate::db::deposit_statuses::save_deposit_status(db, status).await?;
    }

    // KV write (best-effort cache)
    if let Some(kv) = kv
        && let Err(e) = super::write::save_deposit_status(kv, status, None).await
    {
        tracing::warn!(
            attendee_id = %status.attendee_id,
            error = %e,
            "KV deposit status save failed (non-fatal, D1 is primary)"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Registration form config (Issue #049)
// ---------------------------------------------------------------------------

/// Read per-event registration form config.
/// D1-first: queries the `form_config` column on the events table.
/// Falls back to KV if D1 is unavailable.
/// Returns `None` if no custom config is stored (use defaults).
pub async fn get_form_config(
    event_id: &str,
    d1: Option<&D1Database>,
    kv: Option<&KvStore>,
) -> Result<Option<event_checkin_domain::models::event::RegistrationFormConfig>, String> {
    // D1-first
    if let Some(db) = d1 {
        return crate::db::events::get_form_config(db, event_id).await;
    }

    // KV fallback
    if let Some(kv) = kv {
        use super::schema::form_config_key;
        let key = form_config_key(event_id);
        let raw: Option<String> = kv
            .get(&key)
            .text()
            .await
            .map_err(|e| format!("failed to read form config for event '{event_id}': {e:?}"))?;

        return match raw {
            None => Ok(None),
            Some(json_str) => serde_json::from_str(&json_str)
                .map(Some)
                .map_err(|e| format!("failed to parse form config for event '{event_id}': {e}")),
        };
    }

    Ok(None)
}
