//! Events Tab in the master contacts Google Sheet.
//!
//! Stores event metadata in a dedicated "Events" tab alongside the "Contacts"
//! tab, giving organizers a single-sheet view of their events + contacts.
//!
//! Sheet schema (columns A–R):
//!   A: event_id            | solana-bangkok-2025      | Unique event ID
//!   B: name                | Solana x AI Builders     | Display name
//!   C: slug                | solana-bangkok-2025      | URL slug
//!   D: status              | active                   | Draft/Active/Completed/Archived
//!   E: event_format        | in_person                | InPerson/Online/Hybrid
//!   F: event_start_ms      | 1777170600000            | Start timestamp (epoch ms)
//!   G: event_end_ms        | 1777183200000            | End timestamp (epoch ms)
//!   H: deposit_enabled     | true                     | Whether deposit is required
//!   I: deposit_amount_usdc | 15000000                 | USDC amount (6 decimals)
//!   J: deposit_amount_thb  | 500                      | THB amount
//!   K: escrow_status       | initialized              | None/Initialized/Deactivated/Closed
//!   L: location            | Bangkok                  | Venue
//!   M: tagline             | Deep Dive...             | Subtitle
//!   N: organizer_emails    | alice@x.com,bob@y.com    | Comma-separated
//!   O: total_attendees     | 42                       | Attendee count (updated on sync)
//!   P: created_at          | 2025-03-15T10:00:00Z     | ISO 8601
//!   Q: organization_id     | solana-thailand          | Org ID (empty = global)
//!   R: video_url           | https://youtube.com/...  | Video/livestream URL

use worker::KvStore;

use crate::http::{ValueRange, post_json};
use crate::state::AppState;
use event_checkin_domain::models::event::EventConfig;

use super::get_cached_access_token;

// ---------------------------------------------------------------------------
// Column indices (0-based)
// ---------------------------------------------------------------------------

const COL_EVENT_ID: usize = 0;
const COL_NAME: usize = 1;
const COL_SLUG: usize = 2;
const COL_STATUS: usize = 3;
const COL_EVENT_FORMAT: usize = 4;
const COL_EVENT_START_MS: usize = 5;
const COL_EVENT_END_MS: usize = 6;
const COL_DEPOSIT_ENABLED: usize = 7;
const COL_DEPOSIT_AMOUNT_USDC: usize = 8;
const COL_DEPOSIT_AMOUNT_THB: usize = 9;
const COL_ESCROW_STATUS: usize = 10;
const COL_LOCATION: usize = 11;
const COL_TAGLINE: usize = 12;
const COL_ORGANIZER_EMAILS: usize = 13;
const COL_TOTAL_ATTENDEES: usize = 14;
const COL_CREATED_AT: usize = 15;
const COL_ORGANIZATION_ID: usize = 16;
const COL_VIDEO_URL: usize = 17;

const TOTAL_COLUMNS: usize = 18;

// ---------------------------------------------------------------------------
// Public: upsert event row
// ---------------------------------------------------------------------------

/// Write or update an event's row in the Events tab.
///
/// - If the event_id already exists in the sheet, updates the row in-place.
/// - If not found, appends a new row.
/// - Non-fatal: errors are logged but do not propagate.
pub async fn upsert_event_tab(
    config: &EventConfig,
    total_attendees: usize,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    let row = event_config_to_row(config, total_attendees);

    // Find existing row by event_id
    let existing_row = find_event_row(&config.id, sheet_id, sheet_name, &access_token).await?;

    match existing_row {
        Some(row_index) => {
            update_event_row(row_index, &row, sheet_id, sheet_name, &access_token).await?;
            tracing::info!(
                event_id = %config.id,
                row_index,
                "updated event row in Events tab"
            );
        }
        None => {
            append_event_row(&row, sheet_id, sheet_name, &access_token).await?;
            tracing::info!(
                event_id = %config.id,
                "appended new event row to Events tab"
            );
        }
    }

    Ok(())
}

/// Delete an event row from the Events tab.
/// Used when an event is hard-deleted.
pub async fn delete_event_tab(
    event_id: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    let existing_row = find_event_row(event_id, sheet_id, sheet_name, &access_token).await?;

    if let Some(row_index) = existing_row {
        // Write empty row to clear the data
        let empty_row = vec![String::new(); TOTAL_COLUMNS];
        update_event_row(row_index, &empty_row, sheet_id, sheet_name, &access_token).await?;
        tracing::info!(event_id, row_index, "cleared event row in Events tab");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public: read events from tab
// ---------------------------------------------------------------------------

/// Event row from the Events tab.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventTabRow {
    pub event_id: String,
    pub name: String,
    pub slug: String,
    pub status: String,
    pub event_format: String,
    pub event_start_ms: String,
    pub event_end_ms: String,
    pub deposit_enabled: bool,
    pub deposit_amount_usdc: String,
    pub deposit_amount_thb: String,
    pub escrow_status: String,
    pub location: String,
    pub tagline: String,
    pub organizer_emails: String,
    pub total_attendees: usize,
    pub created_at: String,
    pub organization_id: String,
    pub video_url: String,
}

/// Read all events from the Events tab.
pub async fn list_events_tab(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Vec<EventTabRow>, String> {
    let access_token = get_cached_access_token(state, kv).await?;

    let range = format!("{sheet_name}!A:R");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let response: ValueRange = crate::http::get_json(&url, &access_token).await?;

    let mut events = Vec::new();
    for row in &response.values {
        if row.is_empty() {
            continue;
        }
        let event_id = row.first().map(|s| s.as_str()).unwrap_or("");
        if event_id.eq_ignore_ascii_case("event_id") || event_id.is_empty() {
            continue;
        }

        events.push(EventTabRow {
            event_id: event_id.to_string(),
            name: row.get(COL_NAME).cloned().unwrap_or_default(),
            slug: row.get(COL_SLUG).cloned().unwrap_or_default(),
            status: row.get(COL_STATUS).cloned().unwrap_or_default(),
            event_format: row.get(COL_EVENT_FORMAT).cloned().unwrap_or_default(),
            event_start_ms: row.get(COL_EVENT_START_MS).cloned().unwrap_or_default(),
            event_end_ms: row.get(COL_EVENT_END_MS).cloned().unwrap_or_default(),
            deposit_enabled: row
                .get(COL_DEPOSIT_ENABLED)
                .map(|s| s.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            deposit_amount_usdc: row
                .get(COL_DEPOSIT_AMOUNT_USDC)
                .cloned()
                .unwrap_or_default(),
            deposit_amount_thb: row.get(COL_DEPOSIT_AMOUNT_THB).cloned().unwrap_or_default(),
            escrow_status: row.get(COL_ESCROW_STATUS).cloned().unwrap_or_default(),
            location: row.get(COL_LOCATION).cloned().unwrap_or_default(),
            tagline: row.get(COL_TAGLINE).cloned().unwrap_or_default(),
            organizer_emails: row.get(COL_ORGANIZER_EMAILS).cloned().unwrap_or_default(),
            total_attendees: row
                .get(COL_TOTAL_ATTENDEES)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            created_at: row.get(COL_CREATED_AT).cloned().unwrap_or_default(),
            organization_id: row.get(COL_ORGANIZATION_ID).cloned().unwrap_or_default(),
            video_url: row.get(COL_VIDEO_URL).cloned().unwrap_or_default(),
        });
    }

    Ok(events)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert an EventConfig + attendee count into a sheet row.
fn event_config_to_row(config: &EventConfig, total_attendees: usize) -> Vec<String> {
    let mut row = vec![String::new(); TOTAL_COLUMNS];
    row[COL_EVENT_ID] = config.id.clone();
    row[COL_NAME] = config.name.clone();
    row[COL_SLUG] = config.slug.clone();
    row[COL_STATUS] = config.status.as_str().to_string();
    row[COL_EVENT_FORMAT] = config.event_format.as_str().to_string();
    row[COL_EVENT_START_MS] = config.event_start_ms.to_string();
    row[COL_EVENT_END_MS] = config.event_end_ms.to_string();
    row[COL_DEPOSIT_ENABLED] = config.deposit_enabled.to_string();
    row[COL_DEPOSIT_AMOUNT_USDC] = config.deposit_amount_usdc.to_string();
    row[COL_DEPOSIT_AMOUNT_THB] = config.deposit_amount_thb.to_string();
    row[COL_ESCROW_STATUS] = config.escrow_status.as_str().to_string();
    row[COL_LOCATION] = config.location.clone();
    row[COL_TAGLINE] = config.tagline.clone();
    row[COL_ORGANIZER_EMAILS] = config.organizer_emails.join(",");
    row[COL_TOTAL_ATTENDEES] = total_attendees.to_string();
    row[COL_CREATED_AT] = config.created_at.clone();
    row[COL_ORGANIZATION_ID] = config.organization_id.clone();
    row[COL_VIDEO_URL] = config.video_url.clone();
    row
}

/// Find a row by event_id in the Events tab.
/// Returns the 1-based row index if found.
async fn find_event_row(
    event_id: &str,
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<Option<usize>, String> {
    let range = format!("{sheet_name}!A:A");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let response: ValueRange = crate::http::get_json(&url, access_token).await?;

    for (i, row) in response.values.iter().enumerate() {
        let id = row.first().map(|s| s.as_str()).unwrap_or("");
        if id == event_id {
            return Ok(Some(i + 2)); // +2: 1-based + skip header
        }
    }

    Ok(None)
}

/// Update an existing event row by writing all columns A–P.
async fn update_event_row(
    row_index: usize,
    row_data: &[String],
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<(), String> {
    let range = format!("{sheet_name}!A{row_index}:R{row_index}");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}?valueInputOption=USER_ENTERED",
        urlencoding::encode(&range)
    );

    let body = ValueRange {
        range: format!("{sheet_name}!A{row_index}:R{row_index}"),
        values: vec![row_data.to_vec()],
    };

    put_json_ignore(&url, &body, access_token).await?;

    Ok(())
}

/// Append a new event row at the end of the sheet.
async fn append_event_row(
    row: &[String],
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<(), String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}!A:R:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        urlencoding::encode(sheet_name)
    );

    let body = ValueRange {
        range: format!("{sheet_name}!A:R"),
        values: vec![row.to_vec()],
    };

    let _: serde_json::Value = post_json(&url, &body, Some(access_token)).await?;
    Ok(())
}

/// PUT JSON body, ignore response body (only check status).
async fn put_json_ignore(
    url: &str,
    body: &impl serde::Serialize,
    access_token: &str,
) -> Result<(), String> {
    let json_body =
        serde_json::to_string(body).map_err(|e| format!("failed to serialize JSON body: {e}"))?;

    let headers = worker::Headers::new();
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;
    headers
        .set("Authorization", &format!("Bearer {access_token}"))
        .map_err(|e| format!("failed to set auth header: {e:?}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Put)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(url, &init)
        .map_err(|e| format!("failed to create PUT request to {url}: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("PUT {url} failed: {e:?}"))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("PUT {url} returned {status}: {text}"));
    }

    Ok(())
}
