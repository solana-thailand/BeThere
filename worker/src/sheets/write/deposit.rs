//! Deposit, refund, and bank info write operations.

use crate::sheets::a1;
use event_checkin_domain::models::attendee::{ColumnKey as CK, ColumnMapping};
use worker::KvStore;

use crate::http::ValueRange;
use crate::state::AppState;

use super::SheetContext;
use crate::sheets::locate::resolve_row;
use crate::sheets::values::{send, write_cells};
use crate::sheets::{
    get_attendees, get_cached_access_token, get_column_mapping, invalidate_column_map_cache,
};
use event_checkin_domain::models::attendee::SheetRow;

/// Write bank account info (bank_account, bank_name, account_name) to the sheet.
/// Used when THB deposit slip is uploaded so the organizer has refund details.
#[allow(clippy::too_many_arguments)]
pub async fn write_bank_info(
    row: SheetRow,
    bank_account: Option<&str>,
    bank_name: Option<&str>,
    account_name: Option<&str>,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    // Skip if nothing to write
    if bank_account.is_none() && bank_name.is_none() && account_name.is_none() {
        return Ok(());
    }

    let access_token = get_cached_access_token(state, kv).await?;
    let row_index = resolve_row(&row, sheet_id, &sheet_ref, &access_token).await?;

    let cells = [
        (CK::BankAccount, bank_account),
        (CK::BankName, bank_name),
        (CK::AccountName, account_name),
    ]
    .into_iter()
    .filter_map(|(key, value)| Some((key, value?.to_string())))
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
        bank_account_col = ?mapping.column_letter(CK::BankAccount),
        bank_name_col = ?mapping.column_letter(CK::BankName),
        account_name_col = ?mapping.column_letter(CK::AccountName),
        "wrote bank info to google sheet"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Deposit verification — write N (deposit_method), O (deposit_amount), Q (deposit_verified)
// ---------------------------------------------------------------------------

/// Write deposit verification columns to the Google Sheet.
/// Called when a deposit is verified (THB slip approved or USDC on-chain confirmed).
pub async fn write_deposit_verification(
    row: SheetRow,
    deposit_method: &str,
    deposit_amount: &str,
    verified: bool,
    ctx: &SheetContext<'_>,
) -> Result<(), String> {
    let access_token = get_cached_access_token(ctx.state, ctx.kv).await?;
    let sheet_ref = a1::sheet_ref(ctx.sheet_name);
    let row_index = resolve_row(&row, ctx.sheet_id, &sheet_ref, &access_token).await?;

    let verified_cell = match verified {
        true => "Yes",
        false => "No",
    };
    let cells = vec![
        (CK::DepositMethod, deposit_method.to_string()),
        (CK::DepositAmount, deposit_amount.to_string()),
        (CK::DepositVerified, verified_cell.to_string()),
    ];
    write_cells(
        ctx.sheet_id,
        &sheet_ref,
        ctx.mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        row_index = row_index,
        method = %deposit_method,
        amount = %deposit_amount,
        verified = verified,
        "wrote deposit verification to google sheet"
    );

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
    let sheet_ref = a1::sheet_ref(sheet_name);
    let access_token = get_cached_access_token(state, kv).await?;

    // Find the attendee row by api_id
    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    let cells = vec![(CK::DepositMethod, method.to_string())];
    write_cells(
        sheet_id,
        &sheet_ref,
        &mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        %method,
        "wrote credit deposit_method to google sheet"
    );

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
    let sheet_ref = a1::sheet_ref(sheet_name);
    // Invalidate column map cache to ensure fresh header mapping
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;

    let access_token = get_cached_access_token(state, kv).await?;

    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        column = ?mapping.column_letter(CK::RefundStatus),
        total_columns = mapping.total_columns,
        "resolved refund_status column"
    );

    let cells = vec![(CK::RefundStatus, status.to_string())];
    write_cells(
        sheet_id,
        &sheet_ref,
        &mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        %status,
        "wrote refund_status to google sheet"
    );

    Ok(())
}

/// The refund_status and refund_link cells for every row a batch refund
/// settled. Shared by this blocking writer and `bg_sync::write_refund_batch`,
/// so the batch mirrors the same two columns as the single refund.
pub fn refund_batch_ranges(
    sheet_name: &str,
    mapping: &ColumnMapping,
    rows: &[usize],
    status: &str,
    link: &str,
) -> Vec<ValueRange> {
    let sheet_ref = a1::sheet_ref(sheet_name);
    rows.iter()
        .flat_map(|row| {
            [
                a1::cell(
                    &sheet_ref,
                    mapping,
                    CK::RefundStatus,
                    *row,
                    status.to_string(),
                ),
                a1::cell(&sheet_ref, mapping, CK::RefundLink, *row, link.to_string()),
            ]
        })
        .flatten()
        .collect()
}

/// Write refund_status and refund_link for multiple attendees in a single
/// batch update. Takes pre-resolved row indexes to avoid N attendee lookups.
/// Used by the batch THB refund.
#[allow(clippy::too_many_arguments)]
pub async fn write_refund_batch(
    rows: &[usize],
    status: &str,
    link: &str,
    mapping: &ColumnMapping,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }

    let access_token = get_cached_access_token(state, kv).await?;
    let data = refund_batch_ranges(sheet_name, mapping, rows, status, link);
    send(sheet_id, data, &access_token).await?;

    tracing::info!(count = rows.len(), "wrote batch refund to google sheet");

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
    let sheet_ref = a1::sheet_ref(sheet_name);
    // Invalidate column map cache to ensure fresh header mapping
    invalidate_column_map_cache(kv, sheet_id, sheet_name).await;

    let access_token = get_cached_access_token(state, kv).await?;

    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let row_index = attendees
        .iter()
        .find(|a| a.api_id == attendee_api_id)
        .map(|a| a.row_index)
        .ok_or_else(|| format!("attendee {attendee_api_id} not found"))?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        column = ?mapping.column_letter(CK::RefundLink),
        total_columns = mapping.total_columns,
        "resolved refund_link column"
    );

    let cells = vec![(CK::RefundLink, link.to_string())];
    write_cells(
        sheet_id,
        &sheet_ref,
        &mapping,
        row_index,
        cells,
        &access_token,
    )
    .await?;

    tracing::info!(
        %attendee_api_id,
        row_index,
        "wrote refund_link to google sheet"
    );

    Ok(())
}
