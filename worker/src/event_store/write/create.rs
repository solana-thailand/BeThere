//! Event creation.

use worker::KvStore;

use event_checkin_domain::models::event::{
    CreateEventRequest, EscrowStatus, EventConfig, EventIndex, EventStatus,
};

use crate::event_store::read::get_event_index;
use crate::event_store::schema::{deduplicate_slug, slugify};

use super::escrow::save_escrow_index;
use super::index::{save_event_config, save_event_index, sync_event_to_d1};

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
        poster_url: req.poster_url.trim().to_string(),
        recap_published: false,
        post_event_registration_open: false,
        post_event_registration_until_ms: None,
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
        dev_profile_enabled: false,
        community_links: req.community_links.clone(),
        calendar_subscribe_url: req.calendar_subscribe_url.clone(),
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
