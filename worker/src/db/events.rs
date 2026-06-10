//! D1 event query helpers.
//!
//! Phase 2d: Dual-write events to D1 alongside KV.
//! Write path: upsert on every create/update/seed.
//! Read path: D1-first fallback when KV is empty (e.g. after data loss).

use wasm_bindgen_futures::JsFuture;
use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// D1 row type matching the full events table
// ---------------------------------------------------------------------------

/// Full event row from D1, used for reconstructing EventConfig when KV is empty.
///
/// Uses `#[serde(default)]` so that columns added by future migrations deserialize as
/// `None` / zero values even when the migration hasn't been applied to production yet.
#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
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
    // Columns added for Issue #053 Phase 3f
    pub form_config: Option<String>,
    // Columns added for organization calendar subscribe
    pub calendar_subscribe_url: Option<String>,
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
            in_person_capacity: self
                .in_person_capacity
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
            online_capacity: self
                .online_capacity
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
            online_open_mode,
            online_registration_open: self.online_registration_open.unwrap_or(0) == 1,
            deposit_deadline_hours: self
                .deposit_deadline_hours
                .and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
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
            calendar_subscribe_url: self.calendar_subscribe_url.clone().unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Form config D1 helpers (Issue #053 Phase 3f)
// ---------------------------------------------------------------------------

/// Get form config for an event from D1.
/// Returns None if no custom config is stored.
pub async fn get_form_config(
    db: &D1Database,
    event_id: &str,
) -> Result<Option<event_checkin_domain::models::event::RegistrationFormConfig>, String> {
    let sql = format!("SELECT form_config FROM events WHERE id = '{event_id}' LIMIT 1");
    let bound = db.prepare(&sql);

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_form_config first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_form_config first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_form_config: deserialize failed"
        );
        format!("D1 get_form_config deserialize: {e}")
    })?;

    let config_str = row.get("form_config").and_then(|v| v.as_str());
    match config_str {
        Some(json) if !json.is_empty() => serde_json::from_str(json)
            .map(Some)
            .map_err(|e| format!("failed to parse form config for event '{event_id}': {e}")),
        _ => Ok(None),
    }
}

/// Save form config for an event to D1.
pub async fn save_form_config(
    db: &D1Database,
    event_id: &str,
    config: &event_checkin_domain::models::event::RegistrationFormConfig,
) -> Result<(), String> {
    let json_str = serde_json::to_string(config)
        .map_err(|e| format!("failed to serialize form config: {e:?}"))?;
    // Escape single quotes for SQL
    let json_escaped = json_str.replace('\'', "''");
    let sql = format!("UPDATE events SET form_config = '{json_escaped}' WHERE id = '{event_id}'");
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 save_form_config: {e:?}"))?;
    Ok(())
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
         deposit_deadline_hours, updated_by, dev_profile_enabled, community_links, \
         calendar_subscribe_url) \
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
         {deposit_deadline_hours}, '{updated_by}', {dev_profile_enabled}, '{community_links}', \
         '{calendar_subscribe_url}') \
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
         community_links = excluded.community_links, \
         calendar_subscribe_url = excluded.calendar_subscribe_url",
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
        calendar_subscribe_url = config.calendar_subscribe_url,
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
    get_event_raw(db, "id", event_id).await
}

/// Get an event from D1 by column value using raw JSON deserialization.
///
/// Bypasses `first::<D1EventRow>()` which uses `serde_wasm_bindgen::from_value`
/// with `.unwrap()` — panics on certain nullable column types.
async fn get_event_raw(
    db: &D1Database,
    column: &str,
    value: &str,
) -> Result<Option<D1EventRow>, String> {
    let sql = format!("SELECT * FROM events WHERE {column} = '{value}' LIMIT 1");
    let stmt = db.prepare(&sql);
    let raw_first = JsFuture::from(
        stmt.inner()
            .first(None)
            .map_err(|e| format!("D1 get_event_raw first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_event_raw first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: D1EventRow = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 get_event_raw deserialize: {e:?}"))?;
    Ok(Some(row))
}

/// Get the first active event from D1.
pub async fn get_active_event(db: &D1Database) -> Result<Option<D1EventRow>, String> {
    let sql = "SELECT * FROM events WHERE status = 'active' ORDER BY created_at DESC LIMIT 1";
    let stmt = db.prepare(sql);
    let raw_first = JsFuture::from(
        stmt.inner()
            .first(None)
            .map_err(|e| format!("D1 get_active_event first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_active_event first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: D1EventRow = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 get_active_event deserialize: {e:?}"))?;
    Ok(Some(row))
}

/// Get an event from D1 by slug.
pub async fn get_event_by_slug(db: &D1Database, slug: &str) -> Result<Option<D1EventRow>, String> {
    get_event_raw(db, "slug", slug).await
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

/// Raw column subset for public event listings.
/// Avoids `results::<D1EventRow>()` which can panic on nullable columns
/// when deserialized via the workers-rs serde bridge.
#[derive(serde::Deserialize)]
struct PublicEventRow {
    id: Option<String>,
    name: Option<String>,
    slug: Option<String>,
    status: Option<String>,
    event_format: Option<String>,
    event_start_ms: Option<i64>,
    event_end_ms: Option<i64>,
    time_tba: Option<i64>,
    deposit_enabled: Option<i64>,
    tagline: Option<String>,
    location: Option<String>,
    nft_image_url: Option<String>,
    created_at: Option<String>,
    in_person_capacity: Option<i64>,
    online_capacity: Option<i64>,
    visibility: Option<String>,
}

/// List public events from D1 using raw JSON deserialization.
/// Bypasses `results::<T>()` to avoid workers-rs serde panics on nullable columns.
pub async fn list_public_events_raw(db: &D1Database) -> Result<Vec<serde_json::Value>, String> {
    use event_checkin_domain::models::event::*;

    let sql = "SELECT id, name, slug, status, event_format, event_start_ms, event_end_ms, \
               time_tba, deposit_enabled, tagline, location, nft_image_url, \
               created_at, in_person_capacity, online_capacity, visibility \
               FROM events ORDER BY created_at DESC";

    // Bypass workers-rs D1Result::results() which uses serde_wasm_bindgen::from_value
    // with .unwrap() — panics on nullable columns or type mismatches.
    // Instead, use .all() on the inner JsValue, then stringify and parse via serde_json.
    let stmt = db.prepare(sql);
    let raw_result = JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 list_public_events_raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 list_public_events_raw all() await: {e:?}"))?;

    // Extract the `results` array from the D1 response object
    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);

    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let rows: Vec<PublicEventRow> = serde_json::from_str(&json_str).unwrap_or_default();

    Ok(rows
        .into_iter()
        .map(|r| {
            let status: EventStatus = serde_json::from_value(serde_json::Value::String(
                r.status.clone().unwrap_or_default(),
            ))
            .unwrap_or_default();
            let event_format: EventFormat = serde_json::from_value(serde_json::Value::String(
                r.event_format.clone().unwrap_or_default(),
            ))
            .unwrap_or_default();
            let visibility: EventVisibility = serde_json::from_value(serde_json::Value::String(
                r.visibility
                    .clone()
                    .unwrap_or_else(|| "public".to_string()),
            ))
            .unwrap_or_default();

            serde_json::json!({
                "id": r.id.unwrap_or_default(),
                "name": r.name.unwrap_or_default(),
                "slug": r.slug.unwrap_or_default(),
                "status": status.as_str(),
                "event_start_ms": r.event_start_ms.unwrap_or(0),
                "event_end_ms": r.event_end_ms.unwrap_or(0),
                "time_tba": r.time_tba.unwrap_or(0) == 1,
                "deposit_enabled": r.deposit_enabled.unwrap_or(0) != 0,
                "event_format": event_format.as_str(),
                "tagline": r.tagline.unwrap_or_default(),
                "location": r.location.unwrap_or_default(),
                "nft_image_url": r.nft_image_url.unwrap_or_default(),
                "created_at": r.created_at.unwrap_or_default(),
                "in_person_capacity": r.in_person_capacity.and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
                "online_capacity": r.online_capacity.and_then(|v| if v >= 0 { Some(v as u32) } else { None }),
                "visibility": visibility.as_str(),
            })
        })
        .collect())
}

/// Check whether an organization has any non-archived events.
pub async fn has_active_events_for_org(db: &D1Database, org_id: &str) -> Result<bool, String> {
    let stmt = db.prepare(
        "SELECT 1 AS found FROM events WHERE organization_id = ?1 AND status != 'Archived' LIMIT 1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 has_active_events_for_org bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 has_active_events_for_org first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 has_active_events_for_org first() await: {e:?}"))?;

    Ok(!raw_first.is_null() && !raw_first.is_undefined())
}

/// Count non-archived events for an organization.
pub async fn count_active_events_for_org(db: &D1Database, org_id: &str) -> Result<usize, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) AS cnt FROM events WHERE organization_id = ?1 AND status != 'Archived'",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(org_id)])
        .map_err(|e| format!("D1 count_active_events_for_org bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_active_events_for_org first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_active_events_for_org first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(0);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
    Ok(row.get("cnt").and_then(|c| c.as_i64()).unwrap_or(0) as usize)
}
