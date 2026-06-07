//! Check-in, claim, and QR URL mutation operations.

use chrono::Utc;
use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};
use crate::state::AppState;

use crate::sheets::{get_cached_access_token, invalidate_column_map_cache};

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

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(updated)
}
