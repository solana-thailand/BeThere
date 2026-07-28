//! Seeding: from global config and from D1.

use worker::KvStore;

use event_checkin_domain::models::event::{EscrowStatus, EventConfig, EventStatus};

use crate::event_store::read as read_mod;
use crate::event_store::read::get_event_index;
use crate::event_store::schema::slugify;

use super::index::{save_event_config, save_event_index};

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
        quiz_enabled: false,
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
        dev_profile_enabled: false,
        community_links: vec![],
        calendar_subscribe_url: String::new(),
        poster_url: String::new(),
        recap_published: false,
        post_event_registration_open: false,
        post_event_registration_until_ms: None,
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
