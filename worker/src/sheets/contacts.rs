//! Master Contacts Sheet operations.
//!
//! Maintains a deduplicated list of all attendees across all events.
//! Each row is keyed by email (lowercased) and tracks which events
//! the contact has joined, along with contact info and email status.
//!
//! Sheet schema (columns A–M):
//!   A: email              | john@gmail.com       | Primary key (lowercased)
//!   B: name               | John Doe             | Display name
//!   C: first_registered   | 2025-03-15           | First event date
//!   D: last_registered    | 2026-05-22           | Most recent event date
//!   E: events_joined      | evt_abc,evt_xyz      | Comma-separated event IDs
//!   F: event_count        | 2                    | Number of events
//!   G: contact_channel    | Telegram             | Preferred contact channel
//!   H: contact_handle     | @john                | Handle
//!   I: send_email_status  | pending              | Last bulk email status
//!   J: last_emailed_at    | 2026-05-22           | Last email timestamp
//!   K: deposit_credit_thb | 500                  | Rolling deposit credit (THB)
//!   L: deposit_credit_usdc| 15                   | Rolling deposit credit (USDC)
//!   M: deposit_credit_since| 2026-05-22          | Date when credit was first held
//!   N: credit_refund_req   | 1                    | Attendee requested return of held credit (Issue #061 Phase 3)

use worker::KvStore;

use crate::http::{ValueRange, post_json};
use crate::state::AppState;

use super::get_cached_access_token;

// ---------------------------------------------------------------------------
// Column indices (0-based)
// ---------------------------------------------------------------------------

const COL_EMAIL: usize = 0;
const COL_NAME: usize = 1;
const COL_FIRST_REGISTERED: usize = 2;
const COL_LAST_REGISTERED: usize = 3;
const COL_EVENTS_JOINED: usize = 4;
const COL_EVENT_COUNT: usize = 5;
const COL_CONTACT_CHANNEL: usize = 6;
const COL_CONTACT_HANDLE: usize = 7;
const COL_SEND_EMAIL_STATUS: usize = 8;
const COL_LAST_EMAILED_AT: usize = 9;
const COL_DEPOSIT_CREDIT_THB: usize = 10; // K
const COL_DEPOSIT_CREDIT_USDC: usize = 11; // L
const COL_DEPOSIT_CREDIT_SINCE: usize = 12; // M
// N: credit_refund_requested — Issue #061 Phase 3 exit path. "1" = attendee
// has requested return of their held credit; empty/"0" = no open request.
const COL_CREDIT_REFUND_REQUESTED: usize = 13; // N

const TOTAL_COLUMNS: usize = 14;

// ---------------------------------------------------------------------------
// Upsert contact
// ---------------------------------------------------------------------------

/// Input data for upserting a contact.
pub struct ContactUpsert<'a> {
    pub email: &'a str,
    pub name: &'a str,
    pub event_id: &'a str,
    pub contact_channel: Option<&'a str>,
    pub contact_handle: Option<&'a str>,
}

/// Upsert a contact into the master contacts sheet.
///
/// - If the email doesn't exist, appends a new row.
/// - If the email already exists, updates the row: appends event_id to
///   `events_joined`, increments `event_count`, updates `last_registered`
///   and refreshes `name`/`contact_channel`/`contact_handle` with latest values.
///
/// Non-fatal: errors are logged but do not propagate to avoid blocking registration.
pub async fn upsert_contact(
    upsert: &ContactUpsert<'_>,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let email_lower = upsert.email.to_lowercase();
    let now = chrono::Utc::now().to_rfc3339();

    // 1. Find existing row by email
    let existing_row = find_contact_row(&email_lower, sheet_id, sheet_name, &access_token).await?;

    match existing_row {
        Some((row_index, mut row_data)) => {
            // Update existing row
            // Name: update to latest
            row_data[COL_NAME] = upsert.name.to_string();
            // Last registered: update timestamp
            row_data[COL_LAST_REGISTERED] = now.clone();
            // Events joined: append event_id if not already present
            let events_str = row_data.get(COL_EVENTS_JOINED).cloned().unwrap_or_default();
            let mut events: Vec<String> = events_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !events.iter().any(|e| e == upsert.event_id) {
                events.push(upsert.event_id.to_string());
            }
            row_data[COL_EVENTS_JOINED] = events.join(",");
            // Event count: recalculate from events list
            row_data[COL_EVENT_COUNT] = events.len().to_string();
            // Contact channel/handle: update if provided
            if let Some(ch) = upsert.contact_channel {
                row_data[COL_CONTACT_CHANNEL] = ch.to_string();
            }
            if let Some(handle) = upsert.contact_handle {
                row_data[COL_CONTACT_HANDLE] = handle.to_string();
            }

            // Write updated row
            update_contact_row(row_index, &row_data, sheet_id, sheet_name, &access_token).await?;

            tracing::info!(
                %email_lower,
                row_index,
                event_count = row_data[COL_EVENT_COUNT],
                "updated existing contact in master sheet"
            );
        }
        None => {
            // Append new row
            let mut row = vec![String::new(); TOTAL_COLUMNS];
            row[COL_EMAIL] = email_lower.clone();
            row[COL_NAME] = upsert.name.to_string();
            row[COL_FIRST_REGISTERED] = now.clone();
            row[COL_LAST_REGISTERED] = now.clone();
            row[COL_EVENTS_JOINED] = upsert.event_id.to_string();
            row[COL_EVENT_COUNT] = "1".to_string();
            if let Some(ch) = upsert.contact_channel {
                row[COL_CONTACT_CHANNEL] = ch.to_string();
            }
            if let Some(handle) = upsert.contact_handle {
                row[COL_CONTACT_HANDLE] = handle.to_string();
            }
            row[COL_SEND_EMAIL_STATUS] = String::new();
            row[COL_LAST_EMAILED_AT] = String::new();

            append_contact_row(&row, sheet_id, sheet_name, &access_token).await?;

            tracing::info!(
                %email_lower,
                event_id = %upsert.event_id,
                "appended new contact to master sheet"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Fetch the entire contacts sheet and find a row by email.
/// Returns (1-based row index, row data) if found.
async fn find_contact_row(
    email: &str,
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<Option<(usize, Vec<String>)>, String> {
    let range = format!("{sheet_name}!A:N");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let response: ValueRange = crate::http::get_json(&url, access_token).await?;

    let rows = response.values;
    for (i, row) in rows.iter().enumerate() {
        let row_email = row.first().map(|s| s.to_lowercase()).unwrap_or_default();
        if row_email == email {
            // Pad row to TOTAL_COLUMNS
            let mut padded = row.clone();
            padded.resize(TOTAL_COLUMNS, String::new());
            return Ok(Some((i + 2, padded))); // +2: 1-based + skip header
        }
    }

    Ok(None)
}

/// Update an existing contact row by writing all columns A–J.
async fn update_contact_row(
    row_index: usize,
    row_data: &[String],
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<(), String> {
    let range = format!("{sheet_name}!A{row_index}:N{row_index}");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}?valueInputOption=USER_ENTERED",
        urlencoding::encode(&range)
    );

    let body = ValueRange {
        range: format!("{sheet_name}!A{row_index}:N{row_index}"),
        values: vec![row_data.to_vec()],
    };

    // Use PUT to update existing range
    put_json_ignore(&url, &body, access_token).await?;

    Ok(())
}

/// Append a new contact row at the end of the sheet.
async fn append_contact_row(
    row: &[String],
    sheet_id: &str,
    sheet_name: &str,
    access_token: &str,
) -> Result<(), String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}!A:N:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        urlencoding::encode(sheet_name)
    );

    let body = ValueRange {
        range: format!("{sheet_name}!A:N"),
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

    // Check status
    let status = response.status_code();
    if !(200..300).contains(&status) {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("PUT {url} returned {status}: {text}"));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Public: read all contacts
// ---------------------------------------------------------------------------

/// A single contact row from the master contacts sheet.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Contact {
    pub email: String,
    pub name: String,
    pub first_registered: String,
    pub last_registered: String,
    pub events_joined: String,
    pub event_count: u32,
    pub contact_channel: String,
    pub contact_handle: String,
    pub send_email_status: String,
    pub last_emailed_at: String,
    pub deposit_credit_thb: u64,
    pub deposit_credit_usdc: u64,
    pub deposit_credit_since: String,
}

/// Read all contacts from the master contacts sheet.
pub async fn list_contacts(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Vec<Contact>, String> {
    let access_token = get_cached_access_token(state, kv).await?;

    let range = format!("{sheet_name}!A:N");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let response: ValueRange = crate::http::get_json(&url, &access_token).await?;

    let mut contacts = Vec::new();
    for row in &response.values {
        if row.is_empty() {
            continue;
        }
        // Skip header row if present
        let email = row.first().map(|s| s.as_str()).unwrap_or("");
        if email.eq_ignore_ascii_case("email") {
            continue;
        }

        contacts.push(Contact {
            email: email.to_string(),
            name: row.get(COL_NAME).cloned().unwrap_or_default(),
            first_registered: row.get(COL_FIRST_REGISTERED).cloned().unwrap_or_default(),
            last_registered: row.get(COL_LAST_REGISTERED).cloned().unwrap_or_default(),
            events_joined: row.get(COL_EVENTS_JOINED).cloned().unwrap_or_default(),
            event_count: row
                .get(COL_EVENT_COUNT)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            contact_channel: row.get(COL_CONTACT_CHANNEL).cloned().unwrap_or_default(),
            contact_handle: row.get(COL_CONTACT_HANDLE).cloned().unwrap_or_default(),
            send_email_status: row.get(COL_SEND_EMAIL_STATUS).cloned().unwrap_or_default(),
            last_emailed_at: row.get(COL_LAST_EMAILED_AT).cloned().unwrap_or_default(),
            deposit_credit_thb: row
                .get(COL_DEPOSIT_CREDIT_THB)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            deposit_credit_usdc: row
                .get(COL_DEPOSIT_CREDIT_USDC)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            deposit_credit_since: row
                .get(COL_DEPOSIT_CREDIT_SINCE)
                .cloned()
                .unwrap_or_default(),
        });
    }

    Ok(contacts)
}

// ---------------------------------------------------------------------------
// Public: deposit credit operations
// ---------------------------------------------------------------------------

/// Increment a contact's deposit credit balance after they hold their deposit.
/// Called when attendee chooses "Hold Deposit" instead of "Claim Refund".
pub async fn increment_credit(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    email: &str,
    currency: &str,
    amount: u64,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let email_lower = email.to_lowercase();

    // 1. Find existing row by email
    let (row_index, mut row_data) =
        find_contact_row(&email_lower, sheet_id, sheet_name, &access_token)
            .await?
            .ok_or_else(|| format!("contact not found: {email_lower}"))?;

    // 2. Determine which credit column to update
    let credit_col = match currency.to_lowercase().as_str() {
        "thb" => COL_DEPOSIT_CREDIT_THB,
        "usdc" => COL_DEPOSIT_CREDIT_USDC,
        other => return Err(format!("unsupported currency: {other}")),
    };

    // 3. Parse current value and add amount
    let current: u64 = row_data
        .get(credit_col)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let new_total = current + amount;
    row_data[credit_col] = new_total.to_string();

    // 4. Set deposit_credit_since to today if currently empty
    let since = row_data
        .get(COL_DEPOSIT_CREDIT_SINCE)
        .map(|s| s.trim())
        .unwrap_or("");
    if since.is_empty() {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        row_data[COL_DEPOSIT_CREDIT_SINCE] = today;
    }

    // 5. Update the row
    update_contact_row(row_index, &row_data, sheet_id, sheet_name, &access_token).await?;

    tracing::info!(
        %email_lower,
        %currency,
        amount,
        new_total,
        row_index,
        "incremented deposit credit for contact"
    );

    Ok(())
}

/// Get a contact's deposit credit balance.
pub async fn get_credit_balance(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    email: &str,
) -> Result<(u64, u64), String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let email_lower = email.to_lowercase();

    let (_row_index, row_data) =
        find_contact_row(&email_lower, sheet_id, sheet_name, &access_token)
            .await?
            .ok_or_else(|| format!("contact not found: {email_lower}"))?;

    let credit_thb: u64 = row_data
        .get(COL_DEPOSIT_CREDIT_THB)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let credit_usdc: u64 = row_data
        .get(COL_DEPOSIT_CREDIT_USDC)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    Ok((credit_thb, credit_usdc))
}

/// Set the `credit_refund_requested` flag on a contact (Issue #061 Phase 3
/// exit path). Attendee-side "Request Return of Held Credit" action — writes
/// "1" to column N so the organizer sees the request surface in the admin
/// Held-as-Credit tab.
///
/// Idempotent: a re-request just re-writes "1" (no per-click counter for v1).
/// The organizer clears the flag through the existing refund tooling after
/// processing the payout (no automated state machine — Issue #061 §D3).
///
/// Mirrors [`increment_credit`] structure: find row → mutate in place → update
/// row. The find path lowercases the email; `find_contact_row` already pads
/// rows to `TOTAL_COLUMNS`, so the column-N index is always in bounds.
pub async fn set_credit_refund_requested(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    email: &str,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let email_lower = email.to_lowercase();

    // 1. Find existing row by email
    let (row_index, mut row_data) =
        find_contact_row(&email_lower, sheet_id, sheet_name, &access_token)
            .await?
            .ok_or_else(|| format!("contact not found: {email_lower}"))?;

    // 2. Set column N to "1" — find_contact_row pads to TOTAL_COLUMNS so the
    //    index is in bounds even for rows last written before this column
    //    existed (a pre-Phase-3 contact row will have an empty string at N,
    //    which we now overwrite with "1").
    row_data[COL_CREDIT_REFUND_REQUESTED] = "1".to_string();

    // 3. Update the row (writes all columns A:N; columns we did not touch are
    //    re-written verbatim from the find step, so no data loss).
    update_contact_row(row_index, &row_data, sheet_id, sheet_name, &access_token).await?;

    tracing::info!(
        %email_lower,
        row_index,
        "credit refund requested on contact (sheet)"
    );

    Ok(())
}

/// Clears the `credit_refund_requested` flag (column N) on a contact row in
/// the human-readable Sheets master — the sibling of
/// [`set_credit_refund_requested`] that was missing at Phase 3 ship (Issue #061
/// §D3, closes the Sheets clear-sync gap).
///
/// Mirrors [`set_credit_refund_requested`] structure: find row → mutate in
/// place → update row. The find path lowercases the email; `find_contact_row`
/// already pads rows to `TOTAL_COLUMNS`, so the column-N index is always in
/// bounds.
///
/// **Semantic difference from `set_credit_refund_requested`:** a missing
/// contact row is a silent no-op (`Ok(())`) rather than an error. Clearing a
/// non-existent contact is idempotent — there is nothing to clear. This
/// mirrors the D1 `clear_credit_refund_requested`, whose `UPDATE ... WHERE
/// email = ?` affects 0 rows silently when the contact doesn't exist.
///
/// Writes `""` (empty) rather than `"0"` so the column is restored to its
/// default pre-Phase-3 state (matching rows last written before this column
/// existed). The read side treats both empty and `"0"` as "no open request".
pub async fn clear_credit_refund_requested(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    email: &str,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let email_lower = email.to_lowercase();

    // 1. Find existing row by email. A missing row is a silent no-op (the
    //    D1 clear has the same semantics — 0 rows affected).
    let Some((row_index, mut row_data)) =
        find_contact_row(&email_lower, sheet_id, sheet_name, &access_token).await?
    else {
        tracing::debug!(
            %email_lower,
            "clear_credit_refund_requested: contact not in sheet — no-op"
        );
        return Ok(());
    };

    // 2. Clear column N back to empty (default state). find_contact_row pads
    //    to TOTAL_COLUMNS so the index is in bounds even for rows last
    //    written before this column existed.
    row_data[COL_CREDIT_REFUND_REQUESTED] = String::new();

    // 3. Update the row (writes all columns A:N; columns we did not touch are
    //    re-written verbatim from the find step, so no data loss).
    update_contact_row(row_index, &row_data, sheet_id, sheet_name, &access_token).await?;

    tracing::info!(
        %email_lower,
        row_index,
        "credit refund request cleared on contact (sheet)"
    );

    Ok(())
}
