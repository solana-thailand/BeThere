//! Row append, delete, participation type update, and cell clear operations.

use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};
use crate::state::AppState;

use crate::sheets::{get_cached_access_token, invalidate_column_map_cache};

// ---------------------------------------------------------------------------
// Self-registration append
// ---------------------------------------------------------------------------

/// Append a full attendee row to the event's Google Sheet.
///
/// Columns A–AB (28 columns), unused slots filled with empty strings:
/// A[0]=api_id, B[1]=name, C[2]=first_name, D[3]=last_name, E[4]=email,
/// F[5]="Self-Registered", G[6]=registration_date, H[7]="Approved",
/// R[17]=claim_token, I[8]=participation_type.
#[allow(clippy::too_many_arguments)]
pub async fn append_attendee_row(
    api_id: &str,
    name: &str,
    first_name: &str,
    last_name: &str,
    email: &str,
    claim_token: &str,
    participation_type: &str,
    registration_date: &str,
    contact_channel: Option<&str>,
    contact_handle: Option<&str>,
    deposit_agreed: bool,
    consent_given: bool,
    photo_consent_given: bool,
    consent_marketing: Option<bool>,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    // Determine row width: at least enough for all mapped columns
    let row_len = mapping.total_columns.max(31);
    let mut row = vec![String::new(); row_len];

    let set = |row: &mut Vec<String>, key: CK, val: String| {
        let idx = mapping.get_or_default(key);
        if idx < row.len() {
            row[idx] = val;
        }
    };

    set(&mut row, CK::ApiId, api_id.to_string());
    set(&mut row, CK::Name, name.to_string());
    set(&mut row, CK::FirstName, first_name.to_string());
    set(&mut row, CK::LastName, last_name.to_string());
    set(&mut row, CK::Email, email.to_string());
    set(&mut row, CK::TicketName, "Self-Registered".to_string());
    set(
        &mut row,
        CK::RegistrationDate,
        registration_date.to_string(),
    );
    set(&mut row, CK::ApprovalStatus, "Approved".to_string());
    set(&mut row, CK::ClaimToken, claim_token.to_string());
    set(
        &mut row,
        CK::ParticipationType,
        participation_type.to_string(),
    );
    if let Some(channel) = contact_channel {
        set(&mut row, CK::ContactChannel, channel.to_string());
    }
    if let Some(handle) = contact_handle {
        set(&mut row, CK::ContactHandle, handle.to_string());
    }
    if deposit_agreed {
        set(&mut row, CK::DepositAgreed, "Yes".to_string());
    }
    if consent_given {
        set(&mut row, CK::ConsentGiven, "Yes".to_string());
    }
    if photo_consent_given {
        set(&mut row, CK::PhotoConsent, "Yes".to_string());
    }
    if consent_marketing.unwrap_or(false) {
        set(&mut row, CK::ConsentMarketing, "Yes".to_string());
    }

    // Determine the last non-empty column to build the range
    let last_col_idx = row.iter().rposition(|v| !v.is_empty()).unwrap_or(0);
    let last_col_letter = {
        let mut result = String::new();
        let mut n = last_col_idx;
        loop {
            result.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = (n / 26) - 1;
        }
        result
    };

    // Truncate trailing empty columns
    row.truncate(last_col_idx + 1);

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}!A:{last_col_letter}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        urlencoding::encode(sheet_name)
    );

    let body = crate::http::ValueRange {
        range: format!("{sheet_name}!A:{last_col_letter}"),
        values: vec![row],
    };

    crate::http::post_json::<serde_json::Value>(&url, &body, Some(&access_token)).await?;

    tracing::info!(
        %api_id,
        %email,
        %participation_type,
        "appended self-registration row to google sheet"
    );

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Walk-in Sheet Append
// ---------------------------------------------------------------------------

/// Append a walk-in attendee row to the event's Google Sheet.
///
/// Uses the column mapping to place walk-in-specific fields (checked_in_at,
/// checked_in_by, phone, wallet_address, claimed_at) into the correct columns.
#[allow(clippy::too_many_arguments)]
pub async fn append_walkin_row(
    api_id: &str,
    name: &str,
    email: &str,
    phone: Option<&str>,
    claim_token: &str,
    checked_in_at: &str,
    checked_in_by: &str,
    wallet_address: Option<&str>,
    claimed_at: Option<&str>,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let row_len = mapping.total_columns.max(28);
    let mut row = vec![String::new(); row_len];

    let set = |row: &mut Vec<String>, key: CK, val: String| {
        let idx = mapping.get_or_default(key);
        if idx < row.len() {
            row[idx] = val;
        }
    };

    set(&mut row, CK::ApiId, api_id.to_string());
    set(&mut row, CK::Name, name.to_string());
    set(&mut row, CK::Email, email.to_string());
    set(&mut row, CK::TicketName, "Walk-in".to_string());
    set(&mut row, CK::ApprovalStatus, "CheckedIn".to_string());
    set(&mut row, CK::ParticipationType, "In-Person".to_string());
    set(&mut row, CK::ClaimToken, claim_token.to_string());
    set(&mut row, CK::CheckedInAt, checked_in_at.to_string());
    set(&mut row, CK::CheckedInBy, checked_in_by.to_string());

    if let Some(phone) = phone {
        set(&mut row, CK::Phone, phone.to_string());
    }
    if let Some(wallet) = wallet_address {
        set(&mut row, CK::SolanaAddress, wallet.to_string());
    }
    if let Some(claimed) = claimed_at {
        set(&mut row, CK::ClaimedAt, claimed.to_string());
    }

    // Determine the last non-empty column to build the range
    let last_col_idx = row.iter().rposition(|v| !v.is_empty()).unwrap_or(0);
    let last_col_letter = {
        let mut result = String::new();
        let mut n = last_col_idx;
        loop {
            result.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = (n / 26) - 1;
        }
        result
    };

    row.truncate(last_col_idx + 1);

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}!A:{last_col_letter}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
        urlencoding::encode(sheet_name)
    );

    let body = crate::http::ValueRange {
        range: format!("{sheet_name}!A:{last_col_letter}"),
        values: vec![row],
    };

    crate::http::post_json::<serde_json::Value>(&url, &body, Some(&access_token)).await?;

    tracing::info!(
        %api_id,
        %email,
        "appended walk-in row to google sheet"
    );

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Row Deletion
// ---------------------------------------------------------------------------

/// Delete a row from the Google Sheet by removing the entire row dimension.
///
/// Uses `spreadsheets.batchUpdate` with `DeleteDimensionRequest` to remove the row,
/// which shifts subsequent rows up (no gaps left behind).
/// All caches are invalidated after deletion since row indices shift.
#[allow(clippy::too_many_arguments)]
pub async fn delete_sheet_row(
    row_index: usize,
    _mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    // Use batchUpdate with DeleteDimensionRequest to remove the entire row
    // This shifts all subsequent rows up, leaving no gaps
    let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}:batchUpdate");

    // Google Sheets API uses 0-based indices for dimensions
    // row_index from the sheet is 1-based, so subtract 1
    let zero_based_row = if row_index > 0 { row_index - 1 } else { 0 };

    let body = serde_json::json!({
        "requests": [{
            "deleteDimension": {
                "range": {
                    "sheetId": resolve_sheet_gid(sheet_name),
                    "dimension": "ROWS",
                    "startIndex": zero_based_row,
                    "endIndex": zero_based_row + 1
                }
            }
        }]
    });

    let headers = worker::Headers::new();
    headers
        .set("Authorization", &format!("Bearer {access_token}"))
        .map_err(|e| format!("failed to set auth header: {e:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize batch update: {e}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(&url, &init)
        .map_err(|e| format!("failed to create delete request: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("failed to send delete request: {e:?}"))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("sheet row delete failed (HTTP {status}): {body}"));
    }

    tracing::info!(
        row_index = row_index,
        sheet_name = %sheet_name,
        "deleted sheet row (dimension removed)"
    );

    // Invalidate all caches — row indices have shifted
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Resolve a sheet tab name to its numeric GID for dimension operations.
///
/// The `deleteDimension` API requires a `sheetId` (numeric GID), not the tab name.
/// Common defaults: first tab = 0, "Attendees" = 0, "Staff" = 1.
/// Falls back to 0 for the first/default tab.
pub(super) fn resolve_sheet_gid(sheet_name: &str) -> i64 {
    match sheet_name.to_lowercase().as_str() {
        "attendees" => 0,
        "staff" => 1,
        // Default to first tab — covers most cases
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Participation Type Update (deposit deadline auto-switch)
// ---------------------------------------------------------------------------

/// Update a single attendee's participation_type cell in the Google Sheet.
///
/// Used by the deposit deadline auto-switch to change "In-Person" → "Online"
/// when an attendee misses the deposit deadline.
#[allow(clippy::too_many_arguments)]
pub async fn update_participation_type(
    row_index: usize,
    new_value: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;
    let col = mapping.column_letter(CK::ParticipationType);

    let data = vec![ValueRange {
        range: format!("{sheet_name}!{col}{row_index}"),
        values: vec![vec![new_value.to_string()]],
    }];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        new_value = new_value,
        "updated participation_type in google sheet"
    );

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Clear PII cells for specific ranges in the attendee sheet (PDPA data erasure).
/// Uses the batch clear API to wipe values in multiple ranges without affecting formatting.
pub async fn clear_sheet_cells_batch(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    ranges: &[String],
    kv: Option<&KvStore>,
) -> Result<(), String> {
    if ranges.is_empty() {
        return Ok(());
    }

    let access_token = get_cached_access_token(state, kv).await?;

    // Build full A1-notation ranges (prepend sheet name)
    let full_ranges: Vec<String> = ranges.iter().map(|r| format!("{sheet_name}!{r}")).collect();

    // Use batchClear to wipe values in multiple ranges
    // POST /v4/spreadsheets/{id}/values:batchClear
    let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchClear");

    let body = serde_json::json!({
        "ranges": full_ranges
    });

    let headers = worker::Headers::new();
    headers
        .set("Authorization", &format!("Bearer {access_token}"))
        .map_err(|e| format!("failed to set auth header: {e:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("failed to set content-type: {e:?}"))?;

    let json_body = serde_json::to_string(&body)
        .map_err(|e| format!("failed to serialize batch clear: {e}"))?;

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = worker::Request::new_with_init(&url, &init)
        .map_err(|e| format!("failed to create batch clear request: {e:?}"))?;

    let mut response = worker::Fetch::Request(request)
        .send()
        .await
        .map_err(|e| format!("failed to send batch clear request: {e:?}"))?;

    let status = response.status_code();
    if !(200..300).contains(&status) {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("sheet batch clear failed (HTTP {status}): {body}"));
    }

    tracing::info!(ranges = ?full_ranges, "cleared PII cells in sheet");
    Ok(())
}
