//! Check-in, claim, and QR URL mutation operations.

use crate::sheets::a1;
use chrono::Utc;
use event_checkin_domain::models::attendee::{ColumnKey as CK, ColumnMapping};
use worker::KvStore;

use crate::http::ValueRange;
use crate::state::AppState;

use crate::sheets::locate::{resolve_row, resolve_rows};
use crate::sheets::values::{send, write_cells};
use crate::sheets::{get_cached_access_token, invalidate_column_map_cache};
use event_checkin_domain::models::attendee::SheetRow;

/// Mark an attendee as checked in by updating:
/// - Column I: checked_in_at timestamp (ISO 8601)
/// - Column J: checked_in_by staff email
/// - Column R: claim_token (UUID v7 for NFT/refund claim link)
///
/// Uses batch update to write all columns in a single API call.
#[allow(clippy::too_many_arguments)]
pub async fn mark_checked_in(
    row: SheetRow,
    staff_email: &str,
    claim_token: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    let access_token = get_cached_access_token(state, kv).await?;
    let row_index = resolve_row(&row, sheet_id, &sheet_ref, &access_token).await?;
    let timestamp = Utc::now().to_rfc3339();

    let cells = vec![
        (CK::CheckedInAt, timestamp.clone()),
        (CK::CheckedInBy, staff_email.to_string()),
        (CK::ClaimToken, claim_token.to_string()),
    ];
    write_cells(
        sheet_id,
        &sheet_ref,
        mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        row_index = row_index,
        staff_fingerprint = %state.log_fingerprint(staff_email),
        claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token),
        "marked row as checked in"
    );

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(timestamp)
}

/// Mark an online attendee as virtually checked in.
/// Writes checked_in_at (column R) and checked_in_by="virtual" (column S).
/// Does NOT overwrite claim_token (column V) — already set during registration.
pub async fn mark_virtual_checked_in(
    row: SheetRow,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    let access_token = get_cached_access_token(state, kv).await?;
    let row_index = resolve_row(&row, sheet_id, &sheet_ref, &access_token).await?;
    let timestamp = Utc::now().to_rfc3339();

    let cells = vec![
        (CK::CheckedInAt, timestamp.clone()),
        (CK::CheckedInBy, "virtual".to_string()),
    ];
    write_cells(
        sheet_id,
        &sheet_ref,
        mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

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
    row: SheetRow,
    staff_email: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    let access_token = get_cached_access_token(state, kv).await?;
    let row_index = resolve_row(&row, sheet_id, &sheet_ref, &access_token).await?;

    let cells = [
        CK::CheckedInAt,
        CK::CheckedInBy,
        CK::ClaimToken,
        CK::ClaimedAt,
    ]
    .into_iter()
    .map(|key| (key, String::new()))
    .collect();
    write_cells(
        sheet_id,
        &sheet_ref,
        mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        row_index = row_index,
        staff_fingerprint = %state.log_fingerprint(staff_email),
        "cleared check-in fields (undo)"
    );

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(())
}

/// Mark an attendee as claimed by writing wallet, claimed_at, and nft_proof_url columns.
/// Called after a successful cNFT mint to persist the claim on the Google Sheet.
#[allow(clippy::too_many_arguments)]
pub async fn mark_claimed(
    row: SheetRow,
    wallet_address: &str,
    claimed_at: &str,
    nft_proof_url: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<String, String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    let access_token = get_cached_access_token(state, kv).await?;
    let row_index = resolve_row(&row, sheet_id, &sheet_ref, &access_token).await?;

    let cells = vec![
        (CK::SolanaAddress, wallet_address.to_string()),
        (CK::ClaimedAt, claimed_at.to_string()),
        (CK::NftProofUrl, nft_proof_url.to_string()),
    ];
    write_cells(
        sheet_id,
        &sheet_ref,
        mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        row_index = row_index,
        wallet_fingerprint = %state.log_fingerprint(wallet_address),
        nft_proof_url = %nft_proof_url,
        "marked row as claimed"
    );

    Ok(claimed_at.to_string())
}

/// Bulk update QR code URLs for approved attendees.
/// Updates column Q (qr_code_url) for each attendee.
pub async fn update_qr_urls(
    updates: &[(SheetRow, String)],
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<usize, String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    if updates.is_empty() {
        return Ok(0);
    }

    let access_token = get_cached_access_token(state, kv).await?;
    let updates = resolve_rows(updates.to_vec(), sheet_id, &sheet_ref, &access_token).await?;

    // Build batch update with individual value ranges
    let data: Vec<ValueRange> = updates
        .iter()
        .filter_map(|(row_index, url)| {
            a1::cell(&sheet_ref, mapping, CK::QrCodeUrl, *row_index, url.clone())
        })
        .collect();
    send(sheet_id, data, &access_token).await?;

    let updated = updates.len();
    tracing::info!(count = updated, "updated qr code urls in google sheets");

    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;
    Ok(updated)
}
