//! Sheet mutation operations: check-in, claim, QR URL updates, and row appends.

use chrono::Utc;
use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};
use crate::state::AppState;

use super::{get_cached_access_token, invalidate_attendee_cache, invalidate_column_map_cache};

// ---------------------------------------------------------------------------
// Sheet mutations
// ---------------------------------------------------------------------------

/// Mark an attendee as checked in by updating:
/// - Column I: checked_in_at timestamp (ISO 8601)
/// - Column J: checked_in_by staff email
/// - Column R: claim_token (UUID v7 for NFT/refund claim link)
///
/// Uses batch update to write all columns in a single API call.
#[allow(clippy::too_many_arguments)]
pub async fn mark_checked_in(
    row_index: usize,
    staff_email: &str,
    claim_token: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let timestamp = Utc::now().to_rfc3339();

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let col_checked_in_at = mapping.column_letter(CK::CheckedInAt);
    let col_checked_in_by = mapping.column_letter(CK::CheckedInBy);
    let col_claim_token = mapping.column_letter(CK::ClaimToken);

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_at}{row_index}"),
            values: vec![vec![timestamp.clone()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_by}{row_index}"),
            values: vec![vec![staff_email.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_claim_token}{row_index}"),
            values: vec![vec![claim_token.to_string()]],
        },
    ];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        staff_email = %staff_email,
        claim_token = %claim_token,
        "marked row as checked in"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(timestamp)
}

/// Mark an online attendee as virtually checked in.
/// Writes checked_in_at (column R) and checked_in_by="virtual" (column S).
/// Does NOT overwrite claim_token (column V) — already set during registration.
pub async fn mark_virtual_checked_in(
    row_index: usize,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let access_token = get_cached_access_token(state, kv).await?;
    let timestamp = Utc::now().to_rfc3339();

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let col_checked_in_at = mapping.column_letter(CK::CheckedInAt);
    let col_checked_in_by = mapping.column_letter(CK::CheckedInBy);

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_at}{row_index}"),
            values: vec![vec![timestamp.clone()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_by}{row_index}"),
            values: vec![vec!["virtual".to_string()]],
        },
    ];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        "marked row as virtually checked in (online attendee)"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(timestamp)
}

/// Undo a check-in by clearing:
/// - checked_in_at (column R)
/// - checked_in_by (column S)
/// - claim_token (column V)
/// - claimed_at (column W)
///
/// Reverses the effect of `mark_checked_in` so the attendee can be re-checked-in.
pub async fn clear_checked_in(
    row_index: usize,
    staff_email: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let col_checked_in_at = mapping.column_letter(CK::CheckedInAt);
    let col_checked_in_by = mapping.column_letter(CK::CheckedInBy);
    let col_claim_token = mapping.column_letter(CK::ClaimToken);
    let col_claimed_at = mapping.column_letter(CK::ClaimedAt);

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_at}{row_index}"),
            values: vec![vec![String::new()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_checked_in_by}{row_index}"),
            values: vec![vec![String::new()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_claim_token}{row_index}"),
            values: vec![vec![String::new()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_claimed_at}{row_index}"),
            values: vec![vec![String::new()]],
        },
    ];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        staff_email = %staff_email,
        "cleared check-in fields (undo)"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Mark an attendee as claimed by writing wallet and claimed_at columns.
/// Called after a successful cNFT mint to persist the claim on the Google Sheet.
#[allow(clippy::too_many_arguments)]
pub async fn mark_claimed(
    row_index: usize,
    wallet_address: &str,
    claimed_at: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let col_solana = mapping.column_letter(CK::SolanaAddress);
    let col_claimed_at = mapping.column_letter(CK::ClaimedAt);

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!{col_solana}{row_index}"),
            values: vec![vec![wallet_address.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_claimed_at}{row_index}"),
            values: vec![vec![claimed_at.to_string()]],
        },
    ];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(row_index = row_index, wallet_address = %wallet_address, "marked row as claimed");

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    Ok(claimed_at.to_string())
}

/// Bulk update QR code URLs for approved attendees.
/// Updates column Q (qr_code_url) for each attendee.
pub async fn update_qr_urls(
    updates: &[(usize, String)],
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<usize, String> {
    if updates.is_empty() {
        return Ok(0);
    }

    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;
    let col_qr = mapping.column_letter(CK::QrCodeUrl);

    // Build batch update with individual value ranges
    let data: Vec<ValueRange> = updates
        .iter()
        .map(|(row_index, url)| ValueRange {
            range: format!("{sheet_name}!{col_qr}{row_index}"),
            values: vec![vec![url.clone()]],
        })
        .collect();

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    let updated = updates.len();
    tracing::info!(count = updated, "updated qr code urls in google sheets");

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(updated)
}

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
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    // Determine row width: at least enough for all mapped columns
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

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
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

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}
