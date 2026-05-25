//! Sheet mutation operations: check-in, claim, QR URL updates, and row appends.

use chrono::Utc;
use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};
use crate::state::AppState;

use super::{get_cached_access_token, invalidate_attendee_cache, invalidate_column_map_cache};

// ---------------------------------------------------------------------------
// Sheet write context — bundles repeated sheet operation parameters
// ---------------------------------------------------------------------------

/// Bundles the parameters commonly passed to sheet mutation functions.
/// Reduces argument count and avoids repeating sheet_id/sheet_name/kv everywhere.
pub struct SheetContext<'a> {
    pub mapping: &'a ColumnMapping,
    pub state: &'a AppState,
    pub sheet_id: &'a str,
    pub sheet_name: &'a str,
    pub kv: Option<&'a KvStore>,
}

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

/// Mark an attendee as claimed by writing wallet, claimed_at, and nft_proof_url columns.
/// Called after a successful cNFT mint to persist the claim on the Google Sheet.
#[allow(clippy::too_many_arguments)]
pub async fn mark_claimed(
    row_index: usize,
    wallet_address: &str,
    claimed_at: &str,
    nft_proof_url: &str,
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
    let col_nft_proof_url = mapping.column_letter(CK::NftProofUrl);

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!{col_solana}{row_index}"),
            values: vec![vec![wallet_address.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_claimed_at}{row_index}"),
            values: vec![vec![claimed_at.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!{col_nft_proof_url}{row_index}"),
            values: vec![vec![nft_proof_url.to_string()]],
        },
    ];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(row_index = row_index, wallet_address = %wallet_address, nft_proof_url = %nft_proof_url, "marked row as claimed");

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
    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Resolve a sheet tab name to its numeric GID for dimension operations.
///
/// The `deleteDimension` API requires a `sheetId` (numeric GID), not the tab name.
/// Common defaults: first tab = 0, "Attendees" = 0, "Staff" = 1.
/// Falls back to 0 for the first/default tab.
fn resolve_sheet_gid(sheet_name: &str) -> i64 {
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

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Write bank account info (bank_account, bank_name, account_name) to the sheet.
/// Used when THB deposit slip is uploaded so the organizer has refund details.
#[allow(clippy::too_many_arguments)]
pub async fn write_bank_info(
    row_index: usize,
    bank_account: Option<&str>,
    bank_name: Option<&str>,
    account_name: Option<&str>,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    // Skip if nothing to write
    if bank_account.is_none() && bank_name.is_none() && account_name.is_none() {
        return Ok(());
    }

    let access_token = get_cached_access_token(state, kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let mut data = Vec::new();

    if let Some(val) = bank_account {
        let col = mapping.column_letter(CK::BankAccount);
        data.push(ValueRange {
            range: format!("{sheet_name}!{col}{row_index}"),
            values: vec![vec![val.to_string()]],
        });
    }
    if let Some(val) = bank_name {
        let col = mapping.column_letter(CK::BankName);
        data.push(ValueRange {
            range: format!("{sheet_name}!{col}{row_index}"),
            values: vec![vec![val.to_string()]],
        });
    }
    if let Some(val) = account_name {
        let col = mapping.column_letter(CK::AccountName);
        data.push(ValueRange {
            range: format!("{sheet_name}!{col}{row_index}"),
            values: vec![vec![val.to_string()]],
        });
    }

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        bank_account_col = mapping.column_letter(CK::BankAccount).as_str(),
        bank_name_col = mapping.column_letter(CK::BankName).as_str(),
        account_name_col = mapping.column_letter(CK::AccountName).as_str(),
        bank_account_val = ?bank_account,
        bank_name_val = ?bank_name,
        account_name_val = ?account_name,
        "wrote bank info to google sheet"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Deposit verification — write N (deposit_method), O (deposit_amount), Q (deposit_verified)
// ---------------------------------------------------------------------------

/// Write deposit verification columns to the Google Sheet.
/// Called when a deposit is verified (THB slip approved or USDC on-chain confirmed).
pub async fn write_deposit_verification(
    row_index: usize,
    deposit_method: &str,
    deposit_amount: &str,
    verified: bool,
    ctx: &SheetContext<'_>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(ctx.state, ctx.kv).await?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;

    let col_method = ctx.mapping.column_letter(CK::DepositMethod);
    let col_amount = ctx.mapping.column_letter(CK::DepositAmount);
    let col_verified = ctx.mapping.column_letter(CK::DepositVerified);

    let data = vec![
        ValueRange {
            range: format!("{}!{}{}", ctx.sheet_name, col_method, row_index),
            values: vec![vec![deposit_method.to_string()]],
        },
        ValueRange {
            range: format!("{}!{}{}", ctx.sheet_name, col_amount, row_index),
            values: vec![vec![deposit_amount.to_string()]],
        },
        ValueRange {
            range: format!("{}!{}{}", ctx.sheet_name, col_verified, row_index),
            values: vec![vec![if verified {
                "Yes".to_string()
            } else {
                "No".to_string()
            }]],
        },
    ];

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
        ctx.sheet_id
    );

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        row_index = row_index,
        method = %deposit_method,
        amount = %deposit_amount,
        verified = verified,
        "wrote deposit verification to google sheet"
    );

    invalidate_attendee_cache(ctx.kv, ctx.sheet_id, ctx.sheet_name).await;
    Ok(())
}

/// Update the deposit_method column (N) for an attendee, found by api_id.
/// Used when a rolling deposit credit covers the deposit — writes "credit_thb" or "credit_usdc".
pub async fn update_deposit_method(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    attendee_api_id: &str,
    method: &str,
) -> Result<(), String> {
    let access_token = get_cached_access_token(state, kv).await?;

    // Find the attendee row by api_id
    let mapping = super::get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = super::get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;
    let col = mapping.column_letter(CK::DepositMethod);

    let data = vec![ValueRange {
        range: format!("{sheet_name}!{col}{row_index}"),
        values: vec![vec![method.to_string()]],
    }];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        %method,
        "wrote credit deposit_method to google sheet"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Write refund status to the Google Sheet (column AA: refund_status).
/// Called after admin marks a refund as processed.
pub async fn write_refund_status(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    attendee_api_id: &str,
    status: &str,
) -> Result<(), String> {
    // Invalidate column map cache to ensure fresh header mapping
    super::invalidate_column_map_cache(kv, sheet_id, sheet_name).await;

    let access_token = get_cached_access_token(state, kv).await?;

    let mapping = super::get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = super::get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;
    let col = mapping.column_letter(CK::RefundStatus);
    tracing::info!(
        %attendee_api_id,
        row_index,
        column = %col,
        total_columns = mapping.total_columns,
        "resolved refund_status column"
    );

    let data = vec![ValueRange {
        range: format!("{sheet_name}!{col}{row_index}"),
        values: vec![vec![status.to_string()]],
    }];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        %status,
        "wrote refund_status to google sheet"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Write refund link to the Google Sheet (column AC: refund_link).
/// Called when organizer provides a refund link for an attendee.
pub async fn write_refund_link(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    attendee_api_id: &str,
    link: &str,
) -> Result<(), String> {
    // Invalidate column map cache to ensure fresh header mapping
    super::invalidate_column_map_cache(kv, sheet_id, sheet_name).await;

    let access_token = get_cached_access_token(state, kv).await?;

    let mapping = super::get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = super::get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    use event_checkin_domain::models::attendee::ColumnKey as CK;
    let col = mapping.column_letter(CK::RefundLink);
    tracing::info!(
        %attendee_api_id,
        row_index,
        column = %col,
        total_columns = mapping.total_columns,
        "resolved refund_link column"
    );

    let data = vec![ValueRange {
        range: format!("{sheet_name}!{col}{row_index}"),
        values: vec![vec![link.to_string()]],
    }];

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        "wrote refund_link to google sheet"
    );

    invalidate_attendee_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}
