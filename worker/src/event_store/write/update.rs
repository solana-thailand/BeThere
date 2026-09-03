//! Event updates.

use worker::KvStore;

use event_checkin_domain::models::event::{
    DEFAULT_ATTENDEE_SHEET_NAME, DEFAULT_STAFF_SHEET_NAME, EscrowStatus, EventConfig,
    UpdateEventRequest, normalize_sheet_name,
};

use crate::event_store::read::get_event_index;
use crate::event_store::schema::slugify;

use super::escrow::save_escrow_index;
use super::index::{save_event_config, save_event_index, sync_event_to_d1};

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

    // Field validation and application live in `apply_update`, which the main
    // `PUT /events/{id}` handler drives directly. Sharing it is what keeps the
    // two entry points from diverging — they already had, on
    // `dev_profile_enabled`, which only `apply_update` applied.
    apply_update(&mut config, req)?;

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
    apply_sheet_names(config, req)?;
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
    if let Some(ref v) = req.poster_url {
        config.poster_url = v.trim().to_string();
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
    if let Some(v) = req.dev_profile_enabled {
        config.dev_profile_enabled = v;
    }
    if let Some(ref links) = req.community_links {
        config.community_links = links.clone();
    }
    if let Some(ref url) = req.calendar_subscribe_url {
        config.calendar_subscribe_url = url.clone();
    }
    if let Some(ref url) = req.poster_url {
        config.poster_url = url.trim().to_string();
    }

    Ok(())
}

/// Apply the two organiser-supplied Google Sheets tab names, if present.
///
/// Tab names reach an A1 range on every Sheets read and write, so they are
/// validated here rather than at the API — the Sheets calls are detached
/// best-effort work whose errors are logged and dropped, which would turn an
/// unusable name into a tab that silently never fills in.
///
/// A blank value keeps the event's current tab rather than resetting it to the
/// default: clearing the field in the form must not silently retarget a live
/// event from `Registrations` to `Attendees`.
fn apply_sheet_names(config: &mut EventConfig, req: &UpdateEventRequest) -> Result<(), String> {
    if let Some(ref sheet_name) = req.sheet_name {
        config.sheet_name = normalize_sheet_name(
            sheet_name,
            fallback(&config.sheet_name, DEFAULT_ATTENDEE_SHEET_NAME),
        )?;
    }
    if let Some(ref staff_sheet_name) = req.staff_sheet_name {
        config.staff_sheet_name = normalize_sheet_name(
            staff_sheet_name,
            fallback(&config.staff_sheet_name, DEFAULT_STAFF_SHEET_NAME),
        )?;
    }
    Ok(())
}

/// The stored tab name, or `default` when the event predates validation and has
/// none.
fn fallback<'a>(current: &'a str, default: &'a str) -> &'a str {
    match current.trim().is_empty() {
        true => default,
        false => current,
    }
}
