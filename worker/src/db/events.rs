//! D1 event query helpers.
//!
//! Phase 2d: Dual-write events to D1 alongside KV.
//! Write path: upsert on every create/update/seed.
//! Read path: D1-first fallback when KV is empty (e.g. after data loss).

use worker::D1Database;

// ---------------------------------------------------------------------------
// D1 row type matching the full events table
// ---------------------------------------------------------------------------

/// Full event row from D1, used for reconstructing EventConfig when KV is empty.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct D1EventRow {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub status: Option<String>,
    pub event_format: Option<String>,
    pub event_start_ms: Option<i64>,
    pub event_end_ms: Option<i64>,
    pub deposit_enabled: Option<i64>,
    pub deposit_amount_usdc: Option<i64>,
    pub deposit_amount_thb: Option<i64>,
    pub escrow_status: Option<String>,
    pub escrow_pda: Option<String>,
    pub location: Option<String>,
    pub tagline: Option<String>,
    pub organizer_emails: Option<String>,
    pub organization_id: Option<String>,
    pub video_url: Option<String>,
    pub sheet_id: Option<String>,
    pub sheet_name: Option<String>,
    pub staff_sheet_name: Option<String>,
    pub capacity: Option<i64>,
    pub total_attendees: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // Columns added by migration 0003
    pub link: Option<String>,
    pub time_tba: Option<i64>,
    pub quiz_enabled: Option<i64>,
    pub nft_collection_mint: Option<String>,
    pub nft_metadata_uri: Option<String>,
    pub nft_image_url: Option<String>,
    pub nft_name_template: Option<String>,
    pub nft_symbol: Option<String>,
    pub nft_description_template: Option<String>,
    pub merkle_tree: Option<String>,
    pub staff_emails: Option<String>,
    pub claim_base_url: Option<String>,
    pub promptpay_id: Option<String>,
    pub escrow_address: Option<String>,
    pub organizer_wallet: Option<String>,
    pub on_chain_event_id: Option<i64>,
    pub refund_deadline_hours: Option<i64>,
    pub max_refundable_deposits: Option<i64>,
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub require_contact_info: Option<i64>,
    pub require_photo_consent: Option<i64>,
    pub in_person_capacity: Option<i64>,
    pub online_capacity: Option<i64>,
    pub online_open_mode: Option<String>,
    pub online_registration_open: Option<i64>,
    pub deposit_deadline_hours: Option<i64>,
    pub updated_by: Option<String>,
    // Columns added for Issue #049
    pub dev_profile_enabled: Option<i64>,
    // Columns added for community links
    pub community_links: Option<String>,
}

impl D1EventRow {
    /// Convert D1 row to domain EventConfig.
    /// Uses `unwrap_or_default` for columns added by migration 0003
    /// so it's safe even if the migration hasn't run yet.
    pub fn to_event_config(&self) -> event_checkin_domain::models::event::EventConfig {
        use event_checkin_domain::models::event::*;

        let status: EventStatus = serde_json::from_value(serde_json::Value::String(
            self.status.clone().unwrap_or_default(),
        ))
        .unwrap_or_default();
        let escrow_status: EscrowStatus = serde_json::from_value(serde_json::Value::String(
            self.escrow_status.clone().unwrap_or_default(),
        ))
        .unwrap_or_default();
        let event_format: EventFormat = serde_json::from_value(serde_json::Value::String(
            self.event_format.clone().unwrap_or_default(),
        ))
        .unwrap_or_default();
        let visibility: EventVisibility = serde_json::from_value(serde_json::Value::String(
            self.visibility
                .clone()
                .unwrap_or_else(|| "public".to_string()),
        ))
        .unwrap_or_default();
        let online_open_mode: OnlineOpenMode = serde_json::from_value(serde_json::Value::String(
            self.online_open_mode
                .clone()
                .unwrap_or_else(|| "auto_on_full".to_string()),
        ))
        .unwrap_or_default();

        let organizer_emails: Vec<String> = self
            .organizer_emails
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let staff_emails: Vec<String> = self
            .staff_emails
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        EventConfig {
            id: self.id.clone().unwrap_or_default(),
            name: self.name.clone().unwrap_or_default(),
            slug: self.slug.clone().unwrap_or_default(),
            tagline: self.tagline.clone().unwrap_or_default(),
            link: self.link.clone().unwrap_or_default(),
            status,
            event_start_ms: self.event_start_ms.unwrap_or(0),
            event_end_ms: self.event_end_ms.unwrap_or(0),
            time_tba: self.time_tba.unwrap_or(0) == 1,
            sheet_id: self.sheet_id.clone().unwrap_or_default(),
            sheet_name: self.sheet_name.clone().unwrap_or_default(),
            staff_sheet_name: self.staff_sheet_name.clone().unwrap_or_default(),
            quiz_enabled: self.quiz_enabled.unwrap_or(0) == 1,
            nft_collection_mint: self.nft_collection_mint.clone().unwrap_or_default(),
            nft_metadata_uri: self.nft_metadata_uri.clone().unwrap_or_default(),
            nft_image_url: self.nft_image_url.clone().unwrap_or_default(),
            nft_name_template: self.nft_name_template.clone().unwrap_or_default(),
            nft_symbol: self.nft_symbol.clone().unwrap_or_default(),
            nft_description_template: self.nft_description_template.clone().unwrap_or_default(),
            merkle_tree: self.merkle_tree.clone().unwrap_or_default(),
            organization_id: self.organization_id.clone().unwrap_or_default(),
            organizer_emails,
            staff_emails,
            claim_base_url: self.claim_base_url.clone().unwrap_or_default(),
            deposit_enabled: self.deposit_enabled.unwrap_or(0) != 0,
            deposit_amount_usdc: self.deposit_amount_usdc.unwrap_or(0) as u64,
            deposit_amount_thb: self.deposit_amount_thb.unwrap_or(0) as u64,
            promptpay_id: self.promptpay_id.clone().unwrap_or_default(),
            escrow_address: self.escrow_address.clone().unwrap_or_default(),
            escrow_status,
            organizer_wallet: self.organizer_wallet.clone().unwrap_or_default(),
            on_chain_event_id: self.on_chain_event_id.unwrap_or(0) as u64,
            refund_deadline_hours: self.refund_deadline_hours.unwrap_or(168) as u32,
            max_refundable_deposits: self.max_refundable_deposits.unwrap_or(0) as u32,
            description: self.description.clone().unwrap_or_default(),
            location: self.location.clone().unwrap_or_default(),
            video_url: self.video_url.clone().unwrap_or_default(),
            event_format,
            require_contact_info: self.require_contact_info.unwrap_or(1) == 1,
            require_photo_consent: self.require_photo_consent.unwrap_or(0) == 1,
            in_person_capacity: self.in_person_capacity.map(|v| v as u32),
            online_capacity: self.online_capacity.map(|v| v as u32),
            online_open_mode,
            online_registration_open: self.online_registration_open.unwrap_or(0) == 1,
            deposit_deadline_hours: self.deposit_deadline_hours.map(|v| v as u32),
            visibility,
            created_at: self.created_at.clone().unwrap_or_default(),
            updated_at: self.updated_at.clone().unwrap_or_default(),
            updated_by: self.updated_by.clone().unwrap_or_default(),
            dev_profile_enabled: self.dev_profile_enabled.unwrap_or(0) == 1,
            community_links: serde_json::from_str(
                &self
                    .community_links
                    .clone()
                    .unwrap_or_else(|| "[]".to_string()),
            )
            .unwrap_or_default(),
        }
    }
}

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
         on_chain_event_id, refund_deadline_hours, max_refundable_deposits, \
         description, visibility, \
         require_contact_info, require_photo_consent, \
         in_person_capacity, online_capacity, \
         online_open_mode, online_registration_open, \
         deposit_deadline_hours, updated_by, dev_profile_enabled, community_links) \
         VALUES ('{id}', '{name}', '{slug}', '{status}', '{event_format}', \
         {event_start_ms}, {event_end_ms}, \
         {deposit_enabled}, {deposit_amount_usdc}, {deposit_amount_thb}, \
         '{escrow_status}', '{escrow_pda}', '{location}', '{tagline}', \
         '{organizer_emails}', '{organization_id}', '{video_url}', \
         '{sheet_id}', '{sheet_name}', '{staff_sheet_name}', \
         {capacity}, {total_attendees}, '{created_at}', '{updated_at}', \
         '{link}', {time_tba}, {quiz_enabled}, \
         '{nft_collection_mint}', '{nft_metadata_uri}', '{nft_image_url}', \
         '{nft_name_template}', '{nft_symbol}', '{nft_description_template}', \
         '{merkle_tree}', '{staff_emails}', '{claim_base_url}', \
         '{promptpay_id}', '{escrow_address}', '{organizer_wallet}', \
         {on_chain_event_id}, {refund_deadline_hours}, {max_refundable_deposits}, \
         '{description}', '{visibility}', \
         {require_contact_info}, {require_photo_consent}, \
         {in_person_capacity}, {online_capacity}, \
         '{online_open_mode}', {online_registration_open}, \
         {deposit_deadline_hours}, '{updated_by}', {dev_profile_enabled}, '{community_links}') \
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
         community_links = excluded.community_links",
        id = config.id,
        name = config.name.replace('\'', "''"),
        slug = config.slug,
        status = status_str,
        event_format = event_format_str,
        event_start_ms = config.event_start_ms,
        event_end_ms = config.event_end_ms,
        deposit_enabled = config.deposit_enabled as i32,
        deposit_amount_usdc = config.deposit_amount_usdc,
        deposit_amount_thb = config.deposit_amount_thb,
        escrow_status = escrow_status_str,
        escrow_pda = config.escrow_address,
        location = config.location.replace('\'', "''"),
        tagline = config.tagline.replace('\'', "''"),
        organizer_emails = organizer_emails.replace('\'', "''"),
        organization_id = config.organization_id,
        video_url = config.video_url,
        sheet_id = config.sheet_id,
        sheet_name = config.sheet_name,
        staff_sheet_name = config.staff_sheet_name,
        capacity = config.in_person_capacity.unwrap_or(0),
        total_attendees = 0,
        created_at = config.created_at,
        updated_at = config.updated_at,
        link = config.link.replace('\'', "''"),
        time_tba = config.time_tba as i32,
        quiz_enabled = config.quiz_enabled as i32,
        nft_collection_mint = config.nft_collection_mint,
        nft_metadata_uri = config.nft_metadata_uri,
        nft_image_url = config.nft_image_url,
        nft_name_template = config.nft_name_template.replace('\'', "''"),
        nft_symbol = config.nft_symbol,
        nft_description_template = config.nft_description_template.replace('\'', "''"),
        merkle_tree = config.merkle_tree,
        staff_emails = staff_emails.replace('\'', "''"),
        claim_base_url = config.claim_base_url,
        promptpay_id = config.promptpay_id,
        escrow_address = config.escrow_address,
        organizer_wallet = config.organizer_wallet,
        on_chain_event_id = config.on_chain_event_id,
        refund_deadline_hours = config.refund_deadline_hours,
        max_refundable_deposits = config.max_refundable_deposits,
        description = config.description.replace('\'', "''"),
        visibility = visibility_str,
        require_contact_info = config.require_contact_info as i32,
        require_photo_consent = config.require_photo_consent as i32,
        in_person_capacity = config.in_person_capacity.map(|v| v as i64).unwrap_or(-1),
        online_capacity = config.online_capacity.map(|v| v as i64).unwrap_or(-1),
        online_open_mode = online_open_mode_str,
        online_registration_open = config.online_registration_open as i32,
        deposit_deadline_hours = config
            .deposit_deadline_hours
            .map(|v| v as i64)
            .unwrap_or(-1),
        updated_by = config.updated_by,
        dev_profile_enabled = config.dev_profile_enabled as i32,
        community_links =
            serde_json::to_string(&config.community_links).unwrap_or_else(|_| "[]".to_string()),
    );

    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 upsert_event: {e:?}"))?;

    Ok(())
}

/// Delete an event row from D1.
pub async fn delete_event(db: &D1Database, event_id: &str) -> Result<(), String> {
    let sql = format!("DELETE FROM events WHERE id = '{event_id}'");
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 delete_event: {e:?}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Read operations
// ---------------------------------------------------------------------------

/// Get a single event by ID from D1.
/// Returns None if not found.
pub async fn get_event(db: &D1Database, event_id: &str) -> Result<Option<D1EventRow>, String> {
    let sql = format!("SELECT * FROM events WHERE id = '{event_id}' LIMIT 1");
    let result = db
        .prepare(&sql)
        .first::<D1EventRow>(None)
        .await
        .map_err(|e| format!("D1 get_event query: {e:?}"))?;
    Ok(result)
}

/// Get the first active event from D1.
pub async fn get_active_event(db: &D1Database) -> Result<Option<D1EventRow>, String> {
    let sql = "SELECT * FROM events WHERE status = 'active' ORDER BY created_at DESC LIMIT 1";
    let result = db
        .prepare(sql)
        .first::<D1EventRow>(None)
        .await
        .map_err(|e| format!("D1 get_active_event query: {e:?}"))?;
    Ok(result)
}

/// Get an event from D1 by slug.
pub async fn get_event_by_slug(db: &D1Database, slug: &str) -> Result<Option<D1EventRow>, String> {
    let sql = format!("SELECT * FROM events WHERE slug = '{slug}' LIMIT 1");
    let result = db
        .prepare(&sql)
        .first::<D1EventRow>(None)
        .await
        .map_err(|e| format!("D1 get_event_by_slug query: {e:?}"))?;
    Ok(result)
}

/// List all events from D1 (for index rebuild).
pub async fn list_events(db: &D1Database) -> Result<Vec<D1EventRow>, String> {
    let sql = "SELECT * FROM events ORDER BY created_at DESC";
    let result = db
        .prepare(sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_events: {e:?}"))?;
    result
        .results()
        .map_err(|e| format!("D1 list_events results: {e:?}"))
}

/// List all events from D1 as `EventMeta` (for KV fallback).
pub async fn list_events_as_meta(
    db: &D1Database,
) -> Result<Vec<event_checkin_domain::models::event::EventMeta>, String> {
    let rows = list_events(db).await?;
    Ok(rows.iter().map(|r| r.to_event_config().to_meta()).collect())
}
