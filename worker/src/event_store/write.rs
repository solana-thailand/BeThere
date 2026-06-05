//! Write operations: saves, creates, updates, deletes, and migration.

use worker::KvStore;

use event_checkin_domain::models::event::{
    CreateEventRequest, EscrowStatus, EventConfig, EventIndex, EventStatus, UpdateEventRequest,
};

use super::read as read_mod;
use super::read::get_event_index;
use super::schema::{
    deduplicate_slug, deposit_counter_key, deposit_status_key, escrow_index_key, event_config_key,
    slugify, thb_deposit_key, thb_deposit_list_key,
};

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

// ---------------------------------------------------------------------------
// Escrow index writes
// ---------------------------------------------------------------------------

/// Write the escrow → event reverse index entry — D1 + KV (dual-write).
pub async fn save_escrow_index(
    d1: Option<&worker::D1Database>,
    kv: Option<&KvStore>,
    escrow_address: &str,
    event_id: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(()); // no escrow to index
    }

    // D1 write (primary)
    if let Some(db) = d1
        && let Err(e) =
            crate::db::escrow_index::upsert_escrow_index_to_d1(db, escrow_address, event_id).await
    {
        tracing::warn!(escrow_address, error = %e, "D1 escrow index write failed");
    }

    // KV write (fallback / legacy)
    if let Some(kv_ref) = kv {
        let key = escrow_index_key(escrow_address);
        kv_ref
            .put(&key, event_id)
            .map_err(|e| format!("failed to build escrow index put: {e:?}"))?
            .execute()
            .await
            .map_err(|e| format!("failed to write escrow index to KV: {e:?}"))?;
    }

    Ok(())
}

/// Remove the escrow → event reverse index entry — D1 + KV.
pub async fn delete_escrow_index(
    d1: Option<&worker::D1Database>,
    kv: Option<&KvStore>,
    escrow_address: &str,
) -> Result<(), String> {
    if escrow_address.is_empty() {
        return Ok(());
    }

    // D1 delete
    if let Some(db) = d1
        && let Err(e) =
            crate::db::escrow_index::delete_escrow_index_from_d1(db, escrow_address).await
    {
        tracing::warn!(escrow_address, error = %e, "D1 escrow index delete failed");
    }

    // KV delete
    if let Some(kv_ref) = kv {
        kv_ref
            .delete(&escrow_index_key(escrow_address))
            .await
            .map_err(|e| format!("failed to delete escrow index: {e:?}"))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CRUD operations
// ---------------------------------------------------------------------------

/// Create a new event.
///
/// Generates a unique ID from the slug, validates required fields,
/// saves the full config to D1 (primary) and KV (write-through cache if available),
/// and updates the KV event index.
pub async fn create_event(
    kv: Option<&KvStore>,
    d1: Option<&worker::D1Database>,
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
    // Collect existing IDs from KV index + D1 for deduplication.
    let kv_index = if let Some(kv_ref) = kv {
        get_event_index(kv_ref).await?
    } else {
        EventIndex::default()
    };
    let mut existing_ids: Vec<String> = kv_index.events.iter().map(|e| e.id.clone()).collect();
    if let Some(db) = d1
        && let Ok(d1_rows) = crate::db::events::list_events(db).await
    {
        for row in d1_rows {
            if let Some(row_id) = row.id
                && !existing_ids.iter().any(|id| id == &row_id)
            {
                existing_ids.push(row_id);
            }
        }
    }
    let existing_id_refs: Vec<&str> = existing_ids.iter().map(|s| s.as_str()).collect();
    let (id, slug) = deduplicate_slug(&slug, &existing_id_refs);

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
        organization_id: req.organization_id.trim().to_string(),
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
        video_url: req.video_url.trim().to_string(),
        event_format: req.event_format.clone(),
        require_contact_info: req.require_contact_info,
        require_photo_consent: req.require_photo_consent,
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

    // D1 write (primary — always if available)
    sync_event_to_d1(d1, &config).await;

    // KV write-through cache (if available, non-fatal)
    if let Some(kv_ref) = kv {
        if let Err(e) = save_event_config(kv_ref, &config).await {
            tracing::warn!(event_id = %id, error = %e, "KV save failed for new event (D1 is primary)");
        }

        // Maintain escrow reverse index (H7)
        if !config.escrow_address.is_empty() {
            let _ = save_escrow_index(d1, Some(kv_ref), &config.escrow_address, &config.id).await;
        }

        // Update KV index
        let mut index = kv_index;
        index.events.insert(0, config.to_meta());
        if let Err(e) = save_event_index(kv_ref, &index).await {
            tracing::warn!(event_id = %id, error = %e, "KV index update failed for new event");
        }
    }

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
    kv: Option<&KvStore>,
    d1: Option<&worker::D1Database>,
    id: &str,
    req: &UpdateEventRequest,
    updated_by: &str,
) -> Result<EventConfig, String> {
    let mut config = crate::event_store::get_event_config_with_fallback(kv, d1, id)
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
        let deposit_usdc_changed = req
            .deposit_amount_usdc
            .is_some_and(|d| d != config.deposit_amount_usdc);
        let deposit_thb_changed = req
            .deposit_amount_thb
            .is_some_and(|d| d != config.deposit_amount_thb);
        let deadline_changed = req
            .refund_deadline_hours
            .is_some_and(|h| h != config.refund_deadline_hours);
        if wallet_changed
            || event_id_changed
            || deposit_usdc_changed
            || deposit_thb_changed
            || deadline_changed
        {
            return Err(
                "cannot change organizer_wallet, on_chain_event_id, deposit_amount_usdc, deposit_amount_thb, or refund_deadline_hours after escrow is initialized on-chain".to_string()
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
    if let Some(ref v) = req.organization_id {
        config.organization_id = v.trim().to_string();
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
    if let Some(ref v) = req.video_url {
        config.video_url = v.trim().to_string();
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
    if let Some(v) = req.require_photo_consent {
        config.require_photo_consent = v;
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

    // D1 write (primary — always if available)
    sync_event_to_d1(d1, &config).await;

    // KV write-through cache (if available)
    if let Some(kv_ref) = kv {
        save_event_config(kv_ref, &config).await?;
    }

    // Maintain escrow reverse index (D1 + KV dual-write)
    if !config.escrow_address.is_empty() {
        save_escrow_index(d1, kv, &config.escrow_address, &config.id).await?;
    }

    // Update KV index entry (if KV available)
    if let Some(kv_ref) = kv {
        let mut index = get_event_index(kv_ref).await?;
        if let Some(entry) = index.events.iter_mut().find(|e| e.id == id) {
            *entry = config.to_meta();
        }
        save_event_index(kv_ref, &index).await?;
    }

    tracing::info!(event_id = %id, "event updated");

    Ok(config)
}

/// Apply partial update from `UpdateEventRequest` to an existing `EventConfig`.
/// Validates escrow-critical field locks and deposit caps.
/// Does NOT save — caller is responsible for persisting to KV/D1.
pub fn apply_update(config: &mut EventConfig, req: &UpdateEventRequest) -> Result<(), String> {
    // SEC-002: Lock escrow-critical fields after on-chain init.
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
        let deposit_usdc_changed = req
            .deposit_amount_usdc
            .is_some_and(|d| d != config.deposit_amount_usdc);
        let deposit_thb_changed = req
            .deposit_amount_thb
            .is_some_and(|d| d != config.deposit_amount_thb);
        let deadline_changed = req
            .refund_deadline_hours
            .is_some_and(|h| h != config.refund_deadline_hours);
        if wallet_changed
            || event_id_changed
            || deposit_usdc_changed
            || deposit_thb_changed
            || deadline_changed
        {
            return Err(
                "cannot change organizer_wallet, on_chain_event_id, deposit_amount_usdc, deposit_amount_thb, or refund_deadline_hours after escrow is initialized on-chain"
                    .to_string(),
            );
        }
    }

    // SEC-003: Max deposit cap
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
    if let Some(ref v) = req.organization_id {
        config.organization_id = v.trim().to_string();
    }
    if let Some(v) = req.deposit_enabled {
        config.deposit_enabled = v;
    }
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
    if let Some(ref v) = req.video_url {
        config.video_url = v.trim().to_string();
    }
    if let Some(ref v) = req.event_format {
        config.event_format = v.clone();
        if v.has_in_person() {
            config.deposit_enabled = true;
        }
    }
    if let Some(v) = req.require_contact_info {
        config.require_contact_info = v;
    }
    if let Some(v) = req.require_photo_consent {
        config.require_photo_consent = v;
    }
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

    Ok(())
}

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
        && let Some(mut config) = read_mod::get_event_config(kv, &meta.id).await?
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
        organization_id: String::new(), // seeded event has no org
        deposit_enabled: global.event_defaults.deposit_enabled
            || event_checkin_domain::models::event::EventFormat::InPerson.has_in_person(),
        deposit_amount_usdc: global.event_defaults.deposit_amount_usdc,
        deposit_amount_thb: global.event_defaults.deposit_amount_thb,
        promptpay_id: global.event_defaults.promptpay_id.clone(),
        escrow_address: String::new(),
        escrow_status: EscrowStatus::None,
        organizer_wallet: String::new(),
        on_chain_event_id: 0,
        refund_deadline_hours: 168,
        max_refundable_deposits: 0,
        description: String::new(),
        location: String::new(),
        video_url: String::new(),
        event_format: event_checkin_domain::models::event::EventFormat::InPerson,
        require_contact_info: true,
        require_photo_consent: false,
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
// KV seeding from D1 (P0 — ensures KV index is consistent with D1)
// ---------------------------------------------------------------------------

/// Seed KV from D1: read all events from D1, upsert each config + rebuild the index.
///
/// Idempotent — overwrites existing KV entries with D1 data.
/// Returns the number of events synced.
pub async fn seed_kv_from_d1(kv: &KvStore, d1: &worker::D1Database) -> Result<usize, String> {
    let rows = crate::db::events::list_events(d1).await?;
    let count = rows.len();

    if rows.is_empty() {
        tracing::info!("seed_kv_from_d1: no events in D1, nothing to do");
        return Ok(0);
    }

    // Upsert each event config into KV
    let mut index = get_event_index(kv).await?;
    for row in &rows {
        let config = row.to_event_config();
        save_event_config(kv, &config).await?;

        // Upsert into index (update if exists, insert if new)
        let meta = config.to_meta();
        if let Some(existing) = index.events.iter_mut().find(|e| e.id == meta.id) {
            *existing = meta;
        } else {
            index.events.push(meta);
        }
    }

    // Sort by creation date (newest first) to match normal ordering
    index.events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    save_event_index(kv, &index).await?;

    tracing::info!(count, "seed_kv_from_d1: synced events from D1 to KV");
    Ok(count)
}

// ---------------------------------------------------------------------------
// Deposit writes
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Quiz migration
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
