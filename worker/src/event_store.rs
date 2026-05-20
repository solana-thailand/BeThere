//! KV-based event storage for multi-event / organizer support (Issue 004).
//!
//! Events are stored in a Cloudflare KV namespace bound as `EVENTS`:
//!
//!   "events"                         → EventIndex (JSON) — list of EventMeta
//!   "event:{id}"                     → EventConfig (JSON) — full per-event config
//!
//! Per-event quiz data uses the same namespace with prefixed keys:
//!   "event:{id}:quiz:questions"      → QuizConfig (JSON)
//!   "event:{id}:quiz:progress:{tok}" → QuizProgress (JSON)

// Phase 2 helpers used when event-scoping existing handlers.
use worker::KvStore;

use event_checkin_domain::models::event::{
    CreateEventRequest, EscrowStatus, EventConfig, EventIndex, EventMeta, EventStatus,
    UpdateEventRequest,
};

// ---------------------------------------------------------------------------
// Event index (list of all events)
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
// Per-event config
// ---------------------------------------------------------------------------

/// KV key for a specific event's full configuration.
fn event_config_key(id: &str) -> String {
    format!("event:{id}")
}

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

/// Write a single event's full configuration.
async fn save_event_config(kv: &KvStore, config: &EventConfig) -> Result<(), String> {
    let key = event_config_key(&config.id);
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize event config: {e:?}"))?;
    kv.put(&key, &json_str)
        .map_err(|e| format!("failed to build event config put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write event config to KV: {e:?}"))
}

// ---------------------------------------------------------------------------
// Escrow reverse index (escrow address → event ID) — H7
// ---------------------------------------------------------------------------

/// KV key for the escrow → event reverse index.
fn escrow_index_key(escrow_address: &str) -> String {
    format!("escrow:{escrow_address}")
}

/// Look up an event ID by its escrow PDA address via the reverse index.
/// Returns `None` if the escrow address is not indexed.
pub async fn get_event_id_by_escrow(kv: &KvStore, escrow_address: &str) -> Option<String> {
    let key = escrow_index_key(escrow_address);
    kv.get(&key).text().await.ok().flatten()
}

/// Write the escrow → event reverse index entry.
pub async fn save_escrow_index(
    kv: &KvStore,
    escrow_address: &str,
    event_id: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(()); // no escrow to index
    }
    let key = escrow_index_key(escrow_address);
    kv.put(&key, event_id)
        .map_err(|e| format!("failed to build escrow index put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write escrow index to KV: {e:?}"))
}

/// Remove the escrow → event reverse index entry.
async fn delete_escrow_index(kv: &KvStore, escrow_address: &str) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(());
    }
    let key = escrow_index_key(escrow_address);
    kv.delete(&key)
        .await
        .map_err(|e| format!("failed to delete escrow index: {e:?}"))
}

// ---------------------------------------------------------------------------
// CRUD operations
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

/// Create a new event.
///
/// Generates a unique ID from the slug, validates required fields,
/// saves the full config, and updates the event index.
pub async fn create_event(
    kv: &KvStore,
    req: &CreateEventRequest,
    updated_by: &str,
) -> Result<EventConfig, String> {
    // Validate required fields
    if req.name.trim().is_empty() {
        return Err("event name is required".to_string());
    }
    if req.sheet_id.trim().is_empty() {
        return Err("google sheet_id is required".to_string());
    }
    if req.time_tba {
        // TBA mode: ensure at least date-level timestamps
        if req.event_start_ms <= 0 {
            return Err("event_start_ms date is required even for TBA events".to_string());
        }
        if req.event_end_ms <= 0 {
            // Default end = start + 24h if not provided
        }
    } else {
        if req.event_start_ms <= 0 {
            return Err("event_start_ms must be a positive Unix epoch millisecond".to_string());
        }
        if req.event_end_ms <= req.event_start_ms {
            return Err("event_end_ms must be after event_start_ms".to_string());
        }
    }

    // SEC-003: Max deposit cap ($1,000 USDC = 1_000_000_000 smallest units, 6 decimals)
    const MAX_DEPOSIT_USDC: u64 = 1_000_000_000;
    if req.deposit_amount_usdc > MAX_DEPOSIT_USDC {
        return Err(format!(
            "deposit_amount_usdc exceeds maximum cap ({MAX_DEPOSIT_USDC} = $1,000 USDC)"
        ));
    }

    // Generate slug from name if not provided
    let slug = if req.slug.trim().is_empty() {
        slugify(&req.name)
    } else {
        slugify(&req.slug)
    };

    // Auto-deduplicate slug on collision (e.g. "my-event" → "my-event-1" → "my-event-2")
    // Supports recurring events with the same name.
    let index = get_event_index(kv).await?;
    let (id, slug) = {
        let existing_ids: Vec<&str> = index.events.iter().map(|e| e.id.as_str()).collect();
        deduplicate_slug(&slug, &existing_ids)
    };

    let now = chrono::Utc::now().to_rfc3339();

    let config = EventConfig {
        id: id.clone(),
        name: req.name.trim().to_string(),
        slug: slug.clone(),
        tagline: req.tagline.trim().to_string(),
        link: req.link.trim().to_string(),
        status: EventStatus::Draft,
        event_start_ms: req.event_start_ms,
        event_end_ms: req.event_end_ms,
        time_tba: req.time_tba,
        sheet_id: req.sheet_id.trim().to_string(),
        sheet_name: if req.sheet_name.is_empty() {
            "Attendees".to_string()
        } else {
            req.sheet_name.clone()
        },
        staff_sheet_name: if req.staff_sheet_name.is_empty() {
            "staff".to_string()
        } else {
            req.staff_sheet_name.clone()
        },
        quiz_enabled: req.quiz_enabled,
        nft_collection_mint: req.nft_collection_mint.trim().to_string(),
        nft_metadata_uri: req.nft_metadata_uri.trim().to_string(),
        nft_image_url: req.nft_image_url.trim().to_string(),
        nft_name_template: req.nft_name_template.trim().to_string(),
        nft_symbol: req.nft_symbol.trim().to_string(),
        nft_description_template: req.nft_description_template.trim().to_string(),
        organizer_emails: req
            .organizer_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect(),
        staff_emails: req
            .staff_emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect(),
        claim_base_url: req.claim_base_url.trim().to_string(),
        merkle_tree: req.merkle_tree.trim().to_string(),
        // Deposit auto-enabled when format includes in-person track.
        // If organizer explicitly sets deposit_enabled=true for online-only, honor it.
        deposit_enabled: req.deposit_enabled || req.event_format.has_in_person(),
        deposit_amount_usdc: req.deposit_amount_usdc,
        deposit_amount_thb: req.deposit_amount_thb,
        promptpay_id: req.promptpay_id.trim().to_string(),
        escrow_address: req.escrow_address.trim().to_string(),
        escrow_status: EscrowStatus::None,
        organizer_wallet: req.organizer_wallet.trim().to_string(),
        on_chain_event_id: req.on_chain_event_id,
        refund_deadline_hours: req.refund_deadline_hours,
        max_refundable_deposits: req.max_refundable_deposits,
        description: req.description.trim().to_string(),
        location: req.location.trim().to_string(),
        event_format: req.event_format.clone(),
        require_contact_info: req.require_contact_info,
        in_person_capacity: req.in_person_capacity,
        online_capacity: req.online_capacity,
        online_open_mode: req.online_open_mode.clone(),
        online_registration_open: req.online_registration_open,
        deposit_deadline_hours: req.deposit_deadline_hours,
        visibility: req.visibility.clone(),
        created_at: now.clone(),
        updated_at: now,
        updated_by: updated_by.to_string(),
    };

    // Save full config
    save_event_config(kv, &config).await?;

    // Maintain escrow reverse index (H7)
    if !config.escrow_address.is_empty() {
        save_escrow_index(kv, &config.escrow_address, &config.id).await?;
    }

    // Update index
    let mut index = index;
    index.events.insert(0, config.to_meta());
    save_event_index(kv, &index).await?;

    tracing::info!(
        event_id = %id,
        name = %config.name,
        sheet_id = %config.sheet_id,
        "event created"
    );

    Ok(config)
}

/// Update an existing event's configuration.
///
/// Only provided (non-None) fields are updated.
/// Returns the updated EventConfig.
pub async fn update_event(
    kv: &KvStore,
    id: &str,
    req: &UpdateEventRequest,
    updated_by: &str,
) -> Result<EventConfig, String> {
    let mut config = get_event_config(kv, id)
        .await?
        .ok_or_else(|| format!("event '{id}' not found"))?;

    // Optimistic concurrency: if client provides expected_updated_at,
    // verify it matches the stored value to prevent blind overwrites.
    if let Some(ref expected) = req.expected_updated_at
        && expected != &config.updated_at
    {
        return Err(format!(
            "conflict: event was modified by another user at {}. Please reload and retry.",
            config.updated_at
        ));
    }

    // SEC-002: Lock escrow-critical fields after on-chain init.
    // If escrow_address is set (escrow initialized on-chain), reject changes to
    // fields that are baked into the on-chain EventEscrow PDA.
    // Compare actual values — only reject if the values actually changed.
    //
    // Exception: when escrow_status is being reset to None (re-initialization),
    // the on-chain escrow was already verified as closed by the handler (SEC-ESCROW-RESET).
    // Allow unlocking all fields so a fresh escrow can be created.
    let is_escrow_reset = req
        .escrow_status
        .as_ref()
        .is_some_and(|s| matches!(s, EscrowStatus::None));

    if !config.escrow_address.is_empty() && !is_escrow_reset {
        let wallet_changed = req
            .organizer_wallet
            .as_ref()
            .is_some_and(|w| w.trim() != config.organizer_wallet.trim());
        let event_id_changed = req
            .on_chain_event_id
            .is_some_and(|id| id != config.on_chain_event_id);
        let deposit_changed = req
            .deposit_amount_usdc
            .is_some_and(|d| d != config.deposit_amount_usdc);
        let deadline_changed = req
            .refund_deadline_hours
            .is_some_and(|h| h != config.refund_deadline_hours);
        if wallet_changed || event_id_changed || deposit_changed || deadline_changed {
            return Err(
                "cannot change organizer_wallet, on_chain_event_id, deposit_amount_usdc, or refund_deadline_hours after escrow is initialized on-chain".to_string()
            );
        }
    }

    // SEC-003: Max deposit cap in update path
    const MAX_DEPOSIT_USDC: u64 = 1_000_000_000;
    if let Some(v) = req.deposit_amount_usdc {
        if v > MAX_DEPOSIT_USDC {
            return Err(format!(
                "deposit_amount_usdc exceeds maximum cap ({MAX_DEPOSIT_USDC} = $1,000 USDC)"
            ));
        }
        config.deposit_amount_usdc = v;
    }

    // Apply partial updates
    if let Some(ref name) = req.name {
        config.name = name.trim().to_string();
    }
    if let Some(ref slug) = req.slug {
        config.slug = slugify(slug);
    }
    if let Some(ref tagline) = req.tagline {
        config.tagline = tagline.trim().to_string();
    }
    if let Some(ref link) = req.link {
        config.link = link.trim().to_string();
    }
    if let Some(ref status) = req.status {
        config.status = status.clone();
    }
    if let Some(ms) = req.event_start_ms {
        if ms <= 0 {
            return Err("event_start_ms must be positive".to_string());
        }
        config.event_start_ms = ms;
    }
    if let Some(ms) = req.event_end_ms {
        if ms <= config.event_start_ms {
            return Err("event_end_ms must be after event_start_ms".to_string());
        }
        config.event_end_ms = ms;
    }
    if let Some(tba) = req.time_tba {
        config.time_tba = tba;
    }
    if let Some(ref sheet_id) = req.sheet_id {
        if sheet_id.trim().is_empty() {
            return Err("sheet_id cannot be empty".to_string());
        }
        config.sheet_id = sheet_id.trim().to_string();
    }
    if let Some(ref sheet_name) = req.sheet_name {
        config.sheet_name = sheet_name.clone();
    }
    if let Some(ref staff_sheet_name) = req.staff_sheet_name {
        config.staff_sheet_name = staff_sheet_name.clone();
    }
    if let Some(enabled) = req.quiz_enabled {
        config.quiz_enabled = enabled;
    }
    if let Some(ref v) = req.nft_collection_mint {
        config.nft_collection_mint = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_metadata_uri {
        config.nft_metadata_uri = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_image_url {
        config.nft_image_url = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_name_template {
        config.nft_name_template = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_symbol {
        config.nft_symbol = v.trim().to_string();
    }
    if let Some(ref v) = req.nft_description_template {
        config.nft_description_template = v.trim().to_string();
    }
    if let Some(ref emails) = req.organizer_emails {
        config.organizer_emails = emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
    }
    if let Some(ref emails) = req.staff_emails {
        config.staff_emails = emails
            .iter()
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
    }
    if let Some(ref url) = req.claim_base_url {
        config.claim_base_url = url.trim().to_string();
    }
    if let Some(ref v) = req.merkle_tree {
        config.merkle_tree = v.trim().to_string();
    }

    // Deposit fields
    if let Some(v) = req.deposit_enabled {
        config.deposit_enabled = v;
    }
    // deposit_amount_usdc handled above with SEC-003 max cap validation
    if let Some(v) = req.deposit_amount_thb {
        config.deposit_amount_thb = v;
    }
    if let Some(ref v) = req.promptpay_id {
        config.promptpay_id = v.trim().to_string();
    }
    if let Some(ref v) = req.escrow_address {
        config.escrow_address = v.trim().to_string();
    }
    if let Some(ref v) = req.escrow_status {
        // Validate state transition
        let valid = matches!(
            (&config.escrow_status, v),
            (EscrowStatus::None, EscrowStatus::Initialized)
                | (EscrowStatus::Initialized, EscrowStatus::Deactivated)
                | (EscrowStatus::Deactivated, EscrowStatus::Closed)
                | (EscrowStatus::Closed, EscrowStatus::None)
                | (EscrowStatus::Cancelled, EscrowStatus::None)
        );
        if !valid {
            return Err(format!(
                "invalid escrow status transition: {} → {}",
                config.escrow_status, v
            ));
        }
        config.escrow_status = v.clone();
    }
    if let Some(ref v) = req.organizer_wallet {
        config.organizer_wallet = v.trim().to_string();
    }
    if let Some(v) = req.on_chain_event_id {
        config.on_chain_event_id = v;
    }
    if let Some(v) = req.refund_deadline_hours {
        config.refund_deadline_hours = v;
    }
    if let Some(v) = req.max_refundable_deposits {
        config.max_refundable_deposits = v;
    }
    if let Some(ref v) = req.description {
        config.description = v.trim().to_string();
    }
    if let Some(ref v) = req.location {
        config.location = v.trim().to_string();
    }
    if let Some(ref v) = req.event_format {
        config.event_format = v.clone();
        // Re-sync deposit_enabled when format changes:
        // - In-person/Hybrid formats always enable deposit
        // - Online format disables auto-deposit (organizer can still set explicitly)
        if v.has_in_person() {
            config.deposit_enabled = true;
        }
    }
    if let Some(v) = req.require_contact_info {
        config.require_contact_info = v;
    }

    // Capacity settings
    if let Some(v) = req.in_person_capacity {
        config.in_person_capacity = v;
    }
    if let Some(v) = req.online_capacity {
        config.online_capacity = v;
    }
    if let Some(ref v) = req.online_open_mode {
        config.online_open_mode = v.clone();
    }
    if let Some(v) = req.online_registration_open {
        config.online_registration_open = v;
    }
    if let Some(v) = req.deposit_deadline_hours {
        config.deposit_deadline_hours = v;
    }
    if let Some(ref v) = req.visibility {
        config.visibility = v.clone();
    }

    config.updated_at = chrono::Utc::now().to_rfc3339();
    config.updated_by = updated_by.to_string();

    // Save updated config
    save_event_config(kv, &config).await?;

    // Maintain escrow reverse index (H7)
    if !config.escrow_address.is_empty() {
        save_escrow_index(kv, &config.escrow_address, &config.id).await?;
    }

    // Update index entry
    let mut index = get_event_index(kv).await?;
    if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
        *entry = config.to_meta();
    }
    save_event_index(kv, &index).await?;

    tracing::info!(event_id = %id, "event updated");

    Ok(config)
}

/// Archive (soft-delete) an event by setting its status to Archived.
///
/// SEC-004: Rejects archive if the event has an active on-chain escrow.
/// Archive the event first on-chain (close escrow) before archiving here.
pub async fn archive_event(kv: &KvStore, id: &str) -> Result<(), String> {
    let mut config = get_event_config(kv, id)
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
    let mut config = get_event_config(kv, id)
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
    let config = get_event_config(kv, id)
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
        let _ = delete_escrow_index(kv, &config.escrow_address).await;
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

// ---------------------------------------------------------------------------
// Seed from global config
// ---------------------------------------------------------------------------

/// Seed the first event from global AppConfig (env vars).
///
/// Idempotent: if the event index already has an active event, returns it.
/// Otherwise builds an EventConfig with id="default" and status=Active,
/// saves it to KV, and updates the index.
pub async fn seed_from_config(
    kv: &KvStore,
    global: &event_checkin_domain::config::AppConfig,
    state: &crate::state::AppState,
) -> Result<EventConfig, String> {
    // Idempotent: return existing active event if any
    let index = get_event_index(kv).await?;
    if let Some(meta) = index
        .events
        .iter()
        .find(|e| e.status == EventStatus::Active)
        && let Some(mut config) = get_event_config(kv, &meta.id).await?
    {
        // Fix legacy seed events that used hardcoded slug "default"
        let expected_slug = slugify(&global.event_defaults.name);
        if config.slug == "default" && expected_slug != "default" {
            tracing::info!(
                event_id = %config.id,
                old_slug = %config.slug,
                new_slug = %expected_slug,
                "seed: migrating legacy slug"
            );
            config.slug = expected_slug.clone();
            save_event_config(kv, &config).await?;

            // Update index meta too
            let mut index = index;
            if let Some(m) = index.events.iter_mut().find(|e| e.id == config.id) {
                m.slug = expected_slug;
            }
            save_event_index(kv, &index).await?;
        }

        tracing::info!(event_id = %config.id, slug = %config.slug, "seed: already have active event");
        return Ok(config);
    }

    let now = chrono::Utc::now().to_rfc3339();

    let defaults = &global.event_defaults;
    let config = EventConfig {
        id: "default".to_string(),
        name: defaults.name.clone(),
        slug: slugify(&defaults.name),
        tagline: defaults.tagline.clone(),
        link: defaults.link.clone(),
        status: EventStatus::Active,
        event_start_ms: defaults.start_ms,
        event_end_ms: defaults.end_ms,
        time_tba: false,
        sheet_id: global.sheets.sheet_id.clone(),
        sheet_name: global.sheets.sheet_name.clone(),
        staff_sheet_name: global.sheets.staff_sheet_name.clone(),
        quiz_enabled: true,
        nft_collection_mint: global.nft.collection_mint.clone(),
        nft_metadata_uri: global.nft.metadata_uri.clone(),
        nft_image_url: if global.nft.image_url.is_empty() {
            format!(
                "{}/api/badge-hd.svg",
                global.server.url.trim_end_matches('/')
            )
        } else {
            global.nft.image_url.clone()
        },
        nft_name_template: "BeThere - {event_name}".to_string(),
        nft_symbol: "BETHERE".to_string(),
        nft_description_template: "Proof of attendance at {event_name}".to_string(),
        organizer_emails: {
            let mut emails: Vec<String> = global.super_admin_emails.iter().cloned().collect();
            // Merge organizers from Google Sheet staff tab (role "admin" or "organizer")
            let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
            if let Ok(members) = crate::sheets::get_staff_members(
                state,
                &global.sheets.sheet_id,
                &global.sheets.staff_sheet_name,
                kv,
            )
            .await
            {
                for m in &members {
                    if matches!(m.role.as_str(), "admin" | "organizer")
                        && !emails.iter().any(|e| e.eq_ignore_ascii_case(&m.email))
                    {
                        emails.push(m.email.clone());
                    }
                }
            }
            emails
        },
        staff_emails: {
            let mut emails: Vec<String> = global.staff_emails.iter().cloned().collect();
            // Merge staff from Google Sheet staff tab (all members)
            let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
            if let Ok(members) = crate::sheets::get_staff_members(
                state,
                &global.sheets.sheet_id,
                &global.sheets.staff_sheet_name,
                kv,
            )
            .await
            {
                for m in &members {
                    if !emails.iter().any(|e| e.eq_ignore_ascii_case(&m.email)) {
                        emails.push(m.email.clone());
                    }
                }
            }
            emails
        },
        claim_base_url: global.server.claim_base_url.clone(),
        merkle_tree: String::new(), // not in global config — per-event only
        deposit_enabled: false,
        deposit_amount_usdc: 0,
        deposit_amount_thb: 0,
        promptpay_id: String::new(),
        escrow_address: String::new(),
        escrow_status: EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: 0,
        refund_deadline_hours: 168,
        max_refundable_deposits: 0,
        description: String::new(),
        location: String::new(),
        event_format: event_checkin_domain::models::event::EventFormat::InPerson,
        require_contact_info: true,
        in_person_capacity: None,
        online_capacity: None,
        online_open_mode: event_checkin_domain::models::event::OnlineOpenMode::default(),
        online_registration_open: false,
        deposit_deadline_hours: None,
        visibility: event_checkin_domain::models::event::EventVisibility::default(),
        created_at: now.clone(),
        updated_at: now,
        updated_by: String::new(), // seeded from config, no user context
    };

    // Save full config
    save_event_config(kv, &config).await?;

    // Update index
    let mut index = index;
    index.events.insert(0, config.to_meta());
    save_event_index(kv, &index).await?;

    tracing::info!(
        name = %config.name,
        "seed: created default event from config"
    );

    Ok(config)
}

// ---------------------------------------------------------------------------
// Event resolution helpers
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

/// Build the event-scoped KV key for quiz questions.
///
/// When `event_id` is "default" (legacy mode), returns `"questions"` for
/// backward compatibility with the old QUIZ KV namespace.
/// Otherwise returns `"event:{id}:quiz:questions"` for the EVENTS KV namespace.
pub fn quiz_questions_key(event_id: &str) -> String {
    format!("event:{event_id}:quiz:questions")
}

/// Build the event-scoped KV key for quiz progress.
///
/// When `event_id` is "default" (legacy mode), returns `"progress:{token}"`
/// for backward compatibility with the old QUIZ KV namespace.
/// Otherwise returns `"event:{id}:quiz:progress:{token}"`.
pub fn quiz_progress_key(event_id: &str, claim_token: &str) -> String {
    format!("event:{event_id}:quiz:progress:{claim_token}")
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
// Quiz migration (QUIZ → EVENTS namespace)
// ---------------------------------------------------------------------------

/// Result of a quiz migration operation.
pub struct MigrationResult {
    /// `true` if data was copied, `false` if destination already existed.
    pub migrated: bool,
    /// The event ID that was the migration target.
    pub event_id: String,
    /// Human-readable status message.
    pub message: String,
}

/// Migrate quiz config from legacy QUIZ namespace to event-scoped EVENTS namespace.
/// Idempotent — skips if destination key already exists.
pub async fn migrate_quiz_to_event(
    events_kv: &KvStore,
    quiz_kv: &KvStore,
    event_id: &str,
) -> Result<MigrationResult, String> {
    let dest_key = format!("event:{event_id}:quiz:questions");

    // Idempotent: skip if destination already exists
    let existing: Option<String> = events_kv
        .get(&dest_key)
        .text()
        .await
        .map_err(|e| format!("failed to check destination key '{dest_key}': {e:?}"))?;

    match existing {
        Some(_) => {
            tracing::info!(dest_key = %dest_key, "migrate: destination already exists, skipping");
            Ok(MigrationResult {
                migrated: false,
                event_id: event_id.to_string(),
                message: format!("quiz data already migrated to event '{event_id}'"),
            })
        }
        None => {
            // Read source from legacy QUIZ namespace
            let raw: Option<String> =
                quiz_kv.get("questions").text().await.map_err(|e| {
                    format!("failed to read 'questions' from QUIZ namespace: {e:?}")
                })?;

            let source: serde_json::Value =
                serde_json::from_str(raw.as_deref().ok_or_else(|| {
                    "no quiz data found in QUIZ namespace (key 'questions' is empty)".to_string()
                })?)
                .map_err(|e| format!("failed to parse quiz data from QUIZ namespace: {e}"))?;

            // Write to EVENTS namespace
            let json_str = serde_json::to_string(&source)
                .map_err(|e| format!("failed to serialize quiz data: {e:?}"))?;
            events_kv
                .put(&dest_key, &json_str)
                .map_err(|e| format!("failed to build quiz migration put: {e:?}"))?
                .execute()
                .await
                .map_err(|e| format!("failed to write quiz data to '{dest_key}': {e:?}"))?;

            tracing::info!(dest_key = %dest_key, "migrate: copied quiz data");
            Ok(MigrationResult {
                migrated: true,
                event_id: event_id.to_string(),
                message: format!("quiz data migrated to event '{event_id}'"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a string to a URL-friendly slug.
///
/// Lowercases, replaces non-alphanumeric runs with hyphens,
/// strips leading/trailing hyphens.
fn slugify(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Resolve slug collisions by appending an incrementing suffix.
///
/// If `base_slug` is not in `existing_ids`, returns it unchanged.
/// Otherwise tries `base_slug-1`, `base_slug-2`, ... until a free ID is found.
/// Both `id` and `slug` are returned (they are always equal).
fn deduplicate_slug(base_slug: &str, existing_ids: &[&str]) -> (String, String) {
    let existing_set: std::collections::HashSet<&str> = existing_ids.iter().copied().collect();

    if !existing_set.contains(base_slug) {
        return (base_slug.to_string(), base_slug.to_string());
    }

    for i in 1..=1000u32 {
        let candidate = format!("{base_slug}-{i}");
        if !existing_set.contains(candidate.as_str()) {
            return (candidate.clone(), candidate);
        }
    }

    // Extremely unlikely fallback — use timestamp suffix
    let fallback = format!("{base_slug}-{}", chrono::Utc::now().timestamp());
    (fallback.clone(), fallback)
}

// ---------------------------------------------------------------------------
// Deposit KV helpers (Issue 010 — dual-track deposit/refund)
// ---------------------------------------------------------------------------

/// KV key for per-attendee deposit status.
/// Pattern: `event:{id}:deposit:status:{attendee_id}`
pub fn deposit_status_key(event_id: &str, attendee_id: &str) -> String {
    format!("event:{event_id}:deposit:status:{attendee_id}")
}

/// KV key for THB deposit record.
/// Pattern: `event:{id}:deposit:thb:{attendee_id}`
pub fn thb_deposit_key(event_id: &str, attendee_id: &str) -> String {
    format!("event:{event_id}:deposit:thb:{attendee_id}")
}

/// KV key for listing all THB deposits in an event.
/// Pattern: `event:{id}:deposit:thb:list`
pub fn thb_deposit_list_key(event_id: &str) -> String {
    format!("event:{event_id}:deposit:thb:list")
}

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

/// Save deposit status for an attendee.
pub async fn save_deposit_status(
    kv: &KvStore,
    status: &event_checkin_domain::models::deposit::DepositStatus,
) -> Result<(), String> {
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

/// KV key for the deposit counter for an event.
/// Value is a u32 counter (stored as decimal string).
fn deposit_counter_key(event_id: &str) -> String {
    format!("event:{event_id}:deposit:counter")
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

/// Save THB deposit record for an attendee.
pub async fn save_thb_deposit(
    kv: &KvStore,
    deposit: &event_checkin_domain::models::deposit::ThbDeposit,
) -> Result<(), String> {
    let key = thb_deposit_key(&deposit.event_id, &deposit.attendee_id);
    let json = serde_json::to_string(deposit)
        .map_err(|e| format!("failed to serialize THB deposit: {e}"))?;
    kv.put(&key, &json)
        .map_err(|e| format!("failed to build THB deposit put: {e:?}"))?
        .execute()
        .await
        .map_err(|e| format!("failed to write THB deposit to KV: {e:?}"))
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
