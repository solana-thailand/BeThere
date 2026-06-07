//! Deposit, refund, and bank info write operations.

use event_checkin_domain::models::attendee::ColumnMapping;
use worker::KvStore;

use crate::http::{BatchUpdateRequest, ValueRange, batch_update_sheet};
use crate::state::AppState;

use super::SheetContext;
use crate::sheets::{
    get_attendees, get_cached_access_token, get_column_mapping, invalidate_column_map_cache,
};

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
    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv)
        .await
        .unwrap_or_else(|_| ColumnMapping::hardcoded());

    let attendees = get_attendees(state, sheet_id, sheet_name, kv).await?;
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

    Ok(())
}
