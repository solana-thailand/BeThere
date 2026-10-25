//! Event writes: upsert on every create/update/seed, and delete.

use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Write operations
// ---------------------------------------------------------------------------

/// Insert or replace an event row in D1.
///
/// Called on every create/update/seed — mirrors KV writes to D1.
pub async fn upsert_event(
    db: &D1Database,
    config: &event_checkin_domain::models::event::EventConfig,
) -> Result<(), String> {
    let organizer_emails = config.organizer_emails.join(",");
    let staff_emails = config.staff_emails.join(",");
    let status_str = config.status.as_str();
    let escrow_status_str = config.escrow_status.as_str();
    let event_format_str = config.event_format.as_str();
    let visibility_str = config.visibility.as_str();
    let online_open_mode_str = config.online_open_mode.as_str();
    let community_links_json =
        serde_json::to_string(&config.community_links).unwrap_or_else(|_| "[]".to_string());
    let sponsors_json =
        serde_json::to_string(&config.sponsors).unwrap_or_else(|_| "[]".to_string());

    let sql = format!(
        "INSERT INTO events (\
         id, name, slug, status, event_format, event_start_ms, event_end_ms, \
         deposit_enabled, deposit_amount_usdc, deposit_amount_thb, \
         escrow_status, escrow_pda, location, tagline, \
         organizer_emails, organization_id, video_url, \
         sheet_id, sheet_name, staff_sheet_name, \
         capacity, total_attendees, created_at, updated_at, \
         link, time_tba, quiz_enabled, \
         nft_collection_mint, nft_metadata_uri, nft_image_url, \
         nft_name_template, nft_symbol, nft_description_template, \
         merkle_tree, staff_emails, claim_base_url, \
         promptpay_id, escrow_address, organizer_wallet, \
         on_chain_event_id, on_chain_event_id_text, refund_deadline_hours, max_refundable_deposits, \
         description, visibility, \
         require_contact_info, require_photo_consent, \
         in_person_capacity, online_capacity, \
         online_open_mode, online_registration_open, \
         deposit_deadline_hours, updated_by, dev_profile_enabled, community_links, \
         calendar_subscribe_url, poster_url, recap_published, location_map_url, \
         ticket_note_in_person, ticket_note_online, postponed_note, sponsors_json) \
         VALUES (?, ?, ?, ?, ?, \
         {event_start_ms}, {event_end_ms}, \
         {deposit_enabled}, {deposit_amount_usdc}, {deposit_amount_thb}, \
         ?, ?, ?, ?, \
         ?, ?, ?, \
         ?, ?, ?, \
         {capacity}, {total_attendees}, ?, ?, \
         ?, {time_tba}, {quiz_enabled}, \
         ?, ?, ?, \
         ?, ?, ?, \
         ?, ?, ?, \
         ?, ?, ?, \
         {on_chain_event_id}, '{on_chain_event_id}', {refund_deadline_hours}, {max_refundable_deposits}, \
         ?, ?, \
         {require_contact_info}, {require_photo_consent}, \
         {in_person_capacity}, {online_capacity}, \
         ?, {online_registration_open}, \
         {deposit_deadline_hours}, ?, {dev_profile_enabled}, ?, \
         ?, ?, {recap_published}, ?, \
         ?, ?, ?, ?) \
         ON CONFLICT (id) DO UPDATE SET \
         name = excluded.name, slug = excluded.slug, status = excluded.status, \
         event_format = excluded.event_format, \
         event_start_ms = excluded.event_start_ms, event_end_ms = excluded.event_end_ms, \
         deposit_enabled = excluded.deposit_enabled, \
         deposit_amount_usdc = excluded.deposit_amount_usdc, \
         deposit_amount_thb = excluded.deposit_amount_thb, \
         escrow_status = excluded.escrow_status, escrow_pda = excluded.escrow_pda, \
         location = excluded.location, tagline = excluded.tagline, \
         organizer_emails = excluded.organizer_emails, \
         organization_id = excluded.organization_id, \
         video_url = excluded.video_url, \
         sheet_id = excluded.sheet_id, sheet_name = excluded.sheet_name, \
         staff_sheet_name = excluded.staff_sheet_name, \
         capacity = excluded.capacity, total_attendees = excluded.total_attendees, \
         updated_at = excluded.updated_at, \
         link = excluded.link, time_tba = excluded.time_tba, \
         quiz_enabled = excluded.quiz_enabled, \
         nft_collection_mint = excluded.nft_collection_mint, \
         nft_metadata_uri = excluded.nft_metadata_uri, \
         nft_image_url = excluded.nft_image_url, \
         nft_name_template = excluded.nft_name_template, \
         nft_symbol = excluded.nft_symbol, \
         nft_description_template = excluded.nft_description_template, \
         merkle_tree = excluded.merkle_tree, \
         staff_emails = excluded.staff_emails, \
         claim_base_url = excluded.claim_base_url, \
         promptpay_id = excluded.promptpay_id, \
         escrow_address = excluded.escrow_address, \
         organizer_wallet = excluded.organizer_wallet, \
         on_chain_event_id = excluded.on_chain_event_id, \
         on_chain_event_id_text = excluded.on_chain_event_id_text, \
         refund_deadline_hours = excluded.refund_deadline_hours, \
         max_refundable_deposits = excluded.max_refundable_deposits, \
         description = excluded.description, visibility = excluded.visibility, \
         require_contact_info = excluded.require_contact_info, \
         require_photo_consent = excluded.require_photo_consent, \
         in_person_capacity = excluded.in_person_capacity, \
         online_capacity = excluded.online_capacity, \
         online_open_mode = excluded.online_open_mode, \
         online_registration_open = excluded.online_registration_open, \
         deposit_deadline_hours = excluded.deposit_deadline_hours, \
         updated_by = excluded.updated_by, \
         dev_profile_enabled = excluded.dev_profile_enabled, \
         community_links = excluded.community_links, \
         calendar_subscribe_url = excluded.calendar_subscribe_url, \
         poster_url = excluded.poster_url, \
         recap_published = excluded.recap_published, \
         location_map_url = excluded.location_map_url, \
         ticket_note_in_person = excluded.ticket_note_in_person, \
         ticket_note_online = excluded.ticket_note_online, \
         postponed_note = excluded.postponed_note, \
         sponsors_json = excluded.sponsors_json",
        event_start_ms = config.event_start_ms,
        event_end_ms = config.event_end_ms,
        deposit_enabled = config.deposit_enabled as i32,
        deposit_amount_usdc = config.deposit_amount_usdc,
        deposit_amount_thb = config.deposit_amount_thb,
        capacity = config.in_person_capacity.unwrap_or(0),
        total_attendees = 0,
        time_tba = config.time_tba as i32,
        quiz_enabled = config.quiz_enabled as i32,
        on_chain_event_id = config.on_chain_event_id,
        refund_deadline_hours = config.refund_deadline_hours,
        max_refundable_deposits = config.max_refundable_deposits,
        require_contact_info = config.require_contact_info as i32,
        require_photo_consent = config.require_photo_consent as i32,
        in_person_capacity = config.in_person_capacity.map(|v| v as i64).unwrap_or(-1),
        online_capacity = config.online_capacity.map(|v| v as i64).unwrap_or(-1),
        online_registration_open = config.online_registration_open as i32,
        deposit_deadline_hours = config
            .deposit_deadline_hours
            .map(|v| v as i64)
            .unwrap_or(-1),
        dev_profile_enabled = config.dev_profile_enabled as i32,
        recap_published = config.recap_published as i32,
    );

    // Order MUST match the `?` placeholders in the VALUES clause above.
    let args = [
        D1Type::Text(&config.id),
        D1Type::Text(&config.name),
        D1Type::Text(&config.slug),
        D1Type::Text(status_str),
        D1Type::Text(event_format_str),
        D1Type::Text(escrow_status_str),
        D1Type::Text(&config.escrow_address), // escrow_pda
        D1Type::Text(&config.location),
        D1Type::Text(&config.tagline),
        D1Type::Text(&organizer_emails),
        D1Type::Text(&config.organization_id),
        D1Type::Text(&config.video_url),
        D1Type::Text(&config.sheet_id),
        D1Type::Text(&config.sheet_name),
        D1Type::Text(&config.staff_sheet_name),
        D1Type::Text(&config.created_at),
        D1Type::Text(&config.updated_at),
        D1Type::Text(&config.link),
        D1Type::Text(&config.nft_collection_mint),
        D1Type::Text(&config.nft_metadata_uri),
        D1Type::Text(&config.nft_image_url),
        D1Type::Text(&config.nft_name_template),
        D1Type::Text(&config.nft_symbol),
        D1Type::Text(&config.nft_description_template),
        D1Type::Text(&config.merkle_tree),
        D1Type::Text(&staff_emails),
        D1Type::Text(&config.claim_base_url),
        D1Type::Text(&config.promptpay_id),
        D1Type::Text(&config.escrow_address),
        D1Type::Text(&config.organizer_wallet),
        D1Type::Text(&config.description),
        D1Type::Text(visibility_str),
        D1Type::Text(online_open_mode_str),
        D1Type::Text(&config.updated_by),
        D1Type::Text(&community_links_json),
        D1Type::Text(&config.calendar_subscribe_url),
        D1Type::Text(&config.poster_url),
        D1Type::Text(&config.location_map_url),
        D1Type::Text(&config.ticket_note_in_person),
        D1Type::Text(&config.ticket_note_online),
        D1Type::Text(&config.postponed_note),
        D1Type::Text(&sponsors_json),
    ];

    db.prepare(&sql)
        .bind_refs(&args)
        .map_err(|e| format!("D1 upsert_event bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 upsert_event: {e:?}"))?;

    Ok(())
}

/// Delete an event row from D1.
pub async fn delete_event(db: &D1Database, event_id: &str) -> Result<(), String> {
    db.prepare("DELETE FROM events WHERE id = ?")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 delete_event bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_event: {e:?}"))?;
    Ok(())
}
